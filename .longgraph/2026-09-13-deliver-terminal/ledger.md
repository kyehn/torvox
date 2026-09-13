# torvox 终端修复交付 — longgraph Ledger

> 本 ledger 是本轮唯一记分牌。Authority 顺序：docs/specification/ 与用户提示 > directives.md STANDING > 本 ledger。环境事实见 `ops.md`。supervisor 纠正见 `directives.md`。
> 旧 rounds 在 `archive/rounds.md`，正常执行不打开。新 round 所需一切见快照与持久节。

## Status header

Current milestone: M1 证据基线 | Round: 0 (starts at 1) | Last round net lines: —
Next unclosed work item: R1 取 lintDebug 全量报错并登记各门禁基线
Last directive folded: none

Convergence tracker: rounds since last 5: **0** | net lines since last +400: **+0** | **next round converges: no**

Milestone gate: `open`
Run status: `active`

---

## Current slice (the next round starts here)

Item: R1 取 lintDebug 全量报错并登记各门禁基线
Write set: read-only（证据落本 run 目录与 /tmp，不碰产品路径）
Context: C-01, C-02, C-08, C-09
Verify: /tmp/lintDebug.log 含最终结果且基线表填完
Done when: lint 全量错误清单与 GHA/rust/kotlin/模拟器基线全部落表

---

## Starting snapshot (carried-forward — replaces bulk history)

- 已读：docs/specification/（BUILD/DESIGN/STYLE/TESTING）+ AGENTS.md；TerminalSurface.kt 全量 3222 行；cell_builder.rs 全量；AndroidManifest.xml 全量；TerminalDocumentsProvider/DocumentMutations/DocumentQueries 全量；TerminalScreen.kt 大部。
- 已知证据：IME 期间 attachWindow 480x420<->480x819 抖动 + setFontSizeInPlace 高频调用；onSizeChanged/surfaceChanged 直连交换链重建；cell_builder 缺字形分支只推背景 quad；Manifest provider 声明 MANAGE_DOCUMENTS 存疑；windowSoftInputMode=adjustNothing。
- 基线：git 干净，分支 main，HEAD ba206cc；lintDebug 后台曾跑（log 有 Configure 输出，进程待定）。
- 本次 authoring 未改产品文件；新建 .longgraph/2026-09-13-deliver-terminal/（本轮状态）。
- 生效约束：.github/scripts/flake.nix/rust-toolchain.toml/README/AGENTS/docs/specification 禁止改实际内容（AGENTS.md 禁止修改文件节）。

---

## Gate scoreboard（需求追溯：行为 | 真实消费者 | 验收 proof | 状态）

| Gate | Status | Evidence / next action |
| --- | --- | --- |
| lintDebug 全绿 | open | 待 R1 取全量清单 |
| Kotlin 检查全绿（detekt 等） | open | 待 R1 登记确切命令与基线 |
| Rust 检查全绿（clippy/test） | open | 待 R1 登记确切命令与基线 |
| GHA 全绿（只修代码，不改 workflow 内容） | open | 待 R1 登记红 workflow 与基线 |
| 模拟器测试全绿且连续两次 | open | 待 R1 登记命令与基线 |
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
| GAP-001 | P0 | M1 | lintDebug 后台进程状态待收敛（log 曾仅 Configure 输出） |
| GAP-002 | P1 | M3 | MANAGE_DOCUMENTS 声明嫌疑待对照 Termux 行为验证 |
| GAP-003 | P1 | M4 | IME 高度差 399px 来源待确认（adjustNothing 下谁改了 Surface 高度） |
| GAP-004 | P1 | M4 | 缺字形根因待查（atlas/shaping，为何 ASCII h 丢失） |
| GAP-005 | P2 | M5 | 模拟器测试确切命令与两次循环方法待 R1 落定 |

## Rounds log — last 5 only (older → `archive/rounds.md`)

(none yet)
