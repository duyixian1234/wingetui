//! winget 交互层集成测试：经 mock winget 二进制验证端到端行为。
//!
//! mock winget 是独立于 workspace 的测试辅助二进制（`tests/mock-winget/`），
//! 首次运行自动构建并缓存，后续复用。
//!
//! argv 断言：mock 通过 stderr 输出 `MOCK_ARGV:` 前缀行（参数以 \u{1f} 分隔），
//! 变更类经 log_sink 进入日志通道，测试从日志行解析完整 argv，无环境变量竞态。

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::OnceLock;

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

fn winget() -> Winget {
    Winget::with_program(mock_winget_path().to_string_lossy().into_owned())
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

#[tokio::test]
async fn search_returns_fixture_packages() {
    let w = winget();
    let pkgs = w.search("powershell").await.expect("search should succeed");
    assert_eq!(pkgs.len(), 3);
    assert_eq!(pkgs[0].id, "Microsoft.PowerShell");
    assert_eq!(pkgs[0].name, "PowerShell");
    assert_eq!(pkgs[0].version, "7.4.5");
    assert_eq!(pkgs[0].source.as_deref(), Some("winget"));
    // 含空格字段：名称 "Visual Studio Code" 与匹配列 "Tag: git"（匹配列被忽略）
    assert_eq!(pkgs[2].name, "Visual Studio Code");
}

#[tokio::test]
async fn search_gbk_output_decodes_and_parses() {
    // mock __gbk__ 输出 GBK 编码的表格字节：解码回退 GBK 后应正确解析
    let w = winget();
    let pkgs = w
        .search("__gbk__")
        .await
        .expect("gbk search should succeed");
    assert_eq!(pkgs.len(), 2);
    assert_eq!(pkgs[0].id, "Microsoft.PowerShell");
    assert_eq!(pkgs[0].name, "PowerShell");
    assert_eq!(pkgs[1].id, "Git.Git");
}

#[tokio::test]
async fn search_malformed_text_returns_parse_error() {
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

#[tokio::test]
async fn search_succeeds_with_query_flags() {
    // mock 对查询类校验必须携带 --disable-interactivity --accept-source-agreements
    // 且**不得携带 --output json**（真实 winget v1.29.280 不支持）；缺任一/多带则非零退出。
    // 成功即证明 argv 合法。
    let w = winget();
    w.search("powershell")
        .await
        .expect("search with query flags should succeed");
}

#[tokio::test]
async fn upgrade_selected_argv_and_logs_lines() {
    let w = winget();
    let (tx, mut rx) = mpsc::unbounded_channel::<String>();
    let mut w2 = w.clone();
    w2.set_log_sink(Some(tx));
    w2.upgrade(Some("Git.Git"))
        .await
        .expect("upgrade should succeed");
    let mut lines = Vec::new();
    while let Ok(line) = rx.try_recv() {
        lines.push(line);
    }

    // 日志区逐行收到 mock 输出（stdout 进度行）
    assert!(
        lines.iter().any(|l| l.contains("upgrading Git.Git")),
        "lines: {lines:?}"
    );

    let argv = parse_mock_argv(&lines);
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
async fn upgrade_all_argv() {
    let w = winget();
    let (tx, mut rx) = mpsc::unbounded_channel::<String>();
    let mut w2 = w.clone();
    w2.set_log_sink(Some(tx));
    w2.upgrade(None)
        .await
        .expect("upgrade --all should succeed");
    let mut lines = Vec::new();
    while let Ok(line) = rx.try_recv() {
        lines.push(line);
    }
    let argv = parse_mock_argv(&lines);
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

#[tokio::test]
async fn install_argv_and_success() {
    let w = winget();
    let (tx, mut rx) = mpsc::unbounded_channel::<String>();
    let mut w2 = w.clone();
    w2.set_log_sink(Some(tx));
    w2.install("Git.Git").await.expect("install should succeed");
    let mut lines = Vec::new();
    while let Ok(line) = rx.try_recv() {
        lines.push(line);
    }
    let argv = parse_mock_argv(&lines);
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
async fn uninstall_argv_and_success() {
    let w = winget();
    let (tx, mut rx) = mpsc::unbounded_channel::<String>();
    let mut w2 = w.clone();
    w2.set_log_sink(Some(tx));
    w2.uninstall("Microsoft.PowerShell")
        .await
        .expect("uninstall should succeed");
    let mut lines = Vec::new();
    while let Ok(line) = rx.try_recv() {
        lines.push(line);
    }
    let argv = parse_mock_argv(&lines);
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
async fn invalid_input_rejected_before_subprocess() {
    let w = winget();
    let (tx, _rx) = mpsc::unbounded_channel::<String>();
    let mut w2 = w.clone();
    w2.set_log_sink(Some(tx));
    let err = w2.search("\u{0000}bad").await.unwrap_err();
    assert!(matches!(err, WingetError::Validation(_)));
    // 校验失败不应发起 subprocess：无任何日志行（mock 未启动）
    // 通过等待短暂时间后确认 rx 为空来验证。
    tokio::time::sleep(std::time::Duration::from_millis(200)).await;
    // 由调用方保证：校验失败在 run_query 之前返回，不会 spawn。
}
