# Proposal

## Why

`android/.../installer/` 的 bootstrap 安装器已完整实现（staging 解压、清单建链、原子换入、postinst）但 `openspec/specs/` 无对应能力。25 仓逐个深研（02 termux `TermuxInstaller` staging→rename、06 RedTerm 完成后替换、09 ThothTerm staging 语义、21 on3 子进程清单）证明该流程是生态通行的正确模式，需以规范收敛实现行为，防止回归。

## What Changes

- 新增 `bootstrap-install` 能力：staging 解压与防护、清单建链、原子换入与回滚、postinst 幂等执行。
- 只收敛已有实现，不改行为；不碰 `docs/specification/`。

## Capabilities

### New Capabilities

- `bootstrap-install`: bootstrap 经 staging 的原子安装行为

### Modified Capabilities

## Impact

- 影响 `openspec/specs/` 新增能力；不影响代码与受限文件。
