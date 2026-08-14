//! 卸载屏状态。

use winget::Package;

/// 当前进行的卸载操作（用于防重复提交）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UninstallAction {
    pub index: usize,
}

/// 卸载屏状态：已安装列表 + 选中项 + 加载态 + 操作状态。
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct UninstallState {
    pub items: Vec<Package>,
    pub selected: usize,
    pub loading: bool,
    pub action: Option<UninstallAction>,
    pub message: Option<String>,
}

impl UninstallState {
    pub fn new() -> Self {
        Self::default()
    }

    /// 选中项下移（循环）。
    pub fn select_next(&mut self) {
        if self.items.is_empty() {
            return;
        }
        self.selected = (self.selected + 1) % self.items.len();
    }

    /// 选中项上移（循环）。
    pub fn select_prev(&mut self) {
        if self.items.is_empty() {
            return;
        }
        self.selected = if self.selected == 0 {
            self.items.len() - 1
        } else {
            self.selected - 1
        };
    }
}
