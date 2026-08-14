//! 升级屏状态。

use winget::Package;

/// 当前进行的升级操作（用于防重复提交）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UpgradeAction {
    /// 升级选中项。
    Selected(usize),
    /// 升级全部。
    All,
}

/// 升级屏状态：可升级列表 + 选中项 + 加载态 + 操作状态。
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct UpgradeState {
    pub items: Vec<Package>,
    pub selected: usize,
    pub loading: bool,
    pub action: Option<UpgradeAction>,
    /// 最近一次操作结果提示（成功/失败）。
    pub message: Option<String>,
}

impl UpgradeState {
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
