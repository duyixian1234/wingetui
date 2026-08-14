# ADR-0001: 异步方案采用 tokio

- 状态：已接受
- 日期：2026-08-14
- 决策人：杜逸先（用户）

## 上下文

gitui 采用自研轻量异步方案（`asyncgit` crate，线程池 + channel，无 tokio），以保持依赖轻量。wingetui 参考 gitui 技术栈，但用户明确决策：**异步运行时改用 tokio**。

## 决策

使用 tokio 作为异步运行时。winget CLI 的长耗时操作（search/install/upgrade/uninstall）通过 `tokio::process::Command` 异步执行，结果经 channel 回传 UI 线程，保证 TUI 交互不阻塞。

## 后果

- 正向：tokio 生态成熟、`tokio::process` 开箱即用、社区资料丰富；后续扩展（如并发批量操作）有原生支撑
- 负向：引入较重依赖（tokio full 特性）；二进制体积增大；与 gitui"零异步运行时"风格偏离
- 缓解：仅启用所需特性（`rt-multi-thread`、`process`、`sync`、`macros`、`time`），不用 full

## 参考

- [gitui 异步方案](https://github.com/gitui-org/gitui/tree/main/asyncgit)
- [tokio 官方文档](https://tokio.rs/)
