//! winget 交互层集成测试：经 mock winget 二进制验证端到端行为。
//!
//! mock winget 是独立于 workspace 的测试辅助二进制（`tests/mock-winget/`），
//! 首次运行自动构建并缓存，后续复用。

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Mutex, OnceLock};

use tokio::sync::mpsc;
use winget::{Winget, WingetError};

fn mock_winget_path() -> &'static Path {
    static BIN: OnceLock<PathBuf> = OnceLock::new();
    BIN.get_or_init(|| {
        let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
        let mock_dir = manifest_dir.join("../../tests/mock-winget");
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

/// 串行执行需要读写 `MOCK_WINGET_LOG` 环境变量的测试，
/// 避免并行测试间 env 竞态；RAII 保证结束时清理 env。
fn with_argv_log<F>(f: F)
where
    F: FnOnce(&Path),
{
    static LOCK: Mutex<()> = Mutex::new(());
    let _guard = LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let dir = tempfile::tempdir().expect("tempdir");
    let log = dir.path().join("argv.log");
    std::env::set_var("MOCK_WINGET_LOG", &log);
    f(&log);
    std::env::remove_var("MOCK_WINGET_LOG");
}

fn winget() -> Winget {
    Winget::with_program(mock_winget_path().to_string_lossy().into_owned())
}

/// 读取 mock 记录的 argv（每行一个参数）。
fn read_argv(log: &Path) -> Vec<String> {
    std::fs::read_to_string(log)
        .expect("argv log written")
        .lines()
        .map(str::to_string)
        .collect()
}

#[tokio::test]
async fn search_returns_fixture_packages() {
    let w = winget();
    let pkgs = w.search("powershell").await.expect("search should succeed");
    assert_eq!(pkgs.len(), 3);
    assert_eq!(pkgs[0].id, "Microsoft.PowerShell");
    assert_eq!(pkgs[0].name, "PowerShell");
    assert_eq!(pkgs[0].version, "7.4.5");
    assert_eq!(pkgs[0].source.as_deref(), Some("winget"));
}

#[tokio::test]
async fn search_malformed_json_returns_parse_error() {
    let w = winget();
    let err = w.search("__malformed__").await.unwrap_err();
    assert!(matches!(err, WingetError::Parse(_)), "got {err:?}");
}

#[tokio::test]
async fn search_not_found_returns_not_found() {
    let w = winget();
    let err = w.search("__notfound__").await.unwrap_err();
    assert!(matches!(err, WingetError::NotFound), "got {err:?}");
}

#[tokio::test]
async fn search_nonzero_exit_returns_command_failed_with_stderr() {
    let w = winget();
    let err = w.search("__error__").await.unwrap_err();
    match err {
        WingetError::CommandFailed { code, stderr } => {
            assert_ne!(code, 0);
            assert!(
                stderr.contains("simulated search failure"),
                "stderr: {stderr}"
            );
        }
        other => panic!("expected CommandFailed, got {other:?}"),
    }
}

#[test]
fn search_argv_no_shell_concatenation() {
    with_argv_log(|log| {
        let rt = tokio::runtime::Runtime::new().expect("runtime");
        rt.block_on(async {
            let w = winget();
            w.search("powershell").await.expect("search should succeed");
        });
        let argv = read_argv(log);
        assert_eq!(
            argv,
            vec![
                "search",
                "--query",
                "powershell",
                "--output",
                "json",
                "--disable-interactivity",
                "--accept-source-agreements",
            ]
        );
    });
}

#[test]
fn upgrade_selected_argv_and_logs_lines() {
    with_argv_log(|log| {
        let rt = tokio::runtime::Runtime::new().expect("runtime");
        rt.block_on(async {
            let (tx, mut rx) = mpsc::unbounded_channel::<String>();
            let mut w = winget();
            w.set_log_sink(Some(tx));
            w.upgrade(Some("Git.Git"))
                .await
                .expect("upgrade should succeed");

            // 日志区逐行收到 mock 输出（stdout 3 行）
            let mut lines = Vec::new();
            while let Ok(line) = rx.try_recv() {
                lines.push(line);
            }
            assert!(
                lines.iter().any(|l| l.contains("upgrading Git.Git")),
                "lines: {lines:?}"
            );
        });
        let argv = read_argv(log);
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
    });
}

#[test]
fn upgrade_all_argv() {
    with_argv_log(|log| {
        let rt = tokio::runtime::Runtime::new().expect("runtime");
        rt.block_on(async {
            let w = winget();
            w.upgrade(None).await.expect("upgrade --all should succeed");
        });
        let argv = read_argv(log);
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
    });
}

#[tokio::test]
async fn upgrade_failure_returns_command_failed_with_stderr() {
    let w = winget();
    let err = w.upgrade(Some("__error__")).await.unwrap_err();
    match err {
        WingetError::CommandFailed { code, stderr } => {
            assert_ne!(code, 0);
            assert!(
                stderr.contains("simulated upgrade failure"),
                "stderr: {stderr}"
            );
        }
        other => panic!("expected CommandFailed, got {other:?}"),
    }
}

#[tokio::test]
async fn list_upgradeable_returns_packages() {
    let w = winget();
    let pkgs = w.list_upgradeable().await.expect("list upgradeable");
    assert_eq!(pkgs.len(), 2);
    assert_eq!(pkgs[0].id, "Git.Git");
    assert_eq!(pkgs[0].available_version.as_deref(), Some("2.46.0"));
}

#[tokio::test]
async fn list_installed_returns_packages() {
    let w = winget();
    let pkgs = w.list_installed().await.expect("list installed");
    assert_eq!(pkgs.len(), 2);
    assert_eq!(pkgs[0].id, "Microsoft.PowerShell");
}

#[test]
fn install_argv_and_success() {
    with_argv_log(|log| {
        let rt = tokio::runtime::Runtime::new().expect("runtime");
        rt.block_on(async {
            let w = winget();
            w.install("Git.Git").await.expect("install should succeed");
        });
        let argv = read_argv(log);
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
    });
}

#[test]
fn uninstall_argv_and_success() {
    with_argv_log(|log| {
        let rt = tokio::runtime::Runtime::new().expect("runtime");
        rt.block_on(async {
            let w = winget();
            w.uninstall("Microsoft.PowerShell")
                .await
                .expect("uninstall should succeed");
        });
        let argv = read_argv(log);
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
    });
}

#[test]
fn invalid_input_rejected_before_subprocess() {
    with_argv_log(|log| {
        let rt = tokio::runtime::Runtime::new().expect("runtime");
        rt.block_on(async {
            let w = winget();
            let err = w.search("\u{0000}bad").await.unwrap_err();
            assert!(matches!(err, WingetError::Validation(_)));
        });
        // 校验失败不应发起 subprocess：mock 日志文件不应存在
        assert!(
            !log.exists(),
            "subprocess should not be spawned on validation failure"
        );
    });
}
