# Proposal

## Why

针对 2026-10-22 两份 logcat 与 docs/STATUS.md 的未解决问题继续取证与修复：
启用 Kitty 图像协议的 file/shared-memory/temp-file 介质，避免 yazi/kitten icat
因 `EINVAL: unsupported medium` 无法显示图像；同步更新规范与归因记录。

## What Changes

- 终端初始化时启用 KGP 的 file、shared memory、temp file 介质（默认仅 direct）。
- 更新 kitty-graphics 规范，反映介质启用与失败时的明确应答。
- 新增变更记录，汇总本轮取证与修复。

## Capabilities

### New Capabilities

无。

### Modified Capabilities

- `kitty-graphics`: 共享内存/文件介质从“不支持时明确报错”扩展为“平台支持时启用，
  不支持时明确报错”。

## Impact

- `native/src/terminal/ghostty_terminal/internal.rs`
- `openspec/specs/kitty-graphics/spec.md`
