# Context — wingetui

> 需求治理文档 · 主理人：闻道明 · 2026-08-14 · 状态：已确认

## 1. 背景

用户在 Windows 环境频繁使用 winget（Windows Package Manager）管理软件包，命令行交互体验单一。希望拥有一个 **TUI（终端用户界面）工具**，以交互式界面完成 winget 的常见操作，提升效率与可读性。

技术栈参考 [gitui](https://github.com/gitui-org/gitui)（Rust + ratatui + crossterm 的 TUI 项目），但异步方案按用户决策改用 tokio（见 [ADR-0001](adr/0001-tokio-async.md)）。

## 2. 范围

### 2.1 本次交付（MVP）

1. **工程脚手架**：Cargo 工程、质量门禁（rustfmt / clippy / cargo-deny / typos）、GitHub Actions CI、文档治理骨架（AGENTS.md + .rules/）
2. **核心 TUI 流程**：
   - **搜索包**：`winget search` 交互式搜索，结果列表展示
   - **查看可升级**：`winget upgrade` 列出可升级包
   - **升级**：升级指定包 / 全部可升级
   - **安装**：安装指定包
   - **卸载**：卸载已安装包
3. **winget 交互层**：subprocess 调用 winget CLI，JSON 输出解析（见 [ADR-0002](adr/0002-winget-subprocess.md)）
4. **异步**：tokio 运行时 + 后台任务，UI 不阻塞（见 [ADR-0001](adr/0001-tokio-async.md)）

### 2.2 非目标（MVP 明确不做）

- 包详情页（仓库、许可证、版本历史等深度信息）
- 批量多选操作
- 可配置键位 / 主题系统
- winget 配置（`winget configure`）、导入导出（`winget import/export`）
- 非 Windows 平台支持（winget 仅 Windows，见 [ADR-0004](adr/0004-windows-only.md)）

## 3. 术语

| 术语 | 含义 |
|------|------|
| winget | Windows Package Manager，Windows 10/11 自带包管理器 CLI |
| TUI | Terminal User Interface，终端交互界面 |
| ratatui | Rust TUI 框架（gitui 同款） |
| crossterm | Rust 终端控制/事件库（ratatui 配套） |
| Package / 包 | winget 可管理的软件单元，以 `Id`（如 `Microsoft.PowerShell`）唯一标识 |
| 可升级（upgradeable） | 本机已安装且存在更新版本的包 |

## 4. 约束

| 约束 | 说明 |
|------|------|
| 平台 | Windows only（winget 依赖），见 [ADR-0004](adr/0004-windows-only.md) |
| 语言/工具链 | Rust（MSRV 遵循 ratatui 要求），cargo 构建 |
| 依赖管理 | cargo，禁止全局安装混用 |
| 仓库形态 | GitHub 公开仓库 `wingetui`，`main` 为集成分支（见 [ADR-0003](adr/0003-public-repo.md)） |
| winget 版本 | 本机 v1.29.280，需兼容 `--output json` 输出格式 |
| 文档治理 | AGENTS.md 只放基本原则，细则放 `.rules/`；所有 Markdown 须过 `scripts/check-links.py` |

## 5. 质量要求

1. `cargo fmt --check` 通过
2. `cargo clippy -- -D warnings` 通过
3. `cargo test` 全量通过（核心逻辑：winget 输出解析、状态机）
4. `cargo deny` 依赖审计通过（若配置）
5. `python scripts/check-links.py` exit 0
6. CI（GitHub Actions）在 `main` 上全绿

## 6. 关键外部依赖（需确认）

| 依赖 | 用途 | 状态 |
|------|------|------|
| `ratatui` | TUI 框架 | 需定版本（当前主流 ~0.29） |
| `crossterm` | 终端事件 | 需定版本 |
| `tokio` | 异步运行时 | 需定版本 |
| `serde` / `serde_json` | winget JSON 解析 | 需定版本 |
| `anyhow` | 错误处理 | 需定版本 |
| `fuzzy-matcher` | 搜索过滤（可选） | 需评估 |

## 7. 参考

- gitui 仓库：<https://github.com/gitui-org/gitui>
- winget CLI 文档：<https://learn.microsoft.com/windows/package-manager/winget/>
