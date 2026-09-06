## Context

动机见 proposal.md。现状（均经代码与 termux-app 源码对照确认）：

- 项目 `installer/`（6 文件）已有 staging+rename、`SYMLINKS.txt`、`EXEC_PREFIXES` 0700、`termux.env` 标记机制，与 termux `TermuxInstaller.java` 语义对齐。
- 子进程环境（`SecondStageRunner`、`pty.rs`）已有 PREFIX/HOME/PATH/TMPDIR 设置；`LANG=C.UTF-8` 与 termux 的 `en_US.UTF-8` 有意不同。

## Goals / Non-Goals

**Goals:**

- 将绝对必要设置固化为可回归的行为契约。
- 明确记录与 termux 的两处有意差异及理由。

**Non-Goals:**

- 不改任何生产代码（用户约束）。
- 不引入 sha256 sidecar、不引入自定义环境变量（规范禁止）。

## Decisions

### D1: LANG 保持 C.UTF-8，不跟随 termux 的 en_US.UTF-8

- Why：C.UTF-8 无需 locale 数据依赖，在最小 bootstrap 中即可用；规范已明确，termux 值仅作对照记录。

### D2: login 语义与工作目录按 termux 对齐（已实现，只立契约）

- Why：对照确认实现已对齐，无需改动；契约防止回归。

## Risks / Trade-offs

- [Risk] termux 上游变更 env 细节 → Mitigation：本规范以已验证的 termux-app 版本为准绳，上游变更不自动跟进，需另行评估。

## Migration Plan

- 无需迁移：纯文档新增，无代码、无数据、无 API 变更。

## Open Questions

- 无。
