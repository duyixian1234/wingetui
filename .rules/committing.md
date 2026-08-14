# 提交规范（.rules/committing.md）

## 1. 格式

遵循 [Conventional Commits](https://www.conventionalcommits.org/zh-hans/)：

```
<type>(<scope>): <subject>
```

| type | 用途 |
|------|------|
| `feat` | 新功能 |
| `fix` | 缺陷修复 |
| `docs` | 文档变更 |
| `refactor` | 重构（不改行为） |
| `test` | 测试相关 |
| `ci` | CI 配置 |
| `chore` | 杂项（依赖、工具链） |

示例：`feat(winget): 解析 winget search JSON 输出`、`docs: 更新 Context 依赖表`

## 2. 提交原则

1. 一个提交一个逻辑变更，原子提交
2. 文档变更与代码变更同块提交时，须保持文档链接有效（跑 check-links）
3. 提交前 `cargo fmt`，提交后 CI 须全绿
4. 禁止 `--no-verify` 跳过 hook

## 3. 分支命名

- 功能分支：`feat/<issue-number>-<slug>`（如 `feat/3-search`）
- 修复分支：`fix/<issue-number>-<slug>`

## 4. 合入

- PR 到 `main`，标题用 Conventional Commits 格式
- PR 描述引用对应工单（`Closes #N`）
