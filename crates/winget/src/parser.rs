//! winget 文本表格输出解析。
//!
//! winget v1.29.280 的查询类子命令（search / upgrade / list）**不支持 `--output json`**，
//! 输出为**对齐文本表格**（表头列：`名称 ID 版本 匹配(或 可用) 源`）。本模块：
//!
//! - [`decode_winget_output`]：stdout 字节流先试 UTF-8，失败回退 GBK（`encoding_rs`）。
//! - [`parse_packages_text`]：表头定位列边界，数据行按**显示宽度**切分列（CJK 占 2 列，
//!   与 winget 的表格对齐方式一致），按表头 token 映射 `Package` 字段。
//!
//! 容错策略：畸形行（分隔线 / 状态行 / ID 列缺失）跳过降级；无数据行 → 空 vec；
//! 整体无法定位表头 → `Parse`。

use encoding_rs::GBK;
use unicode_width::UnicodeWidthChar;

use crate::error::WingetError;
use crate::models::Package;

/// 解码查询命令 stdout 字节流：先试 UTF-8，失败回退 GBK（encoding_rs）。
///
/// winget 在管道（subprocess）下通常输出 UTF-8；中文控制台/部分版本可能输出 GBK，
/// 二者都须兼容。UTF-8 与 GBK 都失败时用 lossy 兜底，保证不 panic。
pub fn decode_winget_output(bytes: &[u8]) -> String {
    match std::str::from_utf8(bytes) {
        Ok(s) => s.to_string(),
        Err(_) => {
            let (decoded, _, had_errors) = GBK.decode(bytes);
            if had_errors {
                String::from_utf8_lossy(bytes).into_owned()
            } else {
                decoded.into_owned()
            }
        }
    }
}

/// 单个字符的显示宽度（CJK 全角 = 2，其余 = 1）。
fn char_display_width(ch: char) -> usize {
    UnicodeWidthChar::width(ch).unwrap_or(0)
}

/// 表头列布局：每个表头 token 的显示宽度边界 + 字段映射。
struct TableColumns {
    /// 表头所在行号（`output.lines()` 中的索引）。
    header_line_index: usize,
    /// 每个表头列的 `(显示起始列, 显示结束列)`，最后一列为 `None`（延伸到行尾）。
    bounds: Vec<(usize, Option<usize>)>,
    /// 字段对应的列下标（在 `bounds` 中）。
    name_col: usize,
    id_col: usize,
    version_col: usize,
    available_col: Option<usize>,
    source_col: Option<usize>,
}

/// 逐行扫描定位表头行，并解析列布局。
///
/// 表头判定（规格 §4.3 规则 1）：按空白切分后的 token 集合**含 `ID`**，
/// 且含 `名称`/`Name` 与 `版本`/`Version` 至少一个。
/// 找不到 → `WingetError::Parse`。
fn locate_header(output: &str) -> Result<TableColumns, WingetError> {
    for (line_index, line) in output.lines().enumerate() {
        let tokens: Vec<&str> = line.split_whitespace().collect();
        let has_id = tokens.contains(&"ID");
        let has_name = tokens.contains(&"名称") || tokens.contains(&"Name");
        let has_version = tokens.contains(&"版本") || tokens.contains(&"Version");
        if has_id && has_name && has_version {
            return Ok(header_columns(line_index, line, &tokens));
        }
    }
    Err(WingetError::Parse(
        "无法定位表头行（缺少 ID/名称/版本 列）".to_string(),
    ))
}

/// 由表头行计算列边界与字段映射。
fn header_columns(line_index: usize, header: &str, tokens: &[&str]) -> TableColumns {
    // 逐 token 计算显示起始列：累计空白与 token 的显示宽度。
    let mut starts = Vec::with_capacity(tokens.len());
    let mut acc = 0usize;
    let mut iter = header.char_indices().peekable();
    for _ in tokens {
        // 跳过 token 前的空白
        while let Some(&(_, ch)) = iter.peek() {
            if ch.is_whitespace() {
                acc += char_display_width(ch);
                iter.next();
            } else {
                break;
            }
        }
        starts.push(acc);
        // 跳过 token 内容
        while let Some(&(_, ch)) = iter.peek() {
            if !ch.is_whitespace() {
                acc += char_display_width(ch);
                iter.next();
            } else {
                break;
            }
        }
    }

    let mut bounds = Vec::with_capacity(starts.len());
    let mut name_col = 0;
    let mut id_col = 0;
    let mut version_col = 0;
    let mut available_col = None;
    let mut source_col = None;

    for (i, tok) in tokens.iter().enumerate() {
        let end = starts.get(i + 1).copied();
        bounds.push((starts[i], end));
        match *tok {
            "名称" | "Name" => name_col = i,
            "ID" => id_col = i,
            "版本" | "Version" => version_col = i,
            "可用" | "Available" => available_col = Some(i),
            "源" | "Source" => source_col = Some(i),
            // "匹配"/"Match" 忽略（无模型字段）
            _ => {}
        }
    }

    TableColumns {
        header_line_index: line_index,
        bounds,
        name_col,
        id_col,
        version_col,
        available_col,
        source_col,
    }
}

/// 按显示宽度切分一行，返回指定列的切片（已 `trim`）。
///
/// 列边界来自表头（显示列位置）；数据行与表头按同一显示宽度对齐。
/// 行内容不足该列时返回空字符串。
fn slice_cell(line: &str, start_disp: usize, end_disp: Option<usize>) -> &str {
    let start = {
        let mut acc = 0usize;
        let mut found = None;
        for (i, ch) in line.char_indices() {
            if acc >= start_disp {
                found = Some(i);
                break;
            }
            acc += char_display_width(ch);
        }
        found.unwrap_or(line.len())
    };
    if start == line.len() {
        return "";
    }
    let end = match end_disp {
        None => line.len(),
        Some(end_disp) => {
            let width = end_disp.saturating_sub(start_disp);
            let mut acc = 0usize;
            let mut found = line.len();
            for (i, ch) in line[start..].char_indices() {
                if acc >= width {
                    found = start + i;
                    break;
                }
                acc += char_display_width(ch);
            }
            found
        }
    };
    line[start..end].trim()
}

/// 是否状态行（winget 表格外的提示文本，如"找到 N 个匹配项"）。
/// 真实 winget 在无匹配时会输出这类文本，须跳过（规格 §4.3 规则 4）。
fn is_status_line(line: &str) -> bool {
    const STATUS_MARKERS: [&str; 3] = ["找到", "正在", "个匹配项"];
    STATUS_MARKERS.iter().any(|m| line.contains(m))
}

/// 从文本表格解析包列表（规格 §4.3）。
///
/// - 表头定位列边界（兼容中文/英文表头；列顺序不敏感；列宽变化不敏感）。
/// - 数据行按显示宽度切分并 `trim`。
/// - 畸形行（分隔线 / 状态行 / ID 列为空）跳过降级。
/// - 无数据行 → `Ok(vec![])`（由调用方映射为 `NotFound`）。
/// - 整体无法定位表头 → `Parse`。
pub fn parse_packages_text(output: &str) -> Result<Vec<Package>, WingetError> {
    let columns = locate_header(output)?;

    let mut packages = Vec::new();
    for line in output.lines().skip(columns.header_line_index + 1) {
        let trimmed = line.trim();
        // 空行 / 纯 `-` 分隔线
        if trimmed.is_empty() || trimmed.chars().all(|c| c == '-') {
            continue;
        }
        // 状态行（无匹配提示等）
        if is_status_line(trimmed) {
            continue;
        }

        let cells: Vec<&str> = columns
            .bounds
            .iter()
            .map(|&(s, e)| slice_cell(line, s, e))
            .collect();

        // ID 列为空 → 畸形行跳过（状态行/无 ID 行也在此兜底）
        let id = cells[columns.id_col];
        if id.is_empty() {
            continue;
        }

        packages.push(Package {
            id: id.to_string(),
            name: cells[columns.name_col].to_string(),
            version: cells[columns.version_col].to_string(),
            available_version: columns
                .available_col
                .map(|i| cells[i].to_string())
                .filter(|s| !s.is_empty()),
            source: columns
                .source_col
                .map(|i| cells[i].to_string())
                .filter(|s| !s.is_empty()),
        });
    }
    Ok(packages)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 构造与 winget 对齐方式一致的文本表格（列宽 = max(表头, 数据) 显示宽度 + 2 空格）。
    /// 与真实 winget 布局同构，避免手写对齐出错。
    fn table(header: &[&str], rows: &[Vec<&str>]) -> String {
        let ncols = header.len();
        let mut widths: Vec<usize> = header
            .iter()
            .map(|h| h.chars().map(char_display_width).sum())
            .collect();
        for row in rows {
            for (c, cell) in row.iter().enumerate() {
                widths[c] = widths[c].max(cell.chars().map(char_display_width).sum());
            }
        }
        let mut out = String::new();
        for (c, h) in header.iter().enumerate() {
            out.push_str(h);
            if c + 1 < ncols {
                out.push_str(
                    &" ".repeat(widths[c] - h.chars().map(char_display_width).sum::<usize>() + 2),
                );
            }
        }
        let header_width: usize = out.chars().map(char_display_width).sum();
        out.push_str("\r\n");
        out.push_str(&"-".repeat(header_width));
        out.push_str("\r\n");
        for row in rows {
            for (c, cell) in row.iter().enumerate() {
                out.push_str(cell);
                if c + 1 < ncols {
                    out.push_str(&" ".repeat(
                        widths[c] - cell.chars().map(char_display_width).sum::<usize>() + 2,
                    ));
                }
            }
            out.push_str("\r\n");
        }
        out
    }

    #[test]
    fn parses_chinese_search_header_ignores_match_column() {
        let table = table(
            &["名称", "ID", "版本", "匹配", "源"],
            &[
                vec!["PowerShell", "Microsoft.PowerShell", "7.4.5", "", "winget"],
                vec!["Git", "Git.Git", "2.45.1", "Tag: git", "winget"],
                vec![
                    "Visual Studio Code",
                    "Microsoft.VisualStudioCode",
                    "1.90.2",
                    "",
                    "winget",
                ],
            ],
        );
        let pkgs = parse_packages_text(&table).expect("should parse");
        assert_eq!(pkgs.len(), 3);
        assert_eq!(pkgs[0].id, "Microsoft.PowerShell");
        assert_eq!(pkgs[0].name, "PowerShell");
        assert_eq!(pkgs[0].version, "7.4.5");
        assert_eq!(pkgs[0].source.as_deref(), Some("winget"));
        assert_eq!(pkgs[0].available_version, None, "search 无可用列值");
        // 含空格字段：名称 "Visual Studio Code" 与匹配列 "Tag: git"
        assert_eq!(pkgs[2].name, "Visual Studio Code");
        assert_eq!(pkgs[1].id, "Git.Git");
    }

    #[test]
    fn parses_english_search_header() {
        let table = table(
            &["Name", "ID", "Version", "Match", "Source"],
            &[vec![
                "PowerShell",
                "Microsoft.PowerShell",
                "7.4.5",
                "",
                "winget",
            ]],
        );
        let pkgs = parse_packages_text(&table).expect("should parse");
        assert_eq!(pkgs.len(), 1);
        assert_eq!(pkgs[0].id, "Microsoft.PowerShell");
        assert_eq!(pkgs[0].name, "PowerShell");
        assert_eq!(pkgs[0].version, "7.4.5");
        assert_eq!(pkgs[0].source.as_deref(), Some("winget"));
    }

    #[test]
    fn parses_upgradeable_maps_available_column() {
        let table = table(
            &["名称", "ID", "版本", "可用", "源"],
            &[
                vec!["Git", "Git.Git", "2.45.1", "2.46.0", "winget"],
                vec![
                    "PowerShell",
                    "Microsoft.PowerShell",
                    "7.4.5",
                    "7.5.0",
                    "winget",
                ],
            ],
        );
        let pkgs = parse_packages_text(&table).expect("should parse");
        assert_eq!(pkgs.len(), 2);
        assert_eq!(pkgs[0].id, "Git.Git");
        assert_eq!(pkgs[0].available_version.as_deref(), Some("2.46.0"));
        assert_eq!(pkgs[1].id, "Microsoft.PowerShell");
        assert_eq!(pkgs[1].available_version.as_deref(), Some("7.5.0"));
    }

    #[test]
    fn parses_english_available_column() {
        let table = table(
            &["Name", "ID", "Version", "Available", "Source"],
            &[vec!["Git", "Git.Git", "2.45.1", "2.46.0", "winget"]],
        );
        let pkgs = parse_packages_text(&table).expect("should parse");
        assert_eq!(pkgs.len(), 1);
        assert_eq!(pkgs[0].available_version.as_deref(), Some("2.46.0"));
    }

    #[test]
    fn parses_row_with_cjk_name_using_display_width() {
        // 名称含 CJK（"版本"二字），列边界须按显示宽度切分才能取对版本/可用列
        let table = table(
            &["名称", "ID", "版本", "可用", "源"],
            &[vec![
                "FlClash 版本 0.8.94+2026071102",
                "chen08209.FlClash",
                "0.8.94+2026071102",
                "0.8.95+2026081401",
                "winget",
            ]],
        );
        let pkgs = parse_packages_text(&table).expect("should parse");
        assert_eq!(pkgs.len(), 1);
        assert_eq!(pkgs[0].name, "FlClash 版本 0.8.94+2026071102");
        assert_eq!(pkgs[0].id, "chen08209.FlClash");
        assert_eq!(pkgs[0].version, "0.8.94+2026071102");
        assert_eq!(
            pkgs[0].available_version.as_deref(),
            Some("0.8.95+2026081401")
        );
        assert_eq!(pkgs[0].source.as_deref(), Some("winget"));
    }

    #[test]
    fn empty_result_is_empty_vec() {
        // 无数据行：表头 + 分隔线 + 状态行（"未找到..."，含"找到"关键词 → 跳过）
        let table = table(&["名称", "ID", "版本", "匹配", "源"], &[]);
        let table = format!("{table}未找到与输入条件匹配的程序包。\r\n");
        let pkgs = parse_packages_text(&table).expect("empty table should parse");
        assert!(pkgs.is_empty());
    }

    #[test]
    fn no_header_is_parse_error() {
        let err = parse_packages_text("这不是一个表格\r\n没有任何表头行\r\n").unwrap_err();
        assert!(matches!(err, WingetError::Parse(_)), "got {err:?}");
    }

    #[test]
    fn separator_and_status_lines_are_skipped() {
        let mut table = table(
            &["名称", "ID", "版本", "匹配", "源"],
            &[vec![
                "PowerShell",
                "Microsoft.PowerShell",
                "7.4.5",
                "",
                "winget",
            ]],
        );
        // 追加状态行与一个 ID 缺失的畸形行（短文本，落在名称列、ID 列空缺）
        table.push_str("找到 3 个匹配项\r\n");
        table.push_str("仅名称行\r\n");
        let pkgs = parse_packages_text(&table).expect("should parse");
        assert_eq!(pkgs.len(), 1, "分隔线/状态行/畸形行应被跳过");
    }

    #[test]
    fn column_order_insensitive() {
        // 列顺序打乱（ID 在前），仍能正确映射
        let table = table(
            &["ID", "名称", "版本", "源"],
            &[vec![
                "Microsoft.PowerShell",
                "PowerShell",
                "7.4.5",
                "winget",
            ]],
        );
        let pkgs = parse_packages_text(&table).expect("should parse");
        assert_eq!(pkgs.len(), 1);
        assert_eq!(pkgs[0].id, "Microsoft.PowerShell");
        assert_eq!(pkgs[0].name, "PowerShell");
        assert_eq!(pkgs[0].version, "7.4.5");
        assert_eq!(pkgs[0].source.as_deref(), Some("winget"));
    }

    #[test]
    fn missing_optional_columns_are_none() {
        // 无 可用/源 列 → available_version/source 为 None
        let table = table(
            &["名称", "ID", "版本"],
            &[vec!["PowerShell", "Microsoft.PowerShell", "7.4.5"]],
        );
        let pkgs = parse_packages_text(&table).expect("should parse");
        assert_eq!(pkgs.len(), 1);
        assert_eq!(pkgs[0].available_version, None);
        assert_eq!(pkgs[0].source, None);
    }

    // -----------------------------------------------------------------------
    // 解码单测
    // -----------------------------------------------------------------------

    #[test]
    fn decode_utf8_bytes() {
        let s = decode_winget_output("名称 ID".as_bytes());
        assert_eq!(s, "名称 ID");
    }

    #[test]
    fn decode_gbk_bytes() {
        // "名称" 的 GBK 编码：名=C3FB 称=B3C6
        let gbk = [0xc3u8, 0xfb, 0xb3, 0xc6];
        let s = decode_winget_output(&gbk);
        assert_eq!(s, "名称");
    }

    #[test]
    fn decode_invalid_bytes_falls_back_without_panic() {
        // 既非 UTF-8 也非合法 GBK 的字节 → lossy 兜底，不 panic
        let bad = [0xffu8, 0xfe, 0x00, 0x80];
        let s = decode_winget_output(&bad);
        assert!(!s.is_empty(), "lossy 兜底应产出非空字符串");
    }

    #[test]
    fn gbk_table_parses_after_decode() {
        // 真实 GBK fixture（表头 名称/版本/匹配/源 为 GBK 编码），
        // 解码后按文本表格解析出 2 个包。
        let table_gbk: &[u8] = include_bytes!("../tests/fixtures/search-gbk.txt");
        let text = decode_winget_output(table_gbk);
        let pkgs = parse_packages_text(&text).expect("GBK 表格应能解析");
        assert_eq!(pkgs.len(), 2);
        assert_eq!(pkgs[0].id, "Microsoft.PowerShell");
        assert_eq!(pkgs[0].name, "PowerShell");
        assert_eq!(pkgs[0].version, "7.4.5");
        assert_eq!(pkgs[1].id, "Git.Git");
        // 匹配列 "Tag: git" 被忽略，不映射任何字段
    }
}
