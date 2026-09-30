# 审计P0修复

## Why

`docs/REVIEW-SUMMARY.md`与`REVIEW-.md`共记录9个P0，自动化门禁全绿仍存在永久错误状态：析构无界阻塞、二段安装假成功、渲染锁序死锁、输出通道冻结、搜索O(n²)、文档链接全崩、安装路径穿越、跨会话帧复用。需按修复顺序逐项消除，每项几行改动，不碰保护文件，不违反`docs/specification/`。

## What Changes

- `GhosttyTerminal::drop`改用带超时连接，消除无界`join`。
- 二段安装按`errors`判定成功，编排器与视图模型按成功分支置状态。
- `take_kitty_placements`移出`RENDER_STATE`，关闭三处锁序边。
- `pollEvent`输出排空与`flush`移出锁区，渲染失败不冻结全部会话。
- 回滚搜索改为单次前向扫描并按物理行切分，消除O(n²)与列号越界。
- 文档链接两侧同口径规范化，修复符号链接全抛异常。
- `EXECUTABLES.txt`复用已有路径谓词校验，堵住权限位穿越。
- `last_frame`增列会话标识，空闲帧不跨会话复用。
- 全程小步提交推送，每步验证相关测试。

## Capabilities

### New Capabilities

- 无

### Modified Capabilities

- 无

## Impact

- 影响`native/src/terminal`、`native/src/android/ffi.rs`、`android/app/src/main/java/terminal/emulator/installer`、`DocumentQueries`、`TerminalRuntime`。
- 不改保护文件，不新增功能，不改`docs/specification/`。
