# Proposal

## Why

`docs/research/` 六份调研的「可参考点 / 行动建议」此前只有散文结论，核验状态无处
可查：同一批条目被反复再调研，而真缺口（真机测量规程没有成文、增量路径与全量路径
没有一致性判据、resize 循环没有不累积断言）一直沉在文档里。

上轮（`2026-10-24-research-audit-followups`）已补齐三处键位/无障碍/重排缺口，并把
其余条目逐条落到「已有实现 + 用例」或「明确不适用 + 理由」。本轮收尾剩余五条：
两条补判据，三条把口径写成规范；随后把 `docs/research/` 中已解决的条目整体移除，
只留指向 openspec 的归属说明。

## What Changes

- 新增 `performance-measurement`：真机测量操作规程成文（判据用 p95/p99/missed-deadline、
  同设备同条件交替对比、模拟器数据不得作物理设备结论、环境条件随结果记录）；
  同时写明本仓基准负载与探针的口径，以及两条不采纳项（真实 PTY 捕获重放、
  atrace counter 关联）及其理由。
- `grid-extraction-incremental`：resize 后 MUST 整缓存重建，实例数等于当前网格，
  MUST NOT 累积旧网格的行。
- `render-stability`：增量构建与全量构建的输出 MUST 逐字节一致（termlib
  `changed.sameAs(fresh)` 的本仓等效判据）。
- `docs/research/`：六份文档中已解决的「可参考点 / 行动建议 / 观察项」整体移除，
  各留一句指向 openspec 能力的归属说明；研究结论（架构、对照、常量出处）保持原样。

## Capabilities

### New Capabilities

- `performance-measurement`：性能测量的判据、环境与负载口径，含不采纳项理由。

### Modified Capabilities

- `grid-extraction-incremental`：resize 必须整缓存重建。
- `render-stability`：增量与全量渲染输出一致。

## Impact

- `native/src/render/tests.rs`（两条新用例）
- `docs/research/*.md`（移除已解决条目）
- 不改 `docs/specification/` 与 `docs/STATUS.md`
