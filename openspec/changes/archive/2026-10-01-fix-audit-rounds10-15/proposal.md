# 审计修复（第10-15轮）

## Why

`docs/REVIEW-.md`至`.md`新增约10个P0与20余个P1，其中门禁惰性（N0-16）使CI信号失效，小步Kotlin状态机错误（选区钳位/点选计数/表面尺寸）与Rust注释失真持续误导后续修改。高危锁序与输出泵需调试后动，本变更先收敛低风险项。

## What Changes

- 新增`.semgrepignore`恢复`src/test`扫描（N0-16），不碰保护文件。
- Kotlin小步修复：主题应用切后台调度、点选计数复位、表面销毁清尺寸、清除缓存递归、搜索长度同源。
- Rust小步修复：删除`cached_scrollback`只写字段并更正超时注释，批量更正成本数字注释。
- 高危项（`take_kitty_placements`移出锁、`pollEvent`锁区、搜索前向扫描、行缓存方向）仅在实测定位后动，每项独立提交。

## Capabilities

### New Capabilities

- 无

### Modified Capabilities

- 无

## Impact

- 影响`android/app/src/main`、`native/src/android/ffi.rs`、`native/src/terminal`，不改保护文件，不新增功能，不改`docs/specification/`。
