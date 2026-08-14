//! mock winget 二进制：模拟 winget CLI 的查询/变更行为，供集成测试使用。
//!
//! 行为约定（脱敏 fixture，见 `crates/winget/tests/fixtures/`）：
//! - `search --query <q>`：`__error__` → stderr + 退出码 1；`__notfound__` → 空结果；
//!   `__malformed__` → 畸形 JSON；其余 → 正常 search.json
//! - `upgrade`（无 --id/--all）→ upgradeable.json
//! - `upgrade --all` / `upgrade --id <id>` / `install --id <id>` / `uninstall --id <id>`：
//!   `__error__` 触发 stderr 非零退出；其余输出若干进度行后成功退出
//! - `list` → installed.json
//!
//! 附加：若设置环境变量 `MOCK_WINGET_LOG`，退出前将完整 argv（每行一个参数）
//! 写入该文件，供测试断言命令构造无 shell 拼接。

use std::env;
use std::process::ExitCode;

const FIXTURE_SEARCH: &str = include_str!("../../../crates/winget/tests/fixtures/search.json");
const FIXTURE_SEARCH_EMPTY: &str =
    include_str!("../../../crates/winget/tests/fixtures/search-empty.json");
const FIXTURE_UPGRADEABLE: &str =
    include_str!("../../../crates/winget/tests/fixtures/upgradeable.json");
const FIXTURE_INSTALLED: &str =
    include_str!("../../../crates/winget/tests/fixtures/installed.json");
const FIXTURE_MALFORMED: &str =
    include_str!("../../../crates/winget/tests/fixtures/malformed.json");

fn arg_after<'a>(args: &'a [String], flag: &str) -> Option<&'a str> {
    args.iter()
        .position(|a| a == flag)
        .and_then(|i| args.get(i + 1))
        .map(String::as_str)
}

/// 查询类命令必须携带的固定 flags（与 specs §4.3 一致）。
fn has_query_flags(args: &[String]) -> bool {
    args.contains(&"--output".to_string())
        && args.contains(&"json".to_string())
        && args.contains(&"--disable-interactivity".to_string())
        && args.contains(&"--accept-source-agreements".to_string())
}

fn main() -> ExitCode {
    let args: Vec<String> = env::args().skip(1).collect();

    // 将完整 argv 以 stderr 前缀行输出（变更类会经 log_sink 进入日志区），
    // 供测试断言命令构造无 shell 拼接；参数以 \u{1f} 分隔。
    eprintln!("MOCK_ARGV:{}", args.join("\u{1f}"));

    let Some(sub) = args.first().map(String::as_str) else {
        eprintln!("mock-winget: missing subcommand");
        return ExitCode::from(2);
    };

    match sub {
        "search" => {
            if !has_query_flags(&args) {
                eprintln!("mock-winget: missing query flags for search");
                return ExitCode::from(3);
            }
            let query = arg_after(&args, "--query").unwrap_or("");
            match query {
                "__error__" => {
                    eprintln!("mock-winget: simulated search failure");
                    ExitCode::FAILURE
                }
                "__notfound__" => {
                    println!("{FIXTURE_SEARCH_EMPTY}");
                    ExitCode::SUCCESS
                }
                "__malformed__" => {
                    println!("{FIXTURE_MALFORMED}");
                    ExitCode::SUCCESS
                }
                _ => {
                    println!("{FIXTURE_SEARCH}");
                    ExitCode::SUCCESS
                }
            }
        }
        "upgrade" => {
            let has_id = args.iter().any(|a| a == "--id");
            let has_all = args.iter().any(|a| a == "--all");
            if !has_id && !has_all && !has_query_flags(&args) {
                eprintln!("mock-winget: missing query flags for upgrade list");
                return ExitCode::from(3);
            }
            if has_id {
                let id = arg_after(&args, "--id").unwrap_or("");
                if id == "__error__" {
                    eprintln!("mock-winget: simulated upgrade failure");
                    ExitCode::FAILURE
                } else {
                    println!("mock-winget: upgrading {id}...");
                    println!("mock-winget: download started");
                    println!("mock-winget: installed {id}");
                    ExitCode::SUCCESS
                }
            } else if has_all {
                println!("mock-winget: upgrading all packages...");
                println!("mock-winget: all packages up to date");
                ExitCode::SUCCESS
            } else {
                // 列表查询
                println!("{FIXTURE_UPGRADEABLE}");
                ExitCode::SUCCESS
            }
        }
        "install" => {
            let id = arg_after(&args, "--id").unwrap_or("");
            if id == "__error__" {
                eprintln!("mock-winget: simulated install failure");
                ExitCode::FAILURE
            } else {
                println!("mock-winget: installing {id}...");
                println!("mock-winget: installed {id}");
                ExitCode::SUCCESS
            }
        }
        "uninstall" => {
            let id = arg_after(&args, "--id").unwrap_or("");
            if id == "__error__" {
                eprintln!("mock-winget: simulated uninstall failure");
                ExitCode::FAILURE
            } else {
                println!("mock-winget: uninstalling {id}...");
                println!("mock-winget: uninstalled {id}");
                ExitCode::SUCCESS
            }
        }
        "list" => {
            if !has_query_flags(&args) {
                eprintln!("mock-winget: missing query flags for list");
                return ExitCode::from(3);
            }
            println!("{FIXTURE_INSTALLED}");
            ExitCode::SUCCESS
        }
        _ => {
            eprintln!("mock-winget: unknown subcommand {sub}");
            ExitCode::from(2)
        }
    }
}
