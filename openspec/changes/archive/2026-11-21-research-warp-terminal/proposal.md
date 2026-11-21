# Proposal

## Why

本仓此前未对 Warp（`warpdotdev/warp`）做过一手核验：Warp 是与本仓同为 Rust 终端但形态差异极大的对照面
（自研终端模型 + `vte` 状态机 + 自研 UI 框架 + AI/云/Drive/远程能力），其工程实践（秘密脱敏、命令块、
`ref_tests` harness、多语言门禁、Nix 开发环境）与产品能力面（AI/代理、云同步、远程控制、Vi/编辑器）都值得
按本仓的采纳门槛逐条裁定。本变更把这条线补齐：逐条核验 Warp 的架构与能力，给出与本仓的逐项对照与可采纳性
判定，并按 `research-ledger` 协议同批登记索引行与补齐行。

## What Changes

- 新增 `docs/research/warp-terminal-research-2026.md`：Warp 的形态与架构（工作区与 crate 分层、渲染与 UI 框架、
  终端模拟层、输入与编辑、命令块与输出、AI 与代理、云与同步与持久化、远程与控制、搜索与补全与 Drive、平台与
  构建、测试与质量、许可证与治理）、与本仓逐项对照表（能力/测试资产）、可采纳性判定（测试资产门槛、功能面、
  参照点）、事实/推论/未知三分、不做清单、复现方式。
- `research-ledger` 索引表新增一行 `warp-terminal-research-2026.md`；「在库报告的登记与计数」场景补记在库五份。
- `research-ledger` 补齐行台账新增十二行：秘密脱敏、命令块、自研 VT 核、自研 UI 框架与 Metal 渲染、AI/代理与 MCP、
  云同步与 Drive 与环境变量编辑、远程与控制与会话共享、Vi 模式与内置编辑器、命令补全/LSP/仓库元数据、
  ref_tests 测试资产、escape_sequences_tests、多平台支持。
- 不实现报告提出的任何能力；不改受保护文件。

## Capabilities

### New Capabilities

无。

### Modified Capabilities

- `research-ledger`: 索引表新增在库报告的索引行、计数口径补记在库五份、补齐行台账新增十二行。

## Impact

- Affected specs: `research-ledger`。
- Affected docs: `docs/research/warp-terminal-research-2026.md`（新增）。
- Affected code: 无。
- 行为变化：无（纯文档与规范整理）。
- 受保护文件：未改 `Cargo.toml`、`build.gradle.kts`、`settings.gradle.kts`、`docs/specification/`、
  `docs/STATUS.md`、`docs/persistence/`；未改 `flake.lock`。
