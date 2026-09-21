## Why

Kotlin 侧 `TerminalRuntime.ensureMkshPromptRc()` 向 `$HOME/.mkshrc` 写入短提示符（`PS1='$ '`），但 mksh 仅在 `$ENV` 指向该文件时加载；native 侧 `build_env` 未注入 `ENV`，rc 不生效。设备取证确认：`HOME=... /system/bin/sh -i` 不加载 `~/.mkshrc`，`ENV=.../.mkshrc` 才加载；bash 忽略 `ENV`。长提示符（约 38 列）触发 mksh 行内横滚、左缘裁剪、`clear` 失效。

## What Changes

- native `build_env`（`native/src/terminal/pty.rs`）按白名单新增注入 `ENV=$HOME/.mkshrc`（代码实现由另一 agent 负责）。
- `docs/specification/DESIGN.md` Bootstrap 节环境变量白名单新增 `ENV` 条目。

## Capabilities

### New Capabilities

- `shell-env`: 子进程环境变量白名单，声明 `build_env` 允许设置的变量及用途。

### Modified Capabilities

## Impact

- 影响 `build_env` 产出的子进程环境：mksh 交互 shell 加载 `$HOME/.mkshrc` 短提示符；bash 忽略 `ENV`，无行为变化。
- 不新增除 `ENV` 外的任何变量；`LD_LIBRARY_PATH` / `PWD` / `LD_PRELOAD` 仍被禁止。
