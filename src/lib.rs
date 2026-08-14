//! wingetui 库入口（占位）。
//!
//! TUI 主循环 / 状态机将在后续块实现，这里先提供占位测试验证 workspace 可用。

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
