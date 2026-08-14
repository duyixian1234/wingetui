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
    AppState, BackgroundEvent, InstallState, SearchState, UninstallState, UpgradeState,
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
    pub fn new(winget: Winget) -> (Self, UnboundedReceiver<BackgroundEvent>) {
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
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
        self.state = match code {
            KeyCode::Char('s') => AppState::Search(SearchState::new()),
            KeyCode::Char('u') => AppState::Upgrade(UpgradeState::new()),
            KeyCode::Char('i') => AppState::Install(InstallState::new()),
            KeyCode::Char('x') => AppState::Uninstall(UninstallState::new()),
            KeyCode::Char('q') => {
                self.should_quit = true;
                AppState::MainMenu
            }
            _ => AppState::MainMenu,
        };
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
        }
    }

    fn handle_install(&mut self, code: KeyCode) {
        if code == KeyCode::Esc {
            self.state = AppState::MainMenu;
        }
    }

    fn handle_uninstall(&mut self, code: KeyCode) {
        if code == KeyCode::Esc {
            self.state = AppState::MainMenu;
        }
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

    #[test]
    fn main_menu_search_key_goes_to_search() {
        let mut a = app();
        a.update(key(KeyCode::Char('s')));
        assert_eq!(a.state, AppState::Search(SearchState::new()));
    }

    #[test]
    fn main_menu_upgrade_key_goes_to_upgrade() {
        let mut a = app();
        a.update(key(KeyCode::Char('u')));
        assert_eq!(a.state, AppState::Upgrade(UpgradeState::new()));
    }

    #[test]
    fn main_menu_install_key_goes_to_install() {
        let mut a = app();
        a.update(key(KeyCode::Char('i')));
        assert_eq!(a.state, AppState::Install(InstallState::new()));
    }

    #[test]
    fn main_menu_uninstall_key_goes_to_uninstall() {
        let mut a = app();
        a.update(key(KeyCode::Char('x')));
        assert_eq!(a.state, AppState::Uninstall(UninstallState::new()));
    }

    #[test]
    fn main_menu_q_quits() {
        let mut a = app();
        a.update(key(KeyCode::Char('q')));
        assert!(a.should_quit);
    }

    #[test]
    fn ctrl_c_quits_from_any_state() {
        let mut a = app();
        a.update(key(KeyCode::Char('s')));
        assert_eq!(a.state, AppState::Search(SearchState::new()));
        a.update(ctrl_c());
        assert!(a.should_quit);
    }

    #[test]
    fn unknown_main_menu_key_stays() {
        let mut a = app();
        a.update(key(KeyCode::Char('z')));
        assert_eq!(a.state, AppState::MainMenu);
    }

    #[test]
    fn esc_from_search_returns_to_main_menu() {
        let mut a = app();
        a.update(key(KeyCode::Char('s')));
        a.update(key(KeyCode::Esc));
        assert_eq!(a.state, AppState::MainMenu);
    }

    #[test]
    fn esc_from_upgrade_returns_to_main_menu() {
        let mut a = app();
        a.update(key(KeyCode::Char('u')));
        a.update(key(KeyCode::Esc));
        assert_eq!(a.state, AppState::MainMenu);
    }

    #[test]
    fn esc_from_install_returns_to_main_menu() {
        let mut a = app();
        a.update(key(KeyCode::Char('i')));
        a.update(key(KeyCode::Esc));
        assert_eq!(a.state, AppState::MainMenu);
    }

    #[test]
    fn esc_from_uninstall_returns_to_main_menu() {
        let mut a = app();
        a.update(key(KeyCode::Char('x')));
        a.update(key(KeyCode::Esc));
        assert_eq!(a.state, AppState::MainMenu);
    }

    #[test]
    fn esc_from_log_returns_to_main_menu() {
        let mut a = app();
        a.state = AppState::Log(LogState::new());
        a.update(key(KeyCode::Esc));
        assert_eq!(a.state, AppState::MainMenu);
    }

    #[test]
    fn resize_event_does_not_crash_or_change_state() {
        let mut a = app();
        a.update(key(KeyCode::Char('s')));
        a.update(Event::Resize(100, 40));
        assert_eq!(a.state, AppState::Search(SearchState::new()));
        assert!(!a.should_quit);
    }

    #[test]
    fn background_search_done_updates_results() {
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

    #[test]
    fn background_log_line_appends_to_log_state() {
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

    #[test]
    fn background_action_done_marks_log_finished() {
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
