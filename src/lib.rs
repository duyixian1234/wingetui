//! wingetui 库入口。
//!
//! TUI 主循环 / 状态机在此组织：
//! - [`app`]：`App` 状态机 + 主循环
//! - [`event`]：crossterm 事件循环
//! - [`state`]：`AppState` 状态模型 + 后台事件
//! - [`ui`]：ratatui 渲染

pub mod app;
pub mod event;
pub mod state;
pub mod ui;

/// 占位函数：返回固定问候语，供脚手架冒烟测试。
pub fn placeholder_greeting() -> &'static str {
    "wingetui"
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn placeholder_workspace_smoke() {
        assert_eq!(placeholder_greeting(), "wingetui");
    }
}
