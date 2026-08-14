//! winget 命令构造。
//!
//! 参数一律经 `Command::arg` 数组传入，**禁止 shell 拼接**（[安全规范](../../../.rules/security.md)）。
//! 这里只构造 `Vec<String>` 参数列表，由 [`Winget`](crate::Winget) 组装成 `tokio::process::Command`。

/// 查询类命令固定追加参数（防首跑源协议弹窗挂起）。
///
/// 规格契约（`specs/bugfix-query-output.md` §4.1）：winget v1.29.280 的
/// search / list / upgrade 查询类子命令**不支持 `--output json`**（实测退出码
/// `0x8A150002`），因此查询类只保留非交互 flags，解析文本表格输出。
const QUERY_FLAGS: [&str; 2] = ["--disable-interactivity", "--accept-source-agreements"];

/// 变更类命令固定追加参数（非交互执行）。
const ACTION_FLAGS: [&str; 4] = [
    "--silent",
    "--accept-package-agreements",
    "--accept-source-agreements",
    "--disable-interactivity",
];

/// `winget search --query <q>` 参数（查询）。
pub fn search_args(query: &str) -> Vec<String> {
    let mut args = vec![
        "search".to_string(),
        "--query".to_string(),
        query.to_string(),
    ];
    args.extend(QUERY_FLAGS.iter().map(|s| s.to_string()));
    args
}

/// `winget upgrade`（列可升级）参数（查询）。
pub fn list_upgradeable_args() -> Vec<String> {
    let mut args = vec!["upgrade".to_string()];
    args.extend(QUERY_FLAGS.iter().map(|s| s.to_string()));
    args
}

/// `winget list`（列已安装）参数（查询）。
pub fn list_installed_args() -> Vec<String> {
    let mut args = vec!["list".to_string()];
    args.extend(QUERY_FLAGS.iter().map(|s| s.to_string()));
    args
}

/// `winget upgrade --id <id>` 参数（变更）。
pub fn upgrade_id_args(id: &str) -> Vec<String> {
    let mut args = vec!["upgrade".to_string(), "--id".to_string(), id.to_string()];
    args.extend(ACTION_FLAGS.iter().map(|s| s.to_string()));
    args
}

/// `winget upgrade --all` 参数（变更）。
pub fn upgrade_all_args() -> Vec<String> {
    let mut args = vec!["upgrade".to_string(), "--all".to_string()];
    args.extend(ACTION_FLAGS.iter().map(|s| s.to_string()));
    args
}

/// `winget install --id <id>` 参数（变更）。
pub fn install_args(id: &str) -> Vec<String> {
    let mut args = vec!["install".to_string(), "--id".to_string(), id.to_string()];
    args.extend(ACTION_FLAGS.iter().map(|s| s.to_string()));
    args
}

/// `winget uninstall --id <id>` 参数（变更）。
pub fn uninstall_args(id: &str) -> Vec<String> {
    let mut args = vec!["uninstall".to_string(), "--id".to_string(), id.to_string()];
    args.extend(ACTION_FLAGS.iter().map(|s| s.to_string()));
    args
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 断言查询参数与规格 §4.1 完全一致（无 `--output json`；无 shell 拼接：参数是独立数组元素）。
    #[test]
    fn search_argv_matches_spec() {
        assert_eq!(
            search_args("powershell"),
            vec![
                "search",
                "--query",
                "powershell",
                "--disable-interactivity",
                "--accept-source-agreements",
            ]
        );
    }

    #[test]
    fn list_upgradeable_argv_matches_spec() {
        assert_eq!(
            list_upgradeable_args(),
            vec![
                "upgrade",
                "--disable-interactivity",
                "--accept-source-agreements",
            ]
        );
    }

    #[test]
    fn list_installed_argv_matches_spec() {
        assert_eq!(
            list_installed_args(),
            vec![
                "list",
                "--disable-interactivity",
                "--accept-source-agreements",
            ]
        );
    }

    /// 查询类三个命令均不得包含 `--output json`（真实 winget 不支持，根因回归防护）。
    #[test]
    fn query_argv_has_no_output_json() {
        for args in [
            search_args("powershell"),
            list_upgradeable_args(),
            list_installed_args(),
        ] {
            assert!(
                !args.iter().any(|a| a == "--output" || a == "json"),
                "查询类参数不得含 --output json: {args:?}"
            );
        }
    }

    #[test]
    fn upgrade_id_argv_matches_spec() {
        assert_eq!(
            upgrade_id_args("Microsoft.PowerShell"),
            vec![
                "upgrade",
                "--id",
                "Microsoft.PowerShell",
                "--silent",
                "--accept-package-agreements",
                "--accept-source-agreements",
                "--disable-interactivity",
            ]
        );
    }

    #[test]
    fn upgrade_all_argv_matches_spec() {
        assert_eq!(
            upgrade_all_args(),
            vec![
                "upgrade",
                "--all",
                "--silent",
                "--accept-package-agreements",
                "--accept-source-agreements",
                "--disable-interactivity",
            ]
        );
    }

    #[test]
    fn install_argv_matches_spec() {
        assert_eq!(
            install_args("Git.Git"),
            vec![
                "install",
                "--id",
                "Git.Git",
                "--silent",
                "--accept-package-agreements",
                "--accept-source-agreements",
                "--disable-interactivity",
            ]
        );
    }

    #[test]
    fn uninstall_argv_matches_spec() {
        assert_eq!(
            uninstall_args("Git.Git"),
            vec![
                "uninstall",
                "--id",
                "Git.Git",
                "--silent",
                "--accept-package-agreements",
                "--accept-source-agreements",
                "--disable-interactivity",
            ]
        );
    }

    /// 无 shell 拼接：查询词即使含危险字符也作为独立参数，不出现 "cmd /c" / "&" 组合。
    #[test]
    fn no_shell_concatenation_for_hostile_query() {
        let args = search_args("x & del C:\\Windows & echo pwned");
        assert!(args.contains(&"x & del C:\\Windows & echo pwned".to_string()));
        assert!(!args.iter().any(|a| a.contains("cmd")));
    }
}
