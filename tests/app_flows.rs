//! 应用流程集成测试：经 mock winget 验证搜索流程端到端行为。
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
