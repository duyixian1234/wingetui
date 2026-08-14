//! mock winget 二进制：模拟 winget CLI 的查询/变更行为，供集成测试使用。
//!
//! 行为约定（脱敏 fixture，见 `crates/winget/tests/fixtures/`）：
//! - `search --query <q>`：`__error__` → stderr + 退出码 1；`__notfound__` → 空表（无数据行）；
//!   `__malformed__` → 无表头文本；`__gbk__` → **GBK 编码**的表格字节；
//!   其余 → 正常文本表格 search.txt
//! - `upgrade`（无 --id/--all）→ upgradeable.txt（文本表格）
//! - `upgrade --all` / `upgrade --id <id>` / `install --id <id>` / `uninstall --id <id>`：
//!   `__error__` 触发 stderr 非零退出；其余输出若干进度行后成功退出
//! - `list` → installed.txt（文本表格）
//!
//! 查询类校验：必须携带 `--disable-interactivity --accept-source-agreements`，
//! **不得携带 `--output json`**（真实 winget v1.29.280 不支持，根因回归防护）。
//!
//! 附加：若设置环境变量 `MOCK_WINGET_LOG`，退出前将完整 argv（每行一个参数）
//! 写入该文件，供测试断言命令构造无 shell 拼接。

use std::env;
use std::io::Write;
use std::process::ExitCode;

const FIXTURE_SEARCH: &str = include_str!("../../../crates/winget/tests/fixtures/search.txt");
const FIXTURE_SEARCH_EMPTY: &str =
    include_str!("../../../crates/winget/tests/fixtures/search-empty.txt");
const FIXTURE_UPGRADEABLE: &str =
    include_str!("../../../crates/winget/tests/fixtures/upgradeable.txt");
const FIXTURE_INSTALLED: &str =
    include_str!("../../../crates/winget/tests/fixtures/installed.txt");
const FIXTURE_MALFORMED: &str =
    include_str!("../../../crates/winget/tests/fixtures/malformed.txt");
/// GBK 编码的文本表格（表头 名称/版本/匹配/源 为 GBK 字节）。
const FIXTURE_SEARCH_GBK: &[u8] = include_bytes!("../../../crates/winget/tests/fixtures/search-gbk.txt");

fn arg_after<'a>(args: &'a [String], flag: &str) -> Option<&'a str> {
    args.iter()
        .position(|a| a == flag)
        .and_then(|i| args.get(i + 1))
        .map(String::as_str)
}

/// 查询类命令必须携带的固定 flags（与规格 §4.1 一致，且**不得含 --output json**）。
fn has_query_flags(args: &[String]) -> bool {
    args.contains(&"--disable-interactivity".to_string())
        && args.contains(&"--accept-source-agreements".to_string())
        && !args.contains(&"--output".to_string())
        && !args.contains(&"json".to_string())
}

fn main() -> ExitCode {
    let args: Vec<String> = env::args().skip(1).collect();

    // 将完整 argv 以 stderr 前缀行输出（变更类会经 log_sink 进入日志区），
    // 供测试断言命令构造无 shell 拼接；参数以 \u{1f} 分隔。
    eprintln!("MOCK_ARGV:{}", args.join("\u{1f}"));
    if let Ok(log_path) = env::var("MOCK_WINGET_LOG") {
        let _ = std::fs::File::create(log_path)
            .and_then(|mut f| f.write_all(args.join("\n").as_bytes()));
    }

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
                    print!("{FIXTURE_SEARCH_EMPTY}");
                    ExitCode::SUCCESS
                }
                "__malformed__" => {
                    print!("{FIXTURE_MALFORMED}");
                    ExitCode::SUCCESS
                }
                "__gbk__" => {
                    // 输出 GBK 编码字节（中文环境实测行为），验证 UTF-8→GBK 解码回退
                    let _ = std::io::stdout().write_all(FIXTURE_SEARCH_GBK);
                    ExitCode::SUCCESS
                }
                _ => {
                    print!("{FIXTURE_SEARCH}");
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
                print!("{FIXTURE_UPGRADEABLE}");
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
            print!("{FIXTURE_INSTALLED}");
            ExitCode::SUCCESS
        }
        _ => {
            eprintln!("mock-winget: unknown subcommand {sub}");
            ExitCode::from(2)
        }
    }
}
