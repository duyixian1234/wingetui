# ADR-0002: winget 交互采用 subprocess 调用 CLI

- 状态：已接受
- 日期：2026-08-14
- 决策人：杜逸先（用户）

## 上下文

winget 没有官方公开的 SDK / API。管理 winget 有两种可行路径：

1. **subprocess 调用 winget CLI**：执行 `winget search/list/install/upgrade/uninstall`，解析输出
2. **直接操作 Windows 底层机制**（MSIX / Appx / registry / 安装器探测）：复杂、脆弱、无官方保障

用户决策：**subprocess 调用 winget CLI**。

## 决策

统一经 `tokio::process::Command` 调用 winget CLI：

- 查询类命令（`search` / `list` / `upgrade`）追加 `--output json`（winget v1.29 支持），解析 JSON 而非文本
- 变更类命令（`install` / `upgrade` / `uninstall`）使用 `--silent --accept-package-agreements --accept-source-agreements --disable-interactivity` 非交互执行，实时回传 stdout/stderr 行到 UI 日志区
- 命令执行失败（非零退出码）→ 解析 stderr 展示给用户

## 后果

- 正向：实现简单可靠、行为与 winget CLI 完全一致、自动跟随 winget 新版本能力
- 负向：依赖 winget 必须已安装（Windows 10/11 默认自带，约束见 [ADR-0004](0004-windows-only.md)）；JSON 输出格式随 winget 版本演进需保持兼容
- 缓解：解析层做容错（字段缺失降级）；CI 用 mock winget 二进制做输出解析单测

## 参考

- [winget --output 文档](https://learn.microsoft.com/windows/package-manager/winget/import)
