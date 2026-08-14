# AGENTS.md — wingetui

wingetui：管理 winget 的 TUI 工具。Rust + ratatui + crossterm + tokio，技术栈参考 [gitui](https://github.com/gitui-org/gitui)。

> 本文件只维护**基本原则**。具体工作流、测试规范、提交规范、安全规范、部署手册见 [`.rules/`](.rules/) 目录。

## 基本原则

1. **平台**：Windows only（winget 依赖），见 [ADR-0004](docs/adr/0004-windows-only.md)。非 Windows 环境不构建。
2. **技术栈**：Rust（cargo 构建）、ratatui + crossterm（TUI）、tokio（异步）、serde（winget JSON 解析）。决策记录见 [docs/adr/](docs/adr/)，需求上下文见 [docs/context.md](docs/context.md)。
3. **仓库形态**：GitHub 公开仓库 `wingetui`，`main` 为集成分支，PR + CI 全绿后合并，见 [ADR-0003](docs/adr/0003-public-repo.md)。
4. **winget 交互**：一律 subprocess 调用 winget CLI，查询类用 `--output json` 解析，变更类非交互执行，见 [ADR-0002](docs/adr/0002-winget-subprocess.md)。
5. **质量门禁（全量验收）**：`cargo fmt --check`、`cargo clippy -- -D warnings`、`cargo test`、`python scripts/check-links.py`（文档链接）四项全绿方可合入 `main`。
6. **文档治理**：任何编辑过的 Markdown 必须运行 `python scripts/check-links.py`，exit 0 才算通过；禁止绝对路径链接（如 `D:\...`、`/d/...`）。
7. **依赖管理**：严格 cargo 管理，禁止全局安装混用；新增依赖须同步更新 [docs/context.md](docs/context.md) 依赖表并说明用途。

## 流程速查（详见 .rules/）

| 主题 | 文档 |
|------|------|
| 开发工作流（SOP） | [`.rules/workflows.md`](.rules/workflows.md) |
| 测试规范 | [`.rules/testing.md`](.rules/testing.md) |
| 提交规范（Conventional Commits） | [`.rules/committing.md`](.rules/committing.md) |
| 安全规范 | [`.rules/security.md`](.rules/security.md) |
| 部署/发布手册 | [`.rules/deploy.md`](.rules/deploy.md) |

## 项目结构

```
wingetui/
├── Cargo.toml          # workspace 根
├── src/                # TUI 主程序
├── crates/
│   └── winget/         # winget CLI 交互层（独立 crate，仿 gitui asyncgit）
├── docs/
│   ├── context.md      # 需求上下文
│   └── adr/            # 架构决策记录
├── .rules/             # 治理细则
├── scripts/
│   └── check-links.py  # Markdown 链接校验
└── .github/workflows/  # CI
```

> 结构以规格与工单为准，此处为概览；如有出入以实际实现为准。
>
> 实现前先读 [`specs/mvp.md`](specs/mvp.md)：块目标、接口契约、验收标准、块依赖；工单编号见其 §8。
