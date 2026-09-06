## Why

Termux bootstrap 的绝对必要设置（安装原子性、权限、环境变量）散落在实现代码中，无行为契约可依。本变更以 termux-app 实现为对照基准（`TermuxInstaller.java`、`TermuxShellEnvironment.java`），将必要设置固化为长期规范，防止回归，且明确与 termux 有意不同的两点（LANG、sha256 sidecar 禁止）。

## What Changes

- 新增长期规范 `shell/bootstrap-environment`：安装流程契约（staging+rename、SYMLINKS、EXEC_PREFIXES、termux.env 标记）与子进程环境契约（PREFIX/HOME/PATH/TMPDIR/LANG/TERM/login 语义/工作目录）。
- 不改任何生产代码（纯文档 change；用户已约束询问前不得修改实际代码）。

## Capabilities

### New Capabilities

- `shell/bootstrap-environment`: bootstrap 安装与 shell 环境的绝对必要设置契约。

### Modified Capabilities

（无。）

## Impact

- 仅新增 openspec 文档；实现已存在（`installer/`、`pty.rs`），本变更只做契约固化与差异记录。
- 规范依据：`docs/specification/DESIGN.md` 设置·Bootstrap、终端·Shell、软件·应用数据、禁止实现条款；对照实现：termux-app `TermuxInstaller.java`、`TermuxShellEnvironment.java`、`TermuxSession.java`。
