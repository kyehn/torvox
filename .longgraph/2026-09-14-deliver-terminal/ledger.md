# torvox 终端全量修复交付 — longgraph Ledger

> 本 ledger 是本轮唯一记分牌。Authority 顺序：docs/specification/ 与用户提示 > directives.md STANDING > 本 ledger。环境事实见 `ops.md`。supervisor 纠正见 `directives.md`。
> 旧 run 在 `.longgraph/2026-09-13-deliver-terminal/`，正常执行不打开。本轮从零重新取证，不继承旧结论。

## Status header

Current milestone: M1 证据基线 | Round: 1 | Last round net lines: +9
Next unclosed work item: R2 等 clippy/gradle 全门禁结果并落表
Last directive folded: none

Convergence tracker: rounds since last 5: **1** | net lines since last +400: **+9** | **next round converges: no**

Milestone gate: `open`
Run status: `active`

---

## Current slice (the next round starts here)

Item: R2 等 clippy/gradle 全门禁结果并落表
Write set: read-only（证据 /tmp/rust-clippy-baseline.log、/tmp/gradle-gate2.log）
Context: C-02, C-08, C-09
Verify: 两份日志含最终 EXIT 行且基线表填完
Done when: clippy/cargo test/detekt/lintDebug/单测基线全部落表，红项转 M2 slice

---

## Starting snapshot (carried-forward — replaces bulk history)

- 需求：全量修复——GHA/Rust/Kotlin 检查、lintDebug、模拟器测试、DocumentsProvider、IME 闪烁与字体压扁拉伸、底部行遮挡、滚动折叠撕裂、辅助键栏误触、切应用黑屏、启动缓慢、nix 红色 error 不可见、help 丢字母、超宽内容不可横向查看。循环直到连续两次全绿，日志+截图/OCR 验证。
- 基线：git 干净，分支 main，HEAD 87bacdf；R1：cargo fmt 全绿；spotlessKotlinCheck 红（4 文件）→已 spotlessApply 待验证；clippy/gradle 全门禁跑量中。
- GHA 基线：gradle-checks 红在 test-gradle.nu；build-and-release 红在 connectedDebugAndroidTest（有 failing tests）；rust-checks 绿（09-13）。
- 关键嫌疑（待重新取证，不作结论）：IME 期 attachWindow 480x420<->480x819 抖动 + setFontSizeInPlace 高频调用；cell_builder 缺字形分支；Manifest provider 声明；windowSoftInputMode=adjustNothing。
- 生效约束：.github/scripts/flake.nix/rust-toolchain.toml/README/AGENTS/docs/specification 只读（内容）；GHA 变绿只靠修产品代码。

---

## Gate scoreboard（需求追溯：行为 | 真实消费者 | 验收 proof | 状态）

| Gate | Status | Evidence / next action |
| --- | --- | --- |
| lintDebug 全绿 | open | spotless 红→已 spotlessApply，待 gate2 日志验证 |
| Kotlin 检查全绿（detekt 等） | open | 待 gate2（detekt/dokka/单测） |
| Rust 检查全绿（clippy/test） | open | fmt 全绿；clippy 编译中；cargo test 待跑 |
| GHA 全绿（只修代码，不改 workflow 内容） | open | 基线：gradle-checks 红在 test-gradle.nu；build-and-release 红在 connectedDebugAndroidTest；rust-checks 绿 |
| 模拟器测试全绿且连续两次 | open | CI connectedDebugAndroidTest 有 failing tests（明细待本地复现）；本地 emulator-5554 已连接 |
| DocumentsProvider 复制进入/修改正常 | open | 待 M3 slice；SAF 实测 + 单测 |
| IME 无字体压扁拉伸闪烁 | open | 待 M4 slice；日志无抖动 attach + 截图 |
| IME 剩余区域显示底部行 | open | 待 M4 slice；底部行截图/OCR |
| 上下滑正常滚动无折叠撕裂 | open | 待 M4 slice；滚动测试 + 截图 |
| 超宽内容可横向查看 | open | 待 M4 slice；横滑/右键验证 |
| 辅助键栏不被上滑误触 | open | 待 M4 slice；手势测试 |
| 切应用返回无黑屏 | open | 待 M4 slice；后台前台循环 |
| 终端启动不缓慢 | open | 待 M4 slice；冷启动耗时 |
| nix 红色 error 可见、help 字母完整 | open | 待 M4 slice；截图/OCR + 单测 |

## Pending promotion (durable while Milestone gate = `pending-audit`)

Boundary: none
Audit surface: none
Evidence: none

## owner-blocked (genuinely case-by-case human decisions only)

| ID | Decision in plain language | Recommended choice | Other choice(s) | Why now |
| --- | --- | --- | --- | --- |
| OB-001 | .github workflow 本身若有错，是否授权改其实际内容（AGENTS 默认禁止） | B — 先只修代码，workflow 缺陷登记 | A — 授权改指定文件 | 仅当 GHA 红因 workflow 本身错误时阻塞 |

## Debt & gap register (log every gap here; never silently fix or ignore)

| ID | Priority | Milestone | One line |
| --- | --- | --- | --- |
| GAP-001 | P0 | M1 | clippy/gradle 全门禁结果待 R2 落定 |
| GAP-002 | P1 | M3 | provider 声明与 SAF 行为待实测对照验证 |
| GAP-003 | P1 | M4 | IME 高度差来源待确认（adjustNothing 下谁改了 Surface 高度） |
| GAP-004 | P1 | M4 | 缺字形根因待查（atlas/shaping，为何 ASCII 字母丢失） |
| GAP-005 | P1 | M4 | 红色 error 不可见根因待查（颜色管线/主题/SGR 解析） |

## Rounds log — last 5 only (older → `archive/rounds.md`)

- R1 2026-09-14 | M1 基线：fmt 绿、spotless 红定位到 4 文件并 spotlessApply、GHA 基线落表 | changed: 4 kt 文件(格式) | verify: /tmp/gradle-baseline.log, APPLY_EXIT=0 | net +9/-0 | next: R2 + C-02/C-08/C-09
