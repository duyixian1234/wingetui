# ADR-0004: 平台约束为 Windows only

- 状态：已接受
- 日期：2026-08-14
- 决策人：闻道明（主理人，基于事实约束）

## 上下文

winget 是 Windows 平台专属的包管理器（Windows 10 1709+ / Windows 11 内置）。gitui 跨平台（Linux/macOS/Windows），但 wingetui 的**核心功能对象**是 winget，非 Windows 环境无实际意义。

## 决策

- **仅支持 Windows**。非 Windows 平台编译或运行时给出明确错误提示（`#[cfg(not(windows))]` 下编译期报错或运行期友好退出）
- CI 的 build/test job 跑在 `windows-latest` runner 上
- 代码中不引入 `cfg(unix)` 分支逻辑，保持单一平台心智负担

## 后果

- 正向：实现路径清晰，无需维护多平台条件编译；CI 环境单一稳定
- 负向：无法在 macOS/Linux 上开发调试 UI；社区覆盖面窄
- 缓解：TUI 层与 winget 层解耦，若未来出现跨平台需求（如适配其他包管理器）可复用 UI 层

## 参考

- [winget 系统要求](https://learn.microsoft.com/windows/package-manager/winget/#install-winget)
