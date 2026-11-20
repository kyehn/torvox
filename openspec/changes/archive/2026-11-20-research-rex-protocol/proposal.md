# Proposal

## Why

上一份远程后端研究报告（`extensibility-and-remote-backends-2026.md`）把「支持 Rex／Herdr」定位为传输适配器
的目标态，并给出「先声明、后重构、再接后端」的顺序，但没有核验 Rex 协议本身：控制面与数据面各是什么形状、
连接怎么建、字节怎么续、屏幕怎么恢复、输入谁保序、退出码从哪来，都没有一手依据。本变更把这条线补齐：逐条
核验 Rex 协议的一手事实与本仓接缝的差异，给出必要性裁定与方法步骤，并按 `research-ledger` 协议同批登记索引行
与补齐行。

## What Changes

- 新增 `docs/research/rex-protocol-support-2026.md`：Rex 协议一手事实（端点与子协议、控制面 hello／welcome 与
  修订协商、事件 seq 与响应 seq 的合并规则、错误码表、断线重连与事件丢失语义、数据面票据与握手四种消息、
  精确字节计数续传、`GHOSTSNP` 快照与 `resize`／`theme`／`child_exited` APC 消息、block 持有者与尺寸生效规则）、
  本仓接缝差异表、必要性裁定（不立项及其前提）、六阶段方法步骤、协议真值归属、事实／推论／未知三分、不做清单。
- `research-ledger` 索引表新增一行 `rex-protocol-support-2026.md`；「在库报告的登记与计数」场景补记在库两份。
- `research-ledger` 补齐行台账新增六行：Rex 客户端形态、会话传输面接缝、上游快照解码面、WebSocket／TLS 依赖、
  外部项目可复用资产、客户端保存 reconnect token 与远端会话标识。
- 不实现报告提出的任何远程会话能力；不改受保护文件。

## Capabilities

### New Capabilities

无。

### Modified Capabilities

- `research-ledger`: 索引表新增在库报告的索引行、计数口径补记在库两份、补齐行台账新增六行。

## Impact

- Affected specs: `research-ledger`。
- Affected docs: `docs/research/rex-protocol-support-2026.md`（新增）。
- Affected code: 无。
- 行为变化：无（纯文档与规范整理）。
- 受保护文件：未改 `Cargo.toml`、`build.gradle.kts`、`settings.gradle.kts`、`docs/specification/`、
  `docs/STATUS.md`、`docs/persistence/`；未改 `flake.lock`。
