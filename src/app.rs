//! 应用主状态机：`App` 持有 `AppState` 与 `Winget` 门面，事件驱动迁移。
//!
//! `update` 是纯逻辑（不触碰终端），可被状态机单测直接驱动；
//! `run` 才进入 ratatui/crossterm 主循环。

use std::io;
use std::time::Duration;

use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};
use tokio::task::AbortHandle;
use winget::validate::validate_package_input;
use winget::Winget;

use crate::event::{Event, EventLoop};
use crate::state::{
    AppState, BackgroundEvent, InstallState, LogState, SearchState, UninstallAction,
    UninstallState, UpgradeAction, UpgradeState,
};

/// 搜索输入防抖时长。
pub const SEARCH_DEBOUNCE: Duration = Duration::from_millis(300);

/// 应用主体。
pub struct App {
    pub state: AppState,
    pub winget: Winget,
    pub background_tx: UnboundedSender<BackgroundEvent>,
    pub should_quit: bool,
    /// 搜索防抖任务句柄：输入变化时 abort 旧任务，避免重复/过期搜索。
    search_debounce: Option<AbortHandle>,
}

impl App {
    /// 创建应用，返回后台事件接收端（供事件循环合并）。
    ///
    /// 变更类命令的 stdout/stderr 行经 `winget.log_sink` → 转发任务 →
    /// `BackgroundEvent::LogLine` 汇入统一后台事件通道。
    pub fn new(winget: Winget) -> (Self, UnboundedReceiver<BackgroundEvent>) {
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
        let (log_tx, mut log_rx) = tokio::sync::mpsc::unbounded_channel::<String>();

        let mut winget = winget;
        winget.set_log_sink(Some(log_tx));

        // 转发 winget 日志行 → 后台事件（LogLine）
        let fwd_tx = tx.clone();
        tokio::spawn(async move {
            while let Some(line) = log_rx.recv().await {
                if fwd_tx.send(BackgroundEvent::LogLine(line)).is_err() {
                    break;
                }
            }
        });

        (
            Self {
                state: AppState::MainMenu,
                winget,
                background_tx: tx,
                should_quit: false,
                search_debounce: None,
            },
            rx,
        )
    }

    /// 事件驱动状态机（纯逻辑）。
    pub fn update(&mut self, event: Event) {
        match event {
            Event::Key(key) => self.handle_key(key),
            Event::Resize(..) => {
                // Resize 不改变状态，仅触发重绘（渲染层处理）。
            }
            Event::Background(bg) => self.handle_background(bg),
        }
    }

    fn handle_key(&mut self, key: KeyEvent) {
        // 仅处理按下事件；忽略 Release / Repeat，避免重复触发。
        if key.kind != KeyEventKind::Press {
            return;
        }
        if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
            self.should_quit = true;
            return;
        }
        match self.state {
            AppState::MainMenu => self.handle_main_menu(key.code),
            AppState::Search(_) => self.handle_search(key.code),
            AppState::Upgrade(_) => self.handle_upgrade(key.code),
            AppState::Install(_) => self.handle_install(key.code),
            AppState::Uninstall(_) => self.handle_uninstall(key.code),
            AppState::Log(_) => self.handle_log(key.code),
        }
    }

    fn handle_main_menu(&mut self, code: KeyCode) {
        match code {
            KeyCode::Char('s') => self.state = AppState::Search(SearchState::new()),
            KeyCode::Char('u') => {
                self.state = AppState::Upgrade(UpgradeState::new());
                self.trigger_upgrade_list();
            }
            KeyCode::Char('i') => self.state = AppState::Install(InstallState::new()),
            KeyCode::Char('x') => {
                self.state = AppState::Uninstall(UninstallState::new());
                self.trigger_installed_list();
            }
            KeyCode::Char('q') => {
                self.should_quit = true;
            }
            _ => {}
        }
    }

    fn handle_search(&mut self, code: KeyCode) {
        if code == KeyCode::Esc || code == KeyCode::BackTab {
            self.abort_search();
            self.state = AppState::MainMenu;
            return;
        }
        let changed = match code {
            KeyCode::Char(c) => {
                if let AppState::Search(s) = &mut self.state {
                    s.query.push(c);
                    true
                } else {
                    false
                }
            }
            KeyCode::Backspace => {
                if let AppState::Search(s) = &mut self.state {
                    s.query.pop();
                    true
                } else {
                    false
                }
            }
            _ => false,
        };
        if changed {
            self.trigger_search();
        }
    }

    /// 输入变化后触发防抖搜索：空输入/非法输入不发起 subprocess。
    fn trigger_search(&mut self) {
        // 取消上一次防抖任务（300ms 窗口内再次输入不重复触发）。
        self.abort_search();

        let AppState::Search(s) = &mut self.state else {
            return;
        };
        s.error = None;
        let query = s.query.trim().to_string();
        if query.is_empty() {
            s.results.clear();
            s.loading = false;
            return;
        }
        if let Err(e) = validate_package_input(&query) {
            s.error = Some(e.to_string());
            s.loading = false;
            return;
        }

        s.loading = true;
        let winget = self.winget.clone();
        let tx = self.background_tx.clone();
        let handle = tokio::spawn(async move {
            tokio::time::sleep(SEARCH_DEBOUNCE).await;
            let result = winget.search(&query).await;
            let _ = tx.send(BackgroundEvent::SearchDone(result));
        });
        self.search_debounce = Some(handle.abort_handle());
    }

    /// 中止当前搜索防抖任务。
    fn abort_search(&mut self) {
        if let Some(h) = self.search_debounce.take() {
            h.abort();
        }
    }

    fn handle_upgrade(&mut self, code: KeyCode) {
        if code == KeyCode::Esc {
            self.state = AppState::MainMenu;
            return;
        }
        match code {
            KeyCode::Char('u') => self.start_upgrade(UpgradeAction::Selected),
            KeyCode::Char('a') => self.start_upgrade(UpgradeAction::All),
            KeyCode::Char('r') => self.trigger_upgrade_list(),
            KeyCode::Up | KeyCode::Char('k') => {
                if let AppState::Upgrade(s) = &mut self.state {
                    s.select_prev();
                }
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if let AppState::Upgrade(s) = &mut self.state {
                    s.select_next();
                }
            }
            _ => {}
        }
    }

    /// 进入升级屏时自动加载可升级列表。
    fn trigger_upgrade_list(&mut self) {
        let AppState::Upgrade(s) = &mut self.state else {
            return;
        };
        s.loading = true;
        s.message = None;
        let winget = self.winget.clone();
        let tx = self.background_tx.clone();
        tokio::spawn(async move {
            let result = winget.list_upgradeable().await;
            let _ = tx.send(BackgroundEvent::UpgradeListDone(result));
        });
    }

    /// 发起升级：选中项（`Selected`）或全部（`All`）。
    ///
    /// 升级中防重复提交：state 切到 `Log` 后不再响应 `u`/`a`；
    /// 且 `UpgradeState.action` 被标记，直到任务结束/重新加载前不可再次触发。
    fn start_upgrade(&mut self, action: UpgradeAction) {
        let id = match action {
            UpgradeAction::Selected => {
                let AppState::Upgrade(s) = &self.state else {
                    return;
                };
                // 加载中或已在操作 → 防重复提交
                if s.loading || s.action.is_some() || s.items.is_empty() {
                    return;
                }
                Some(s.items[s.selected].id.clone())
            }
            UpgradeAction::All => {
                let AppState::Upgrade(s) = &self.state else {
                    return;
                };
                if s.loading || s.action.is_some() {
                    return;
                }
                None
            }
        };

        // 标记进行中（防重复）
        if let AppState::Upgrade(s) = &mut self.state {
            s.action = Some(action);
        }

        // 切到日志屏，实时显示变更输出
        self.state = AppState::Log(LogState::new());

        let winget = self.winget.clone();
        let tx = self.background_tx.clone();
        tokio::spawn(async move {
            let result = winget.upgrade(id.as_deref()).await;
            let _ = tx.send(BackgroundEvent::ActionDone(result));
        });
    }

    fn handle_install(&mut self, code: KeyCode) {
        if code == KeyCode::Esc {
            self.state = AppState::MainMenu;
            return;
        }
        match code {
            KeyCode::Char(c) => {
                if let AppState::Install(s) = &mut self.state {
                    if s.is_busy() {
                        return;
                    }
                    // 确认态下按 y 确认执行
                    if c == 'y' && s.pending_confirm.is_some() {
                        self.confirm_install();
                        return;
                    }
                    s.input.push(c);
                    s.error = None;
                    s.pending_confirm = None;
                }
            }
            KeyCode::Backspace => {
                if let AppState::Install(s) = &mut self.state {
                    if !s.is_busy() {
                        s.input.pop();
                        s.error = None;
                        s.pending_confirm = None;
                    }
                }
            }
            KeyCode::Enter => self.confirm_install(),
            _ => {}
        }
    }

    /// 安装确认：首次 Enter 校验并进入确认态；确认态再次 Enter/y 执行安装。
    fn confirm_install(&mut self) {
        let AppState::Install(s) = &mut self.state else {
            return;
        };
        if s.is_busy() {
            return;
        }
        let input = s.input.trim().to_string();
        if input.is_empty() {
            s.error = Some("请输入包 Id".to_string());
            return;
        }
        if let Err(e) = validate_package_input(&input) {
            s.error = Some(e.to_string());
            return;
        }
        if s.pending_confirm.as_deref() == Some(input.as_str()) {
            // 已确认：执行安装
            let winget = self.winget.clone();
            let tx = self.background_tx.clone();
            self.state = AppState::Log(LogState::new());
            tokio::spawn(async move {
                let result = winget.install(&input).await;
                let _ = tx.send(BackgroundEvent::ActionDone(result));
            });
        } else {
            // 首次 Enter：进入确认态
            s.pending_confirm = Some(input.clone());
            s.error = None;
        }
    }

    fn handle_uninstall(&mut self, code: KeyCode) {
        if code == KeyCode::Esc {
            self.state = AppState::MainMenu;
            return;
        }
        match code {
            KeyCode::Enter => self.start_uninstall(),
            KeyCode::Char('r') => self.trigger_installed_list(),
            KeyCode::Up | KeyCode::Char('k') => {
                if let AppState::Uninstall(s) = &mut self.state {
                    s.select_prev();
                }
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if let AppState::Uninstall(s) = &mut self.state {
                    s.select_next();
                }
            }
            _ => {}
        }
    }

    /// 进入卸载屏时自动加载已安装列表。
    fn trigger_installed_list(&mut self) {
        let AppState::Uninstall(s) = &mut self.state else {
            return;
        };
        s.loading = true;
        s.message = None;
        let winget = self.winget.clone();
        let tx = self.background_tx.clone();
        tokio::spawn(async move {
            let result = winget.list_installed().await;
            let _ = tx.send(BackgroundEvent::InstalledListDone(result));
        });
    }

    /// 卸载选中项：防重复提交，执行期间切日志屏。
    fn start_uninstall(&mut self) {
        let id = {
            let AppState::Uninstall(s) = &self.state else {
                return;
            };
            if s.loading || s.action.is_some() || s.items.is_empty() {
                return;
            }
            Some(s.items[s.selected].id.clone())
        };
        let Some(id) = id else {
            return;
        };
        if let AppState::Uninstall(s) = &mut self.state {
            s.action = Some(UninstallAction { index: s.selected });
        }
        self.state = AppState::Log(LogState::new());
        let winget = self.winget.clone();
        let tx = self.background_tx.clone();
        tokio::spawn(async move {
            let result = winget.uninstall(&id).await;
            let _ = tx.send(BackgroundEvent::ActionDone(result));
        });
    }

    fn handle_log(&mut self, code: KeyCode) {
        if code == KeyCode::Esc {
            self.state = AppState::MainMenu;
        }
    }

    /// 后台任务事件更新对应状态。
    fn handle_background(&mut self, bg: BackgroundEvent) {
        match bg {
            BackgroundEvent::SearchDone(result) => {
                if let AppState::Search(s) = &mut self.state {
                    s.loading = false;
                    match result {
                        Ok(pkgs) => {
                            s.results = pkgs;
                            s.error = None;
                        }
                        Err(e) => s.error = Some(e.to_string()),
                    }
                }
            }
            BackgroundEvent::UpgradeListDone(result) => {
                if let AppState::Upgrade(s) = &mut self.state {
                    s.loading = false;
                    match result {
                        Ok(pkgs) => {
                            s.items = pkgs;
                            s.message = None;
                        }
                        Err(e) => s.message = Some(format!("加载失败: {e}")),
                    }
                }
            }
            BackgroundEvent::InstalledListDone(result) => {
                if let AppState::Uninstall(s) = &mut self.state {
                    s.loading = false;
                    match result {
                        Ok(pkgs) => {
                            s.items = pkgs;
                            s.message = None;
                        }
                        Err(e) => s.message = Some(format!("加载失败: {e}")),
                    }
                }
            }
            BackgroundEvent::ActionDone(result) => {
                if let AppState::Log(s) = &mut self.state {
                    s.finish(match result {
                        Ok(()) => "操作成功".to_string(),
                        Err(e) => format!("操作失败: {e}"),
                    });
                }
            }
            BackgroundEvent::LogLine(line) => {
                if let AppState::Log(s) = &mut self.state {
                    s.push_line(line);
                }
            }
        }
    }
}

/// 进入 ratatui 主循环：渲染当前屏 → 等待事件 → 更新状态，直到退出。
pub async fn run(
    terminal: &mut Terminal<CrosstermBackend<std::io::Stdout>>,
    app: &mut App,
    events: &mut EventLoop,
) -> io::Result<()> {
    while !app.should_quit {
        terminal.draw(|frame| crate::ui::draw(frame, app))?;
        let event = events.next().await;
        app.update(event);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::event::Event;
    use crate::state::{AppState, LogState};
    use winget::Package;

    fn key(code: KeyCode) -> Event {
        Event::Key(KeyEvent::new(code, KeyModifiers::NONE))
    }

    fn ctrl_c() -> Event {
        Event::Key(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL))
    }

    fn app() -> App {
        App::new(Winget::new()).0
    }

    #[tokio::test]
    async fn main_menu_search_key_goes_to_search() {
        let mut a = app();
        a.update(key(KeyCode::Char('s')));
        assert_eq!(a.state, AppState::Search(SearchState::new()));
    }

    #[tokio::test]
    async fn main_menu_upgrade_key_goes_to_upgrade() {
        let mut a = app();
        a.update(key(KeyCode::Char('u')));
        match &a.state {
            AppState::Upgrade(s) => {
                // 进入升级屏自动触发加载
                assert!(s.loading);
            }
            other => panic!("expected Upgrade, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn main_menu_install_key_goes_to_install() {
        let mut a = app();
        a.update(key(KeyCode::Char('i')));
        assert_eq!(a.state, AppState::Install(InstallState::new()));
    }

    #[tokio::test]
    async fn main_menu_uninstall_key_goes_to_uninstall() {
        let mut a = app();
        a.update(key(KeyCode::Char('x')));
        match &a.state {
            AppState::Uninstall(s) => {
                // 进入卸载屏自动触发已安装列表加载
                assert!(s.loading);
            }
            other => panic!("expected Uninstall, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn main_menu_q_quits() {
        let mut a = app();
        a.update(key(KeyCode::Char('q')));
        assert!(a.should_quit);
    }

    #[tokio::test]
    async fn ctrl_c_quits_from_any_state() {
        let mut a = app();
        a.update(key(KeyCode::Char('s')));
        assert_eq!(a.state, AppState::Search(SearchState::new()));
        a.update(ctrl_c());
        assert!(a.should_quit);
    }

    #[tokio::test]
    async fn unknown_main_menu_key_stays() {
        let mut a = app();
        a.update(key(KeyCode::Char('z')));
        assert_eq!(a.state, AppState::MainMenu);
    }

    #[tokio::test]
    async fn esc_from_search_returns_to_main_menu() {
        let mut a = app();
        a.update(key(KeyCode::Char('s')));
        a.update(key(KeyCode::Esc));
        assert_eq!(a.state, AppState::MainMenu);
    }

    #[tokio::test]
    async fn esc_from_upgrade_returns_to_main_menu() {
        let mut a = app();
        a.update(key(KeyCode::Char('u')));
        a.update(key(KeyCode::Esc));
        assert_eq!(a.state, AppState::MainMenu);
    }

    #[tokio::test]
    async fn esc_from_install_returns_to_main_menu() {
        let mut a = app();
        a.update(key(KeyCode::Char('i')));
        a.update(key(KeyCode::Esc));
        assert_eq!(a.state, AppState::MainMenu);
    }

    #[tokio::test]
    async fn esc_from_uninstall_returns_to_main_menu() {
        let mut a = app();
        a.update(key(KeyCode::Char('x')));
        a.update(key(KeyCode::Esc));
        assert_eq!(a.state, AppState::MainMenu);
    }

    #[tokio::test]
    async fn esc_from_log_returns_to_main_menu() {
        let mut a = app();
        a.state = AppState::Log(LogState::new());
        a.update(key(KeyCode::Esc));
        assert_eq!(a.state, AppState::MainMenu);
    }

    #[tokio::test]
    async fn resize_event_does_not_crash_or_change_state() {
        let mut a = app();
        a.update(key(KeyCode::Char('s')));
        a.update(Event::Resize(100, 40));
        assert_eq!(a.state, AppState::Search(SearchState::new()));
        assert!(!a.should_quit);
    }

    #[tokio::test]
    async fn background_search_done_updates_results() {
        let mut a = app();
        a.update(key(KeyCode::Char('s')));
        let pkg = Package {
            id: "Microsoft.PowerShell".to_string(),
            name: "PowerShell".to_string(),
            version: "7.4.5".to_string(),
            available_version: None,
            source: Some("winget".to_string()),
        };
        a.update(Event::Background(BackgroundEvent::SearchDone(Ok(vec![
            pkg,
        ]))));
        match a.state {
            AppState::Search(s) => {
                assert_eq!(s.results.len(), 1);
                assert_eq!(s.results[0].id, "Microsoft.PowerShell");
                assert!(!s.loading);
            }
            other => panic!("expected Search, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn background_log_line_appends_to_log_state() {
        let mut a = app();
        a.state = AppState::Log(LogState::new());
        a.update(Event::Background(BackgroundEvent::LogLine(
            "installing...".to_string(),
        )));
        match a.state {
            AppState::Log(s) => {
                assert_eq!(s.lines, vec!["installing..."]);
            }
            other => panic!("expected Log, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn background_action_done_marks_log_finished() {
        let mut a = app();
        a.state = AppState::Log(LogState::new());
        a.update(Event::Background(BackgroundEvent::ActionDone(Ok(()))));
        match a.state {
            AppState::Log(s) => {
                assert!(s.done);
                assert_eq!(s.result.as_deref(), Some("操作成功"));
            }
            other => panic!("expected Log, got {other:?}"),
        }
    }
}
