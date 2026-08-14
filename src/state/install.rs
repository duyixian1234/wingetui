//! 安装屏状态。

/// 安装屏状态：输入包 Id + 操作状态。
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct InstallState {
    pub input: String,
    /// 待确认的包 Id（输入合法后等待确认执行）。
    pub pending_confirm: Option<String>,
    pub loading: bool,
    pub message: Option<String>,
    pub error: Option<String>,
}

impl InstallState {
    pub fn new() -> Self {
        Self::default()
    }

    /// 是否正在执行安装（防重复提交）。
    pub fn is_busy(&self) -> bool {
        self.loading
    }
}
