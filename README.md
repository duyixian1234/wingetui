# wingetui

管理 [winget](https://learn.microsoft.com/windows/package-manager/winget/) 的 TUI 工具。

技术栈参考 [gitui](https://github.com/gitui-org/gitui)：Rust + [ratatui](https://ratatui.rs/) + [crossterm](https://github.com/crossterm-rs/crossterm) + [tokio](https://tokio.rs/)。winget CLI 经 subprocess 调用，查询类解析 JSON 输出，变更类非交互执行。

## 平台

Windows only（winget 依赖），见 [ADR-0004](docs/adr/0004-windows-only.md)。

## 构建

```powershell
cargo build --release
```

## 运行

```powershell
cargo run
```

MVP 功能：

- 搜索包（`winget search`）
- 查看可升级 / 升级（`winget upgrade`）
- 安装 / 卸载（`winget install` / `winget uninstall`）

## 质量门禁

本地全量验收（四绿）：

```powershell
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all
python scripts/check-links.py
```

## 文档

- [需求上下文](docs/context.md)
- [架构决策记录](docs/adr/)
- [MVP 规格](specs/mvp.md)
