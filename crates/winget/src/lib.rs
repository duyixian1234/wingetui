//! winget CLI 交互层。
//!
//! 统一经 `tokio::process::Command` 调用 winget CLI：
//! - 查询类命令（search / upgrade / list）**不带 `--output json`**（v1.29.280 不支持），
//!   解析文本表格输出（先解码字节流 UTF-8→GBK 回退）
//! - 变更类命令（upgrade / install / uninstall）非交互执行，实时回传 stdout/stderr 行到日志通道
//! - 命令执行失败（非零退出码）→ `WingetError::CommandFailed`（附解码后 stderr）

pub mod commands;
pub mod error;
pub mod models;
pub mod parser;
pub mod validate;

use std::process::Stdio;

use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::sync::mpsc::UnboundedSender;

pub use error::{timeouts, WingetError};
pub use models::Package;

/// winget 无状态门面。
///
/// 默认调用 PATH 中的 `winget`；测试可经 [`Winget::with_program`] 注入 mock 二进制路径。
/// `Clone` 供后台任务 move 门面副本进入 tokio task。
#[derive(Clone)]
pub struct Winget {
    program: String,
    /// 变更类命令实时 stdout/stderr 行回传通道。
    log_sink: Option<UnboundedSender<String>>,
}

impl Default for Winget {
    fn default() -> Self {
        Self::new()
    }
}

impl Winget {
    /// 创建门面，默认调用 `winget`。
    pub fn new() -> Self {
        Self {
            program: "winget".to_string(),
            log_sink: None,
        }
    }

    /// 指定 winget/mock 可执行程序路径（测试用）。
    pub fn with_program(program: impl Into<String>) -> Self {
        Self {
            program: program.into(),
            log_sink: None,
        }
    }

    /// 设置变更类命令实时输出回传通道；`None` 表示丢弃。
    pub fn set_log_sink(&mut self, sink: Option<UnboundedSender<String>>) {
        self.log_sink = sink;
    }

    fn command(&self, args: &[String]) -> tokio::process::Command {
        let mut cmd = tokio::process::Command::new(&self.program);
        cmd.args(args);
        cmd
    }

    /// 查询类：搜索包。无匹配 → `NotFound`。
    pub async fn search(&self, query: &str) -> Result<Vec<Package>, WingetError> {
        validate::validate_package_input(query)?;
        let bytes = self.run_query(&commands::search_args(query)).await?;
        let pkgs = self.parse_query_output(&bytes)?;
        if pkgs.is_empty() {
            return Err(WingetError::NotFound);
        }
        Ok(pkgs)
    }

    /// 查询类：列出可升级包。无匹配 → `NotFound`。
    pub async fn list_upgradeable(&self) -> Result<Vec<Package>, WingetError> {
        let bytes = self.run_query(&commands::list_upgradeable_args()).await?;
        let pkgs = self.parse_query_output(&bytes)?;
        if pkgs.is_empty() {
            return Err(WingetError::NotFound);
        }
        Ok(pkgs)
    }

    /// 查询类：列出已安装包。无匹配 → `NotFound`。
    pub async fn list_installed(&self) -> Result<Vec<Package>, WingetError> {
        let bytes = self.run_query(&commands::list_installed_args()).await?;
        let pkgs = self.parse_query_output(&bytes)?;
        if pkgs.is_empty() {
            return Err(WingetError::NotFound);
        }
        Ok(pkgs)
    }

    /// 查询输出字节流 → 解码 → 文本表格解析。
    fn parse_query_output(&self, bytes: &[u8]) -> Result<Vec<Package>, WingetError> {
        let text = parser::decode_winget_output(bytes);
        parser::parse_packages_text(&text)
    }

    /// 变更类：升级指定包（`Some(id)`）或全部（`None`）。
    pub async fn upgrade(&self, id: Option<&str>) -> Result<(), WingetError> {
        let args = match id {
            Some(id) => {
                validate::validate_package_input(id)?;
                commands::upgrade_id_args(id)
            }
            None => commands::upgrade_all_args(),
        };
        self.run_action(&args).await
    }

    /// 变更类：安装指定包。
    pub async fn install(&self, id: &str) -> Result<(), WingetError> {
        validate::validate_package_input(id)?;
        self.run_action(&commands::install_args(id)).await
    }

    /// 变更类：卸载指定包。
    pub async fn uninstall(&self, id: &str) -> Result<(), WingetError> {
        validate::validate_package_input(id)?;
        self.run_action(&commands::uninstall_args(id)).await
    }

    /// 执行查询类命令：等待完成，返回 stdout 原始字节；非零退出 → `CommandFailed`（stderr 解码）。
    async fn run_query(&self, args: &[String]) -> Result<Vec<u8>, WingetError> {
        let mut cmd = self.command(args);
        cmd.stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);

        let child = cmd
            .spawn()
            .map_err(|e| WingetError::Io(format!("无法启动 winget: {e}")))?;

        let output = tokio::time::timeout(timeouts::QUERY_TIMEOUT, child.wait_with_output())
            .await
            .map_err(|_| WingetError::Timeout)?
            .map_err(|e| WingetError::Io(e.to_string()))?;

        if output.status.success() {
            Ok(output.stdout)
        } else {
            Err(WingetError::CommandFailed {
                code: output.status.code().unwrap_or(-1),
                stderr: parser::decode_winget_output(&output.stderr),
            })
        }
    }

    /// 执行变更类命令：逐行回传 stdout/stderr 到日志通道；非零退出 → `CommandFailed`。
    ///
    /// 行读取按**字节**切行后经 [`parser::decode_winget_output`] 解码，
    /// 兼容 GBK/UTF-8 混合输出（`AsyncBufReadExt::lines` 遇非 UTF-8 会报错中断）。
    async fn run_action(&self, args: &[String]) -> Result<(), WingetError> {
        let mut cmd = self.command(args);
        cmd.stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);

        let mut child = cmd
            .spawn()
            .map_err(|e| WingetError::Io(format!("无法启动 winget: {e}")))?;

        let stdout = child.stdout.take().expect("stdout 已 piped");
        let stderr = child.stderr.take().expect("stderr 已 piped");

        let sink_stdout = self.log_sink.clone();
        let sink_stderr = self.log_sink.clone();
        let read_stdout = tokio::spawn(stream_lines(stdout, sink_stdout));
        let read_stderr = tokio::spawn(stream_lines(stderr, sink_stderr));

        let status = tokio::time::timeout(timeouts::ACTION_TIMEOUT, child.wait())
            .await
            .map_err(|_| {
                let _ = child.start_kill();
                WingetError::Timeout
            })?
            .map_err(|e| WingetError::Io(e.to_string()))?;

        let stderr_lines = read_stderr
            .await
            .map_err(|e| WingetError::Io(e.to_string()))?;
        let _ = read_stdout.await;

        if status.success() {
            Ok(())
        } else {
            Err(WingetError::CommandFailed {
                code: status.code().unwrap_or(-1),
                stderr: stderr_lines.join("\n"),
            })
        }
    }
}

/// 逐行读取流（按字节切行，解码 UTF-8→GBK 回退）并回传到日志通道；
/// 同时收集行用于错误信息。
async fn stream_lines<R>(reader: R, sink: Option<UnboundedSender<String>>) -> Vec<String>
where
    R: tokio::io::AsyncRead + Unpin,
{
    let mut reader = BufReader::new(reader);
    let mut collected = Vec::new();
    loop {
        let mut buf = Vec::new();
        match reader.read_until(b'\n', &mut buf).await {
            Ok(0) => break,
            Ok(_) => {
                let line = parser::decode_winget_output(&buf);
                let line = line.trim_end_matches(&['\r', '\n'][..]).to_string();
                if let Some(sink) = &sink {
                    let _ = sink.send(line.clone());
                }
                collected.push(line);
            }
            Err(_) => break,
        }
    }
    collected
}
