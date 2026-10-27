# Proposal

## Why

`docs/specification/DESIGN.md` Shell 节要求：`PATH` 为 原 `PATH` 附加
`/data/data/com.termux/files/usr/bin`，仅目录存在时附加设置；failsafe 时保留原 `PATH`，
不附加。实现却停留在更早的规范版本（`PATH` 为 `/data/data/com.termux/files/usr/bin`
一项，failsafe 时完全不设 `PATH`）：规范改写（把环境变量白名单从 Bootstrap 节移入
Shell 节）时未同步实现。

后果有两处：

- 有引导程序的会话里 `PATH` 只有 `$PREFIX/bin`，系统命令（`am` `pm` `getprop`
  `dumpsys`）无法按名解析，只能写绝对路径。
- failsafe 会话（无 prefix）根本不设 `PATH`，子进程拿不到宿主 `PATH` 原值，
  与「保留原 `PATH`」的要求相悖。

## What Changes

- `base_env_with_host` 负责合成 `PATH`：宿主 `PATH` 原值在前，`{prefix}/bin` 在后，
  且 `{prefix}/bin` 仅在目录存在时附加；无 prefix（failsafe / 未装引导程序）时只保留
  宿主 `PATH`。
- Termux 的 bin 目录排在宿主 `PATH` 之前：Termux 工具（GNU 版 `grep` `sed` `ls`）
  优先于系统同名命令（toybox），系统命令仍可按名解析。
- `base_env_with_host` 增加「目录是否存在」的注入点，使 `PATH` 的断言与运行环境无关
  （沿用既有的宿主变量注入模式）。
- 增补单测：合成规则四则 + 一则端到端（真实 fork/execve 后子进程 `PATH` 回显）。

## Capabilities

### New Capabilities

无。

### Modified Capabilities

- `shell-env`: `PATH` 由「仅 `{prefix}/bin`」改为「宿主 `PATH` 原值 + `{prefix}/bin`
  （目录存在时）」，failsafe 会话保留宿主 `PATH`。

## Impact

- `native/src/terminal/pty.rs`：`base_env_with_host` 合成 `PATH`；`build_env` 不再
  单独写入 `PATH`。
- `native/src/terminal/pty/tests.rs`：`PATH` 相关断言改为注入宿主环境，补端到端用例。
- `docs/specification/DESIGN.md` 无需修改（约束已存在）。
