# 安全规范（.rules/security.md）

## 1. 凭据与隐私

1. 仓库为**公开**仓库：代码、fixture、文档中禁止出现任何本机真实信息（软件清单、用户名、路径、token）
2. 测试 fixture 一律脱敏（如 `Microsoft.PowerShell` 等通用公开包名）
3. 禁止提交 `.env`、密钥文件、`Cargo.lock` 中不存在的私有依赖

## 2. 命令执行安全

1. winget 命令参数一律经 `Command` 结构体传参，**禁止 shell 拼接字符串**（防注入，包名来自用户输入）
2. 包名/搜索词输入做校验：拒绝空串、控制字符、超长输入（>200 字符）
3. subprocess 超时控制：变更类命令默认 10 分钟超时，查询类 30 秒

## 3. 依赖安全

1. `cargo deny` 审计（若配置）：禁止引入已知漏洞依赖
2. 新增依赖需说明用途（见 AGENTS.md 原则 7）

## 4. CI 安全

- 工作流使用固定 commit SHA 的 actions（防供应链投毒）
- 不打印 token / 密钥
