//! winget `--output json` 输出解析。
//!
//! 容错策略：字段缺失降级为默认值；`Matches` 缺失视为空结果；
//! 只有整体无法解析（非 JSON / 顶层非对象）才返回 `Parse`。

use serde_json::Value;

use crate::error::WingetError;
use crate::models::Package;

/// 从 winget 查询类命令的 stdout 解析包列表。
///
/// - 正常 JSON → 正确字段映射
/// - 畸形 JSON（无法 parse）→ `Parse`
/// - `Matches` 字段缺失 → 空 vec（由调用方决定是否映射为 NotFound）
/// - 单个匹配项缺少 `Package` 字段 → 跳过该项（降级）
pub fn parse_packages(output: &str) -> Result<Vec<Package>, WingetError> {
    let root: Value = serde_json::from_str(output)
        .map_err(|e| WingetError::Parse(format!("无法解析 JSON: {e}")))?;

    let obj = root
        .as_object()
        .ok_or_else(|| WingetError::Parse("JSON 顶层不是对象".to_string()))?;

    let matches = match obj.get("Matches") {
        Some(Value::Array(arr)) => arr,
        Some(_) => return Err(WingetError::Parse("Matches 字段不是数组".to_string())),
        None => return Ok(Vec::new()),
    };

    let mut packages = Vec::with_capacity(matches.len());
    for item in matches {
        let Some(pkg_obj) = item.get("Package").and_then(|p| p.as_object()) else {
            // 单个匹配项无 Package 字段：降级跳过，不算整体失败
            continue;
        };
        packages.push(Package {
            id: str_field(pkg_obj, "Id").unwrap_or_default(),
            name: str_field(pkg_obj, "Name").unwrap_or_default(),
            version: str_field(pkg_obj, "Version").unwrap_or_default(),
            available_version: str_field(pkg_obj, "AvailableVersion"),
            source: str_field(pkg_obj, "Source"),
        });
    }
    Ok(packages)
}

/// 读取字符串字段；缺失或非字符串返回 `None`（调用方决定降级策略）。
fn str_field(obj: &serde_json::Map<String, Value>, key: &str) -> Option<String> {
    obj.get(key).and_then(|v| v.as_str()).map(str::to_string)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SEARCH_JSON: &str = r#"{
        "Source": "winget",
        "TotalMatches": 2,
        "Matches": [
            {
                "Package": {
                    "Id": "Microsoft.PowerShell",
                    "Name": "PowerShell",
                    "Version": "7.4.5",
                    "Source": "winget"
                }
            },
            {
                "Package": {
                    "Id": "Git.Git",
                    "Name": "Git",
                    "Version": "2.45.1",
                    "AvailableVersion": "2.46.0",
                    "Source": "winget"
                }
            }
        ]
    }"#;

    #[test]
    fn parses_normal_json() {
        let pkgs = parse_packages(SEARCH_JSON).expect("should parse");
        assert_eq!(pkgs.len(), 2);
        assert_eq!(pkgs[0].id, "Microsoft.PowerShell");
        assert_eq!(pkgs[0].name, "PowerShell");
        assert_eq!(pkgs[0].version, "7.4.5");
        assert_eq!(pkgs[0].available_version, None);
        assert_eq!(pkgs[0].source.as_deref(), Some("winget"));
        assert_eq!(pkgs[1].id, "Git.Git");
        assert_eq!(pkgs[1].available_version.as_deref(), Some("2.46.0"));
    }

    #[test]
    fn malformed_json_is_parse_error() {
        let err = parse_packages("{ not json").unwrap_err();
        assert!(matches!(err, WingetError::Parse(_)));
    }

    #[test]
    fn top_level_non_object_is_parse_error() {
        let err = parse_packages("[1,2,3]").unwrap_err();
        assert!(matches!(err, WingetError::Parse(_)));
    }

    #[test]
    fn matches_not_array_is_parse_error() {
        let err = parse_packages(r#"{"Matches": "oops"}"#).unwrap_err();
        assert!(matches!(err, WingetError::Parse(_)));
    }

    #[test]
    fn missing_matches_field_is_empty() {
        let pkgs = parse_packages(r#"{"Source": "winget"}"#).expect("should parse");
        assert!(pkgs.is_empty());
    }

    #[test]
    fn empty_matches_is_empty() {
        let pkgs = parse_packages(r#"{"Source": "winget", "Matches": []}"#).expect("should parse");
        assert!(pkgs.is_empty());
    }

    #[test]
    fn missing_package_field_is_skipped() {
        let json = r#"{
            "Matches": [
                {"Package": {"Id": "A.B", "Name": "A", "Version": "1"}},
                {"NotPackage": true},
                {"Package": {"Id": "C.D", "Name": "C", "Version": "2"}}
            ]
        }"#;
        let pkgs = parse_packages(json).expect("should parse");
        assert_eq!(pkgs.len(), 2);
        assert_eq!(pkgs[0].id, "A.B");
        assert_eq!(pkgs[1].id, "C.D");
    }

    #[test]
    fn missing_fields_degrade_to_defaults() {
        let json = r#"{
            "Matches": [
                {"Package": {"Id": "OnlyId", "Name": null, "Version": null}}
            ]
        }"#;
        let pkgs = parse_packages(json).expect("should parse");
        assert_eq!(pkgs[0].id, "OnlyId");
        assert_eq!(pkgs[0].name, "");
        assert_eq!(pkgs[0].version, "");
        assert_eq!(pkgs[0].available_version, None);
        assert_eq!(pkgs[0].source, None);
    }
}
