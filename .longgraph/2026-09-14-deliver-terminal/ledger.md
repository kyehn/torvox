# torvox 终端全量修复交付 — longgraph Ledger

> 本 ledger 是本轮唯一记分牌。Authority 顺序：docs/specification/ 与用户提示 > directives.md STANDING > 本 ledger。环境事实见 `ops.md`。supervisor 纠正见 `directives.md`。
> 旧 run 在 `.longgraph/2026-09-13-deliver-terminal/`，正常执行不打开。本轮从零重新取证，不继承旧结论。

## Status header

Current milestone: M5 双绿验证 | Round: 12 | Last round net lines: +21
Next unclosed work item: R13 等 CI build-and-release（探针修复）结果；本地交互验证暂停（宿主杀模拟器）
Last directive folded: none

Convergence tracker: rounds since last 5: **1** | net lines since last +400: **-8** | **next round converges: no**

Milestone gate: `open`
Run status: `active`

---

## Current slice (the next round starts here)

Item: R13 等 CI build-and-release（542b2ad 探针修复）结果并收敛双绿
Write set: read-only（gh 轮询 + 日志分析）
Context: C-09, C-10
Verify: CI run 34803109259 结论 + 失败明细（如有）
Done when: 三 workflow 全绿确认；若仍红则定位下一 slice；随后跑第二次全绿循环

---

## Starting snapshot (carried-forward — replaces bulk history)

- 需求：全量修复——GHA/Rust/Kotlin 检查、lintDebug、模拟器测试、DocumentsProvider、IME 闪烁与字体压扁拉伸、底部行遮挡、滚动折叠撕裂、辅助键栏误触、切应用黑屏、启动缓慢、nix 红色 error 不可见、help 丢字母、超宽内容不可横向查看。循环直到连续两次全绿，日志+截图/OCR 验证。
- 已合入：336cac3（spotless+detekt）、695bf9d（主机 so 前置+按键测试）、fae18c0（测试收敛）、472b130（空闲渲染降频）、a14a293（测试 hermetic+lint 内存）、99a5527（IME 网格统一+光标跟随）、07baa52（平移纯函数+单测）、542b2ad（探针 AssertionError 修复），均已推 main。
- M2 CLOSED（本地）：cargo fmt/clippy/test 全绿；test-gradle.nu 全绿（GATE5/GATE9_EXIT=0）。
- CI：gradle-checks✓ rust-checks✓（a14a293）；build-and-release 在 99a5527 红（connected 字体测试），根因为测试探针误用 runCatchingCancellable 捕获不到 AssertionError，已修（542b2ad），待 CI 重跑确认。
- M4 已验证：空闲 166fps→~3fps（设备日志）；IME 网格统一+光标跟随（单测 7/7）；断言探针修复（编译+lint 绿）。
- 本地交互验证暂停：宿主环境 6 次杀模拟器进程（无崩溃日志），转 CI 设备信号 + 主机门禁。
- 生效约束：.github/scripts/flake.nix/rust-toolchain.toml/README/AGENTS/docs/specification 只读（内容）；GHA 变绿只靠修产品代码。

---

## Gate scoreboard（需求追溯：行为 | 真实消费者 | 验收 proof | 状态）

| Gate | Status | Evidence / next action |
| --- | --- | --- |
| lintDebug 全绿 | closed | GATE5/GATE9_EXIT=0（0 FAILED） |
| Kotlin 检查全绿（detekt 等） | closed | detekt 绿 + 全门禁绿 |
| Rust 检查全绿（clippy/test） | closed | fmt/clippy/test-workspace/test-lib1 全绿 |
| GHA 全绿（只修代码，不改 workflow 内容） | in-progress | gradle-checks✓ rust-checks✓；build-and-release 重跑中（542b2ad） |
| 模拟器测试全绿且连续两次 | in-progress | 本地字体测试 2 失败（探针 bug 已修，待 CI 确认）；本地交互暂停（宿主杀模拟器） |
| DocumentsProvider 复制进入/修改正常 | in-progress | 合同完整+单测绿；SAF 实测待稳定设备 |
| IME 无字体压扁拉伸闪烁 | in-progress | 网格统一（无重排）+ 单测；动画截图待稳定设备 |
| IME 剩余区域显示底部行 | in-progress | 光标最小跟随 + 单测 7/7；截图待稳定设备 |
| 上下滑正常滚动无折叠撕裂 | open | 渲染路径静态无致命缺陷；待设备复现 |
| 超宽内容可横向查看 | open | 待设备复现定性（换行 vs 截断 vs 丢字） |
| 辅助键栏不被上滑误触 | in-progress | 松手确认+单测锁定；设备手势待验 |
| 切应用返回无黑屏 | open | 恢复路径静态完整；待设备循环验证 |
| 终端启动不缓慢 | in-progress | 空闲 166fps→3fps 已验证；冷启动耗时待量 |
| nix 红色 error 可见、help 字母完整 | open | 候选：增量渲染过期；待设备复现 |

## Pending promotion (durable while Milestone gate = `pending-audit`)

Boundary: M2→M3/M4
Audit surface: /tmp/gradle-gate9.log（GATE9_EXIT=0）,/tmp/rust-gate3.log（RUSTGATE3_EXIT=0）,commits 336cac3..542b2ad
Evidence: M2 本地全绿；CI gradle/rust✓；build-and-release 重跑中

## owner-blocked (genuinely case-by-case human decisions only)

| ID | Decision in plain language | Recommended choice | Other choice(s) | Why now |
| --- | --- | --- | --- | --- |
| OB-001 | .github workflow 本身若有错，是否授权改其实际内容（AGENTS 默认禁止） | B — 先只修代码，workflow 缺陷登记 | A — 授权改指定文件 | 仅当 GHA 红因 workflow 本身错误时阻塞 |

## Debt & gap register (log every gap here; never silently fix or ignore)

| ID | Priority | Milestone | One line |
| --- | --- | --- | --- |
| GAP-002 | P1 | M3 | provider SAF 实测待稳定设备（合同完整+单测绿） |
| GAP-003 | P1 | M4 | 滚动折叠/撕裂待设备复现（增量路径静态无致命缺陷） |
| GAP-004 | P1 | M4 | 缺字形/红字不可见待设备复现（候选：增量渲染过期） |
| GAP-005 | P1 | M4 | 超宽内容/黑屏返回/启动耗时待设备验证 |
| GAP-006 | P0 | M5 | CI build-and-release（542b2ad）待绿；后跑第二次全绿循环 |
| GAP-007 | P1 | M5 | 本地交互验证暂停：宿主 6 次杀模拟器进程，记录为环境缺陷 |

## Rounds log — last 5 only (older → `archive/rounds.md`)

- R8 2026-09-14 | IME 网格统一+光标跟随打包（Rust/Kotlin），CI gradle/rust 重跑成功，提交 99a5527 | changed: 6 文件 | verify: GATE8=0, CI✓✓ | net +152/-48 | next: R10 平移单测
- R9 2026-09-14 | 平移纯函数抽取+7 单测全绿（XML 7/7），提交 07baa52 | changed: 2 文件 | verify: PAN_EXIT=0 | net +75/-12 | next: R10 探针修复
- R10 2026-09-14 | 测试探针 AssertionError 逃逸根因并修复 5 处，提交 542b2ad | changed: 2 测试文件 | verify: PROBE2_EXIT=0 | net +21/-14 | next: R11 收敛记分牌
- R11 2026-09-14 | 记分牌同步 + 本地交互暂停（宿主环境缺陷）+ CI build-and-release 重跑中 | changed: ledger | verify: run 34803109259 | net +0/-0 | next: R12 双绿收敛
- R12 pending | 等 CI build-and-release（542b2ad）结果 | changed: none | verify: gh run view | net +0/-0 | next: 第二次全绿循环
