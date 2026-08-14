//! 应用流程集成测试：经 mock winget 验证搜索/升级/安装/卸载流程端到端行为。
//!
//! 状态机纯逻辑（无终端）由事件驱动；mock winget 见 `tests/mock-winget/`。

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::OnceLock;
use std::time::Duration;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use tokio::sync::mpsc::UnboundedReceiver;
use winget::Winget;
use wingetui::app::App;
use wingetui::event::Event;
use wingetui::state::{AppState, BackgroundEvent};

fn mock_winget_path() -> &'static Path {
    static BIN: OnceLock<PathBuf> = OnceLock::new();
    BIN.get_or_init(|| {
        let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
        let mock_dir = manifest_dir.join("tests/mock-winget");
        let exe = if cfg!(windows) {
            "mock-winget.exe"
        } else {
            "mock-winget"
        };
        let bin = mock_dir.join("target/debug").join(exe);
        if !bin.exists() {
            let status = Command::new("cargo")
                .arg("build")
                .arg("--manifest-path")
                .arg(mock_dir.join("Cargo.toml"))
                .status()
                .expect("failed to invoke cargo build for mock-winget");
            assert!(status.success(), "mock-winget build failed");
        }
        bin
    })
}

fn key(code: KeyCode) -> Event {
    Event::Key(KeyEvent::new(code, KeyModifiers::NONE))
}

fn search_app() -> (App, UnboundedReceiver<BackgroundEvent>) {
    let w = Winget::with_program(mock_winget_path().to_string_lossy().into_owned());
    let (mut app, rx) = App::new(w);
    app.update(key(KeyCode::Char('s')));
    (app, rx)
}

/// 从日志行中解析 mock 记录的完整 argv（取最后一条 `MOCK_ARGV:` 行）。
fn parse_mock_argv(lines: &[String]) -> Vec<String> {
    let line = lines
        .iter()
        .rev()
        .find(|l| l.starts_with("MOCK_ARGV:"))
        .unwrap_or_else(|| panic!("日志中缺少 MOCK_ARGV 行: {lines:?}"));
    line.trim_start_matches("MOCK_ARGV:")
        .split('\u{1f}')
        .map(str::to_string)
        .collect()
}

/// 等待下一个后台事件（带超时，防挂死）。
async fn next_bg(rx: &mut UnboundedReceiver<BackgroundEvent>) -> BackgroundEvent {
    tokio::time::timeout(Duration::from_secs(5), rx.recv())
        .await
        .expect("background event within timeout")
        .expect("background channel not closed")
}

#[tokio::test]
async fn search_typing_triggers_debounced_search_and_updates_list() {
    let (mut app, mut rx) = search_app();

    // 输入 "pow"
    for c in ['p', 'o', 'w'] {
        app.update(key(KeyCode::Char(c)));
    }

    let ev = next_bg(&mut rx).await;
    app.update(Event::Background(ev));

    match app.state {
        AppState::Search(s) => {
            assert_eq!(s.query, "pow");
            assert!(!s.loading);
            assert!(s.error.is_none());
            assert_eq!(s.results.len(), 3);
            assert_eq!(s.results[0].id, "Microsoft.PowerShell");
        }
        other => panic!("expected Search, got {other:?}"),
    }
}

#[tokio::test]
async fn search_empty_input_does_not_trigger() {
    let (mut app, mut rx) = search_app();

    // 输入 "p" 再退格清空：不应发起搜索
    app.update(key(KeyCode::Char('p')));
    app.update(key(KeyCode::Backspace));

    let timed_out = tokio::time::timeout(Duration::from_millis(500), rx.recv()).await;
    assert!(
        timed_out.is_err(),
        "空输入不应触发搜索任务（不应收到后台事件）"
    );
    match app.state {
        AppState::Search(s) => {
            assert_eq!(s.query, "");
            assert!(!s.loading);
            assert!(s.results.is_empty());
        }
        other => panic!("expected Search, got {other:?}"),
    }
}

#[tokio::test]
async fn search_over_200_chars_shows_error_and_no_subprocess() {
    let (mut app, mut rx) = search_app();

    for _ in 0..201 {
        app.update(key(KeyCode::Char('a')));
    }

    match app.state {
        AppState::Search(s) => {
            assert!(s.error.is_some(), "应显示超长输入错误");
            assert!(!s.loading, "非法输入不应进入加载态");
        }
        other => panic!("expected Search, got {other:?}"),
    }

    let timed_out = tokio::time::timeout(Duration::from_millis(400), rx.recv()).await;
    assert!(timed_out.is_err(), "非法输入不应发起 subprocess");
}

#[tokio::test]
async fn search_control_char_shows_error_and_no_subprocess() {
    let (mut app, mut rx) = search_app();

    app.update(key(KeyCode::Char('\u{1}')));

    match app.state {
        AppState::Search(s) => {
            assert!(s.error.is_some(), "控制字符应报错");
        }
        other => panic!("expected Search, got {other:?}"),
    }
    let timed_out = tokio::time::timeout(Duration::from_millis(400), rx.recv()).await;
    assert!(timed_out.is_err(), "控制字符输入不应发起 subprocess");
}

#[tokio::test]
async fn search_debounce_aborts_previous_and_uses_latest_query() {
    let (mut app, mut rx) = search_app();

    // 快速连续输入，模拟防抖窗口内的输入
    app.update(key(KeyCode::Char('p')));
    // 立即输入更多字符：前一次防抖任务应被 abort
    for c in ['o', 'w', 'e', 'r'] {
        app.update(key(KeyCode::Char(c)));
    }

    let ev = next_bg(&mut rx).await;
    app.update(Event::Background(ev));

    match app.state {
        AppState::Search(s) => {
            // 防抖生效：以最新完整输入 "power" 搜索
            assert_eq!(s.query, "power");
            assert_eq!(s.results.len(), 3);
        }
        other => panic!("expected Search, got {other:?}"),
    }
}

#[tokio::test]
async fn search_esc_returns_to_main_menu_and_aborts_pending() {
    let (mut app, mut rx) = search_app();

    app.update(key(KeyCode::Char('p')));
    app.update(key(KeyCode::Esc));

    assert_eq!(app.state, AppState::MainMenu);

    // 离开搜索屏后 pending 任务被 abort，不应有事件到达
    let timed_out = tokio::time::timeout(Duration::from_millis(400), rx.recv()).await;
    assert!(timed_out.is_err(), "Esc 返回后不应再有搜索事件");
}

// ---------------------------------------------------------------------------
// 升级流程
// ---------------------------------------------------------------------------

fn upgrade_app() -> (App, UnboundedReceiver<BackgroundEvent>) {
    let w = Winget::with_program(mock_winget_path().to_string_lossy().into_owned());
    let (mut app, rx) = App::new(w);
    app.update(key(KeyCode::Char('u')));
    (app, rx)
}

#[tokio::test]
async fn upgrade_screen_auto_loads_upgradeable_list() {
    let (mut app, mut rx) = upgrade_app();

    match &app.state {
        AppState::Upgrade(s) => assert!(s.loading, "进入升级屏应处于加载态"),
        other => panic!("expected Upgrade, got {other:?}"),
    }

    let ev = next_bg(&mut rx).await;
    app.update(Event::Background(ev));

    match &app.state {
        AppState::Upgrade(s) => {
            assert!(!s.loading);
            assert_eq!(s.items.len(), 2);
            assert_eq!(s.items[0].id, "Git.Git");
            assert_eq!(s.items[0].available_version.as_deref(), Some("2.46.0"));
        }
        other => panic!("expected Upgrade, got {other:?}"),
    }
}

#[tokio::test]
async fn upgrade_selected_sends_id_and_shows_log_then_result() {
    let (mut app, mut rx) = upgrade_app();
    // 等待列表加载
    let ev = next_bg(&mut rx).await;
    app.update(Event::Background(ev));

    // 默认选中第一项 Git.Git，按 u 升级选中
    app.update(key(KeyCode::Char('u')));
    match &app.state {
        AppState::Log(_) => {}
        other => panic!("升级中应切日志屏, got {other:?}"),
    }

    // 逐行处理日志直到 ActionDone
    let mut saw_line = false;
    loop {
        let ev = next_bg(&mut rx).await;
        if matches!(ev, BackgroundEvent::ActionDone(_)) {
            app.update(Event::Background(ev));
            break;
        }
        if let BackgroundEvent::LogLine(l) = &ev {
            if l.contains("upgrading Git.Git") {
                saw_line = true;
            }
        }
        app.update(Event::Background(ev));
    }
    assert!(saw_line, "应收到升级实时日志行");

    let argv = match &app.state {
        AppState::Log(s) => {
            assert!(s.done);
            assert_eq!(s.result.as_deref(), Some("操作成功"));
            assert!(s.lines.iter().any(|l| l.contains("upgrading Git.Git")));
            parse_mock_argv(&s.lines)
        }
        other => panic!("expected Log, got {other:?}"),
    };
    assert_eq!(
        argv,
        vec![
            "upgrade",
            "--id",
            "Git.Git",
            "--silent",
            "--accept-package-agreements",
            "--accept-source-agreements",
            "--disable-interactivity",
        ]
    );
}

#[tokio::test]
async fn upgrade_all_sends_all_flag_and_shows_result() {
    let (mut app, mut rx) = upgrade_app();
    let ev = next_bg(&mut rx).await;
    app.update(Event::Background(ev));

    // 升级全部
    app.update(key(KeyCode::Char('a')));
    match &app.state {
        AppState::Log(_) => {}
        other => panic!("expected Log, got {other:?}"),
    }

    // 逐行处理日志直到 ActionDone
    loop {
        let ev = next_bg(&mut rx).await;
        if matches!(ev, BackgroundEvent::ActionDone(_)) {
            app.update(Event::Background(ev));
            break;
        }
        app.update(Event::Background(ev));
    }

    let argv = match &app.state {
        AppState::Log(s) => parse_mock_argv(&s.lines),
        other => panic!("expected Log, got {other:?}"),
    };
    assert_eq!(
        argv,
        vec![
            "upgrade",
            "--all",
            "--silent",
            "--accept-package-agreements",
            "--accept-source-agreements",
            "--disable-interactivity",
        ]
    );
}

#[tokio::test]
async fn upgrade_esc_returns_to_main_menu() {
    let (mut app, _rx) = upgrade_app();
    app.update(key(KeyCode::Esc));
    assert_eq!(app.state, AppState::MainMenu);
}

// ---------------------------------------------------------------------------
// 安装流程
// ---------------------------------------------------------------------------

fn install_app() -> (App, UnboundedReceiver<BackgroundEvent>) {
    let w = Winget::with_program(mock_winget_path().to_string_lossy().into_owned());
    let (mut app, rx) = App::new(w);
    app.update(key(KeyCode::Char('i')));
    (app, rx)
}

#[tokio::test]
async fn install_valid_id_confirm_then_runs_and_shows_log() {
    let (mut app, mut rx) = install_app();
    // 输入 Git.Git
    for c in ['G', 'i', 't', '.', 'G', 'i', 't'] {
        app.update(key(KeyCode::Char(c)));
    }
    // 首次 Enter：确认态（不执行）
    app.update(key(KeyCode::Enter));
    match &app.state {
        AppState::Install(s) => {
            assert_eq!(
                s.pending_confirm.as_deref(),
                Some("Git.Git"),
                "应进入确认态"
            );
            assert!(!s.is_busy(), "确认态不应开始执行");
        }
        other => panic!("expected Install, got {other:?}"),
    }
    // 再次 Enter：执行安装
    app.update(key(KeyCode::Enter));
    match &app.state {
        AppState::Log(_) => {}
        other => panic!("安装中应切日志屏, got {other:?}"),
    }
    // 等待 ActionDone（忽略中间日志行）
    loop {
        let ev = next_bg(&mut rx).await;
        if matches!(ev, BackgroundEvent::ActionDone(_)) {
            app.update(Event::Background(ev));
            break;
        }
        app.update(Event::Background(ev));
    }
    let argv = match &app.state {
        AppState::Log(s) => {
            assert!(s.done);
            assert_eq!(s.result.as_deref(), Some("操作成功"));
            assert!(s.lines.iter().any(|l| l.contains("installing Git.Git")));
            parse_mock_argv(&s.lines)
        }
        other => panic!("expected Log, got {other:?}"),
    };
    assert_eq!(
        argv,
        vec![
            "install",
            "--id",
            "Git.Git",
            "--silent",
            "--accept-package-agreements",
            "--accept-source-agreements",
            "--disable-interactivity",
        ]
    );
}

#[tokio::test]
async fn install_invalid_input_shows_error_and_no_subprocess() {
    let (mut app, mut rx) = install_app();
    for _ in 0..201 {
        app.update(key(KeyCode::Char('a')));
    }
    app.update(key(KeyCode::Enter));

    match &app.state {
        AppState::Install(s) => {
            assert!(s.error.is_some(), "超长输入应报错");
            assert_eq!(s.pending_confirm, None, "不应进入确认态");
        }
        other => panic!("expected Install, got {other:?}"),
    }
    let timed_out = tokio::time::timeout(Duration::from_millis(400), rx.recv()).await;
    assert!(timed_out.is_err(), "非法输入不应发起 subprocess");
}

#[tokio::test]
async fn install_empty_input_shows_error() {
    let (mut app, _rx) = install_app();
    app.update(key(KeyCode::Enter));
    match &app.state {
        AppState::Install(s) => {
            assert!(s.error.is_some(), "空输入应报错");
        }
        other => panic!("expected Install, got {other:?}"),
    }
}

// ---------------------------------------------------------------------------
// 卸载流程
// ---------------------------------------------------------------------------

fn uninstall_app() -> (App, UnboundedReceiver<BackgroundEvent>) {
    let w = Winget::with_program(mock_winget_path().to_string_lossy().into_owned());
    let (mut app, rx) = App::new(w);
    app.update(key(KeyCode::Char('x')));
    (app, rx)
}

#[tokio::test]
async fn uninstall_screen_auto_loads_installed_list() {
    let (mut app, mut rx) = uninstall_app();
    match &app.state {
        AppState::Uninstall(s) => assert!(s.loading, "进入卸载屏应处于加载态"),
        other => panic!("expected Uninstall, got {other:?}"),
    }
    let ev = next_bg(&mut rx).await;
    app.update(Event::Background(ev));
    match &app.state {
        AppState::Uninstall(s) => {
            assert!(!s.loading);
            assert_eq!(s.items.len(), 2);
            assert_eq!(s.items[0].id, "Microsoft.PowerShell");
        }
        other => panic!("expected Uninstall, got {other:?}"),
    }
}

#[tokio::test]
async fn uninstall_selected_sends_id_and_shows_log() {
    let (mut app, mut rx) = uninstall_app();
    let ev = next_bg(&mut rx).await;
    app.update(Event::Background(ev));

    // 默认选中第一项 Microsoft.PowerShell，Enter 卸载
    app.update(key(KeyCode::Enter));
    match &app.state {
        AppState::Log(_) => {}
        other => panic!("卸载中应切日志屏, got {other:?}"),
    }
    let mut saw_line = false;
    loop {
        let ev = next_bg(&mut rx).await;
        if matches!(ev, BackgroundEvent::ActionDone(_)) {
            app.update(Event::Background(ev));
            break;
        }
        if let BackgroundEvent::LogLine(l) = &ev {
            if l.contains("uninstalling Microsoft.PowerShell") {
                saw_line = true;
            }
        }
        app.update(Event::Background(ev));
    }
    assert!(saw_line, "应收到卸载实时日志行");
    let argv = match &app.state {
        AppState::Log(s) => {
            assert!(s.done);
            assert_eq!(s.result.as_deref(), Some("操作成功"));
            parse_mock_argv(&s.lines)
        }
        other => panic!("expected Log, got {other:?}"),
    };
    assert_eq!(
        argv,
        vec![
            "uninstall",
            "--id",
            "Microsoft.PowerShell",
            "--silent",
            "--accept-package-agreements",
            "--accept-source-agreements",
            "--disable-interactivity",
        ]
    );
}

#[tokio::test]
async fn uninstall_esc_returns_to_main_menu() {
    let (mut app, _rx) = uninstall_app();
    app.update(key(KeyCode::Esc));
    assert_eq!(app.state, AppState::MainMenu);
}
