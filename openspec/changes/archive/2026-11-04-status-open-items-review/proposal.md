# Proposal

## Why

`docs/STATUS.md` 是维护者手写的真机待办清单，文件头写着「禁止对本文件进行任何修改」。
本仓因此没有别的地方记这 11 条的处置口径：条目只在会话里口头交代，证据随会话消失，
下一轮复查又要从头取证；已经修好的条目与仍然存在的条目混排在一起，读者无法分辨
哪些还有事要做。

同一批待办里还有两类「以为做完、其实没做完」的账：

- `docs/persistence/` 是上游调研记录，写成时本仓还没实现响铃三态与 `termux.properties`
  体系，文档里「本仓忽略 BEL」「`termux.properties` 不适用」等结论随实现推进变成了错话，
  且与 `openspec/specs/upstream-alignment/spec.md` 的上游取用台账互相矛盾。
- `openspec/specs/upstream-alignment/spec.md` 的「上游 API 取用判定登记」把 `on_bell`
  列为「不采用 / Kotlin 侧零消费者」，而实现早已按 `DESIGN.md` 的 `bell-character` 三态
  接线。规范与实现相反，比没有规范更糟：后来者照台账会把已接线的回调删掉。

## What Changes

- 新增留档 `status-disposition.md`：`docs/STATUS.md` 的 11 条逐条给出当前状态、根因、
  落盘位置与可运行判据，并如实标出仍需真机或人工确认的部分与原因。
- 修正 `docs/persistence/` 中与实现相反的结论（响铃三态、`termux.properties` 实施口径）
  与已失效的模块路径（`session.rs` → `session/`、`render/context.rs` → `render/context/`）。
- 修正 `openspec/specs/upstream-alignment/spec.md` 的响铃取用台账：拆出 `on_bell`
  独立行并改为「采用」，其余行不再引用「零消费者」这一已失效前提。

## Capabilities

无规范增量：本变更只留处置证据并纠正已被实现推翻的台账行，不新增行为要求。

## Impact

- `openspec/changes/archive/2026-11-04-status-open-items-review/status-disposition.md`（新增）
- `openspec/changes/archive/2026-11-04-status-open-items-review/proposal.md`、`tasks.md`（新增）
- `docs/persistence/osc-notifications.md`、`docs/persistence/terminal-protocols.md`、
  `docs/persistence/termux-app.md`、`docs/persistence/libghostty.md`（结论与路径订正）
- `openspec/specs/upstream-alignment/spec.md`（响铃取用台账订正）
- 实现侧无改动。
