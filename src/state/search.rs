//! 搜索屏状态。

use winget::Package;

/// 搜索屏状态：输入框 + 结果列表 + 加载态 + 错误提示。
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SearchState {
    pub query: String,
    pub results: Vec<Package>,
    pub loading: bool,
    pub error: Option<String>,
}

impl SearchState {
    pub fn new() -> Self {
        Self::default()
    }

    /// 清空结果与错误（输入变化时调用）。
    pub fn reset_results(&mut self) {
        self.results.clear();
        self.error = None;
    }
}
