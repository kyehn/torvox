# Proposal

## Why

上一份可扩展性研究报告（`extensibility-research.md`）已在 `8936267c` 登记后删除，其结论全部指向
「不采纳／未声明／需明确同意」，但它没有评估任何外部实现，因此「支持 Rex／Herdr 时该走应用内扩展
程序还是组件库化」缺少一手证据。本变更把新一轮研究落盘：逐仓核验五个外部项目与两个对标参照，
给出形态选择与成本收益结论，并按 `research-ledger` 协议同批登记索引行与补齐行。

## What Changes

- 新增 `docs/research/extensibility-and-remote-backends-2026.md`：扩展形态评估（应用内插件 vs 组件库化
  vs 传输适配器）、五个外部项目逐仓评估、Termux 分层与 termlib 库化对标、能力模型对照、成本收益矩阵、
  事实／推论／未知三分的结论与推荐执行顺序。
- `research-ledger` 索引表新增一行 `extensibility-and-remote-backends-2026.md`；新增「在库报告的登记与计数」场景，
  把在库报告与已退役的 112 份分列。
- `research-ledger` 补齐行台账新增四行：会话传输抽象面（未声明不立项）、库化交付（不采纳，待产品决策）、
  外部项目可复用资产（仅参照，不引入代码）、Herdr 服务端渲染模式与本仓模拟器重复（仅参照）。
- 不实现报告提出的任何扩展面。

## Capabilities

### New Capabilities

无。

### Modified Capabilities

- `research-ledger`: 索引表新增在库报告的索引行、计数口径校正、补齐行台账新增四行。

## Impact

- Affected specs: `research-ledger`。
- Affected docs: `docs/research/extensibility-and-remote-backends-2026.md`（新增）。
- Affected code: 无。
- 行为变化：无（纯文档与规范整理）。
- 受保护文件：未改 `Cargo.toml`、`build.gradle.kts`、`settings.gradle.kts`、`docs/specification/`、
  `docs/STATUS.md`、`docs/persistence/`；未改 `flake.lock`。
