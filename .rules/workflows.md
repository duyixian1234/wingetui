# 工作流规范（.rules/workflows.md）

wingetui 的开发工作流。所有阶段流程遵循 mermaid 定义，改流程先改图再改文。

## 1. 完整交付 SOP

```mermaid
flowchart LR
    A[需求想法] --> B[Phase 1: grill-with-docs<br/>需求拷问]
    B --> C[Context + ADR 文档<br/>check-links 校验]
    C --> D[Phase 2: to-specs + to-tickets<br/>规格文档 + GitHub 工单]
    D --> E[Phase 3: implement<br/>按工单分块实现]
    E --> F[增量质检<br/>fmt/clippy/单测 仅限已编辑文件]
    F --> G[Phase 4: 全量验收<br/>fmt+clippy+test+check-links]
    G -->|通过| H[交付报告]
    G -->|未通过| E
```

## 2. 实现块质检流程（Phase 3 内部）

```mermaid
flowchart LR
    A[领取工单] --> B[实现该块]
    B --> C[格式化 cargo fmt]
    C --> D[静态检查 cargo clippy -D warnings]
    D --> E[针对性单测 cargo test 该模块]
    E --> F[提交 Conventional Commits]
    F --> G[回传主理人]
```

## 3. 全量验收流程（Phase 4）

```mermaid
flowchart LR
    A[全部工单完成] --> B[cargo fmt --check]
    B --> C[cargo clippy -- -D warnings]
    C --> D[cargo test 全量]
    D --> E[python scripts/check-links.py]
    E -->|全部通过| F[生成交付报告]
    E -->|任一失败| G[失败详情回传马奇成修复]
    G --> B
```

## 4. 角色分工

| 角色 | 成员 | 职责 |
|------|------|------|
| 主理人 | 闻道明 | Phase 1 拷问、Context/ADR、验收编排、交付报告 |
| 规格工程师 | 章定规 | Phase 2 to-specs / to-tickets |
| 实现工程师 | 马奇成 | Phase 3 按工单实现 |

## 5. 分支与合入

- 功能开发在 `feat/<工单号>` 分支，经 PR 合入 `main`
- PR 必须 CI 全绿（fmt + clippy + test + check-links）
- 禁止直接 push 到 `main`
