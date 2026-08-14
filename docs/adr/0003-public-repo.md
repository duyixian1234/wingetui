# ADR-0003: 公开 GitHub 仓库 wingetui

- 状态：已接受
- 日期：2026-08-14
- 决策人：杜逸先（用户）

## 上下文

专家团治理规范要求工作对象必须是 GitHub 公开或私有仓库，以 `main` 为集成分支。用户决策：**公开仓库**，便于后续开源分享。

## 决策

- 仓库：`https://github.com/duyixian1234/wingetui`（公开）
- 集成分支：`main`，禁止直接 push（走 PR + CI 全绿后合并）
- 本地仓库与远程保持同步，远程为事实来源（`git ls-remote` 为准）

## 后果

- 正向：符合治理规范、可公开协作、CI 可云端验证
- 负向：公开仓库无隐私保护；winget JSON 解析的单测 fixture 需避免含本机真实软件列表隐私
- 缓解：单测 fixture 用脱敏的示例 JSON；README 明确项目定位

## 参考

- GitHub 仓库：<https://github.com/duyixian1234/wingetui>
