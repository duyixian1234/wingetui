//! 错误类型。

use std::time::Duration;

/// 全局超时配置。
pub mod timeouts {
    use super::*;

    /// 查询类命令超时：30 秒。
    pub const QUERY_TIMEOUT: Duration = Duration::from_secs(30);
    /// 变更类命令超时：10 分钟。
    pub const ACTION_TIMEOUT: Duration = Duration::from_secs(600);
}

/// winget 交互层错误。
#[derive(Debug, thiserror::Error)]
pub enum WingetError {
    /// 输入校验失败（空/控制字符/超长）。
    #[error("输入校验失败: {0}")]
    Validation(String),

    /// 无匹配包（winget 未找到）。
    #[error("未找到匹配的包")]
    NotFound,

    /// 命令执行超时（查询 30s / 变更 10min）。
    #[error("命令执行超时")]
    Timeout,

    /// IO 错误（进程无法启动等）。
    #[error("IO 错误: {0}")]
    Io(String),

    /// JSON 解析失败（字段缺失降级，整体失败才报）。
    #[error("JSON 解析失败: {0}")]
    Parse(String),

    /// winget 命令非零退出。
    #[error("命令执行失败 (code={code}): {stderr}")]
    CommandFailed { code: i32, stderr: String },
}
