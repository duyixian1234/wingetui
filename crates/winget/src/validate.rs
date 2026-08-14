//! 输入校验。

use crate::error::WingetError;

/// 包名/搜索词输入校验规则（TUI 层与 winget 层共用）：
/// - 拒空串 / 纯空白
/// - 拒控制字符
/// - 拒超过 200 字符
pub fn validate_package_input(s: &str) -> Result<(), WingetError> {
    let trimmed = s.trim();
    if trimmed.is_empty() {
        return Err(WingetError::Validation("输入为空".to_string()));
    }
    if trimmed.chars().count() > 200 {
        return Err(WingetError::Validation("输入超过 200 字符".to_string()));
    }
    if trimmed.chars().any(|c| c.is_control()) {
        return Err(WingetError::Validation("输入包含控制字符".to_string()));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_string_rejected() {
        assert!(matches!(
            validate_package_input(""),
            Err(WingetError::Validation(_))
        ));
    }

    #[test]
    fn whitespace_only_rejected() {
        assert!(matches!(
            validate_package_input("   \t  "),
            Err(WingetError::Validation(_))
        ));
    }

    #[test]
    fn control_char_rejected() {
        assert!(matches!(
            validate_package_input("Micro\nsoft"),
            Err(WingetError::Validation(_))
        ));
        assert!(matches!(
            validate_package_input("a\u{0000}b"),
            Err(WingetError::Validation(_))
        ));
    }

    #[test]
    fn over_200_chars_rejected() {
        let long = "a".repeat(201);
        assert!(matches!(
            validate_package_input(&long),
            Err(WingetError::Validation(_))
        ));
    }

    #[test]
    fn exactly_200_chars_accepted() {
        let s = "a".repeat(200);
        assert!(validate_package_input(&s).is_ok());
    }

    #[test]
    fn normal_id_accepted() {
        assert!(validate_package_input("Microsoft.PowerShell").is_ok());
        assert!(validate_package_input("Git.Git").is_ok());
    }

    #[test]
    fn surrounding_whitespace_stripped() {
        // 前后空白应被忽略，核心内容合法则通过
        assert!(validate_package_input("  Microsoft.PowerShell  ").is_ok());
    }
}
