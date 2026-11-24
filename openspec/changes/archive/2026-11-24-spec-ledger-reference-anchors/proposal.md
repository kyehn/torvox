# Proposal

## Why

上一条变更把上游对齐的台账与约束归位后，复验索引表与补齐行台账里的条目名引用，仍有三处
指不到现行条目：

- 「禁改区待维护者裁定项」比 `code-health` 的条目名少一个「台账」字。
- 「不得反向采纳的做法」在五处索引行里指向参照面台账中那批标为不得反向采纳的行，而该台账
  只有「反面做法点名与归属」「反面做法可独立复核」两个场景，没有同名锚点。
- 「上游 VT 语义不由本仓测试覆盖」的引用跨行折行，字符串对不上条目名。

## What Changes

- 索引表里「禁改区待维护者裁定项」补全为「禁改区待维护者裁定项台账」。
- `research-ledger` 的「参照面与反面做法收敛补齐行」增补场景「不得反向采纳的做法」：被标为
  不得反向采纳的行必须点名出处仓库与具体做法，不得只写结论。
- `terminal-state-regression` 的「按输入是否格式合规分流采纳」把跨行折行的条目名引用收回
  同一行。

## Capabilities

### New Capabilities

无。

### Modified Capabilities

- `research-ledger`：索引表一处条目名补全，参照面台账增补一个场景锚点。
- `terminal-state-regression`：一处跨行折行的条目名引用收回同一行。

## Impact

- `openspec/changes/2026-11-24-spec-ledger-reference-anchors/`：提案、任务与两份规约增量。
- `openspec/specs/`：归档时同步上述两处。
- 源码、`docs/specification/`、受保护文件：均不修改。
