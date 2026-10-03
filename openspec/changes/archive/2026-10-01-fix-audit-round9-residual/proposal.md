# Proposal

## Why

第 9 轮残留中 `scrollbackLine` 负行号抛异常（N1-15）与 `:install` 进程重置崩溃计数（N0-12）仍未修：前者在正常滚动越界时抛 `IllegalArgumentException`，后者让崩溃循环兜底永久不触发。

## What Changes

- `scrollbackLine` 负行号直接返回 `null`（无数据），不抛异常；加单测断言。
- `TerminalApp.onCreate` 在 `:install` 进程跳过监控安装与健康标记。
- 无能力变更，纯缺陷修复。

## Capabilities

### New Capabilities

- 无

### Modified Capabilities

- 无

## Impact

- 影响 `native/src/android/ffi.rs`、`android/app/src/test`、 `android/app/src/main/java/terminal/emulator/TerminalApp.kt`，不改保护文件，不新增功能，不改 `docs/specification/`。
