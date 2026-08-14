//! winget CLI 交互层（占位）。
//!
//! 数据模型、JSON 解析、命令构造、输入校验将在 Block 1 实现。

/// 占位函数：返回 crate 名，供脚手架冒烟测试。
pub fn crate_name() -> &'static str {
    "winget"
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn placeholder_crate_smoke() {
        assert_eq!(crate_name(), "winget");
    }
}
