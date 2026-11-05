# Proposal

## Why

「深度审查全部代码」不能停在「跑一遍扫描器」。扫描器给的是**形状**，不是缺陷：每一处
命中要么是真问题，要么是有前提的例外，两者的区别只能靠读代码与陷阱文档判定。若不把
判定写下来，命中就既不是缺陷也不是结论——下一次扫描仍会命中同一处，审查等于没做。

本轮用 `footgun-scan` 扫了 Android 主源码集（81 个文件，10 处命中），并对本会话刚
提交的 Rust 改动做了一次 `rust-review` 式复核。两边的结果都需要逐条落成文字：

- 10 处命中里，**没有**一处构成缺陷，但每一处都有具体的「为什么不是」——判据不写
  下来就无法复核。
- Rust 复核发现**一处真实回退**：把两个回归钉共用的持锁夹具收敛成共享守卫时，
  原先的 `join().expect("用户写入线程 panicked")` 断言被降级成了静默 join，测试在
  持锁线程 panic 时不再判红。这处已修（显式 `release()` 判红 + 析构兜底）。

## What Changes

- 新增留档 `triage.md`：
  - `footgun-scan` 的扫描范围、10 处命中的逐条判定与依据；
  - 本会话 Rust 改动的 review 报告（Must fix / Should fix / Consider / 已做得好），
    含已修的那处测试严格性回退。
- 规约 `code-health` ADDED「扫描命中必须逐条判定并留档」：命中不等于缺陷，未判定
  的命中一律视为未完成审查。

## Capabilities

- `code-health`（增量）：扫描命中的判定纪律。

## Impact

- `openspec/changes/archive/2026-11-05-footgun-triage/triage.md`（新增）
- `openspec/changes/archive/2026-11-05-footgun-triage/proposal.md`、`tasks.md`（新增）
- `openspec/specs/code-health/spec.md`（新增一条 Requirement）
- 实现侧改动：会话测试夹具的守卫新增显式 `release()`（判红），与本批次
  `refactor(test): 收敛仪器测试与 Rust 用例的重复夹具` 同批落地。
