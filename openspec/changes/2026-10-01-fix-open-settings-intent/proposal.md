# Proposal

## Why

Activity 已在栈中时带打开设置 intent 二次启动只翻转普通变量，不触发重组，打开的是终端而非设置页。

## What Changes

- 打开设置请求改为可观察状态，新 intent 到达即生效。
- 无能力变更，纯缺陷修复。

## Capabilities

### New Capabilities

- 无

### Modified Capabilities

- 无

## Impact

- 影响 `android/app/src/main/java/terminal/emulator/MainActivity.kt`，不改保护文件，不新增功能，不改 `docs/specification/`。
