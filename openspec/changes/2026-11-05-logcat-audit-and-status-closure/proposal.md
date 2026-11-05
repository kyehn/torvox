# Proposal

## Why

上一轮审计（`2026-11-04-status-open-items-review`）把 `docs/STATUS.md` 的「文档提供者找不出
rollback 文件警告」一条挂在「缺原始告警文本，拿到前不改代码」上。本轮真机 logcat 给到了原文：
平台框架在文档提供者抛出 `FileNotFoundException` 时打出 W 级「Failed during query」，异常来自本仓
提供者的 `queryDocument`。该条可以收口。

同一批 logcat 里还有三类此前没进台账的条目——平台帧时间线的同步组查询告警、上游未实现的 VT420
矩形区域属性（DECSACE）、kitty 图像文件介质被上游按目录基准拒绝。没有台账时，每轮复查都要从日志
重新取证；而「客户端回查缺失文档」这类条目极易被误判成应用缺陷，再被「兜底」成空响应——那会把
「文档不存在」伪装成「文档存在但没有内容」。

## What Changes

- 新增留档 `status-disposition.md`：本轮 logcat 的 W/E 条目按「应用自身／上游终端／平台／外部
  程序」逐类归因，给出根因、可复核事实与处置；并收口 `docs/STATUS.md` 的 rollback 告警一条。
- `logcat-hygiene` 增补两条要求的场景：框架契约触发的告警、上游未实现的矩形区域属性动作、
  平台帧时间线同步组告警的归因口径。
- `documents-provider` 增补一条要求：缺失文档的契约异常不得被空游标或占位行替代——该语义正是
  框架 W 级告警的来源，不得靠改写响应把它消掉。

## Capabilities

### New Capabilities

无。

### Modified Capabilities

- `logcat-hygiene`：「外部与上游条目必须逐条归因并留档」「平台与驱动侧条目必须给出可复核的
  上游事实」两条要求增补场景。
- `documents-provider`：在「缺失路径的查询按 AOSP 契约抛 FileNotFoundException」之外增补一条
  要求，把「不得以空游标或占位行替代契约异常」钉住。

## Impact

- `openspec/changes/2026-11-05-logcat-audit-and-status-closure/`：新增提案、规约增量、设计、任务
  与留档。
- 归档后同步 `openspec/specs/logcat-hygiene/spec.md` 与 `openspec/specs/documents-provider/spec.md`。
- 实现侧只增一条回归用例（缺失让位文件的查询契约），不改运行时行为：被审计的告警都不是应用
  缺陷，改动它们要么越过上游协议，要么靠吞掉日志制造假绿。
