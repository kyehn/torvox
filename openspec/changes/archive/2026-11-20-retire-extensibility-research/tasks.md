# 任务

## 一、索引登记

- [x] `research-ledger` 索引表新增 `extensibility-research.md` 行：实质（库化交付／会话传输接口／
      远程后端选型研究）、判定（插件体系与 MCP 与 Termux:API 兼容层撞 `PROHIBITED.md`；库化拆分与
      远程后端属未声明功能面且需改受保护文件）、依据（`PROHIBITED.md`、`DESIGN.md`、
      `STYLE.md` 不得定义 crate features、AGENTS.md 受保护文件约束）
- [x] 计数口径校正：「全部 111 份报告已整体删除」补上本仓自产的第 112 份

## 二、报告退役

- [x] 逐条核对报告的可执行项：插件运行时、库化拆分、远程后端、传输接口——全部落在
      「明禁／未声明／需用户同意」，无仍需遵守的结论待落地
- [x] 删除 `docs/extensibility-research.md`

## 三、验证

- [x] `semgrep scan --error --config .semgrep/rules.yml`：0 findings
- [x] `openspec validate --all --strict`：通过
- [x] `markdownlint-cli2 "**/*.md"`：0 issues
