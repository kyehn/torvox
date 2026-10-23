# Proposal

## Why

2026-10-22 两份 logcat 的 753 条 W/E/F 已在归档变更「logcat-warning-audit」中做过一轮归因，
但有两类上游协议条目当时只留了笼统结论（「未实现的 CSI 与 kitty graphics 查询」）：

- `W/stream: unimplemented CSI action: ESC [ * {  } x`（DECSACE 选区擦除）——5 条；
- `W/kitty_gfx: erroneous kitty graphics response: EINVAL: unsupported medium`——5 条。

本轮把这两类条目落到可复核的上游事实（终端上游 master 与绑定层所钉提交两处各一份），
并把此前散落在实现里的三条日志纪律——应用自身的分级、高频诊断限频、外部条目逐条归因——
写成规范，使 753 条 W/E/F 每条都有结论、结论可被后续复查。

## What Changes

- 新增能力 `logcat-hygiene`：应用自身日志的分级纪律、诊断日志限频、外部与上游条目的
  归因留档要求。
- `upstream-alignment` 增量：补「终端上游与绑定层同时缺失的能力」的取证口径，
  并登记本轮确认的两条缺口（DECSACE 选区擦除、kitty 共享内存介质）。
- 新增审计文档 `logcat-audit.md`：753 条 W/E/F 的逐类计数、归因与结论；
  **不保留任何原始日志行、时间戳、PID、设备标识或下载链接**。

## Capabilities

### New Capabilities

- `logcat-hygiene`: 运行期日志的卫生要求——应用自身的正常路径不产生 W/E，
  高频诊断限频，外部与上游条目逐条归因并留档。

### Modified Capabilities

- `upstream-alignment`: 补「终端上游与绑定层同时缺失」的场景口径。

## Impact

- `openspec/specs/logcat-hygiene/spec.md`（新增）
- `openspec/specs/upstream-alignment/spec.md`
- `openspec/changes/2026-10-23-logcat-second-pass/logcat-audit.md`
- 实现侧无改动：本轮只补规范与归因留档；此前已落地的分级、限频与 kitty 介质启用
  保持不变。
