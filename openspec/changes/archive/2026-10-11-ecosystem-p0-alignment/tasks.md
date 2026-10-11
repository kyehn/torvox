# Tasks

## 0. 逐点核验结论（2026-10-11 二轮）

先核验各任务的实现现状再推进，结论如下：

- **1.1 已实现**：`setPixelSize`（`native/src/android/ffi.rs:846`）→ `pty.set_pixel_size`
  （`native/src/terminal/pty.rs:518`）已组装 `winsize` 像素字段；`Session::resize`
  对同尺寸走 no-op 跳过（`native/src/terminal/session.rs` 的 `resize` 分支）。
- **1.2 已实现但需注意口径**：`Session::request_exit`（`session.rs`）已实现
  「前台组优先 + HUP→SIGCONT→宽限→KILL 有界升级」，含自杀 guard（`pgid == own_pgid`
  时退化为直杀子进程）。与设计稿的差别：取的是 `getpgid(child)` 而非
  `tcgetpgrp(master_fd)`。二者在子进程已 `setsid`（本仓 spawn 就是这么做的）时
  通常同值；只有当作业自己 `setpgid` 抢过前台权时才分叉。
- **1.3 已实现**：`pty.rs:686` 置 IUTF8，且关闭 IXON/IXOFF；宽字符退格有单测。
- **2.1 / 2.2 未见实现**：grep `broadcast|fanout|apply_settings_all` 在 `native/src` 无命中。
- **3.1 / 3.2 未见实现**：搜索导航复用命中表与宽表孤立代理项守卫均无对应代码。
- **4.1 部分完成**：`docs/research/` 的 §19/§20 收敛见 `design.md` Context。

### 假测试陷阱（务必先读）

为 1.2 取证时写过一个「前台组收尾」单测，**先复现失败、再改成通过，最后发现它
零判别力**：把 `session.request_exit()` 整行注释掉，测试依然绿。原因是
`spawn_test_session()` 用非交互 `/bin/sh`，作业从不真正抢到前台组，
`tcgetpgrp` 只是短暂读到子 sh 的 pid，随后临界条件自然消失——断言无论改不改
实现都成立。该测试已删除，未提交。

**教训（对本变更后续任务的约束）**：任何「先红后绿」的测试 MUST 再做一次反向
验证——注释掉被测的那一行实现，断言必须转红。否则就是把巧合当回归保护，
比没有测试更糟（它给人虚假的安全感）。

## 1. PTY 生命周期

- [x] 1.1 实现像素 winsize 组装与 no-op 跳过并通过对应单测验证
- [x] 1.2 实现前台进程组优先收尾与有界轮询升级并通过对应单测验证
      （口径差异见第 0 节：当前取 `getpgid(child)`，未取 `tcgetpgrp`）
- [x] 1.3 核对 IUTF8 标志与 CJK 退格语义并通过宽字符退格单测验证

## 2. 设置下发二分

- [x] 2.1 实现 live 设置全会话扇出并通过多会话集成测试验证
      （`TerminalRuntime.applySettings` 用 `sessions.values.forEach { applyRenderContract }`
      覆盖全部会话，`TerminalRuntime.kt:3094-3097`）
- [x] 2.2 实现 spawn-time 设置仅新建生效并通过新建与现存会话对照验证
      （`buildConfig()` 读取全部当前设置，仅在建会话路径消费；
      failsafe 分支与「只读不消费」注释见 `TerminalRuntime.kt:1018-1022`）

## 3. 搜索导航与宽表边界

- [x] 3.1 实现定位上下文保留与导航复用命中表并通过搜索导航单测验证
      （`TerminalScreen.kt` 的 `onPrevious`/`onNext` 只改 `currentIndex` 并复用
      `searchState.results`，不重跑 `searchAllInScrollback`；收窄时
      `SearchResult.isNarrowingDown` 保留 `currentIndex`，见 `TerminalScreen.kt:395-401`）
- [x] 3.2 实现行尾孤立代理项长度守卫并通过边界单测验证
      （`wideCharTailCols` 已在 Rust 侧实现并回传，Kotlin 经
      `Bridge.wideCharTailCols` 消费；Rust 侧 3 个边界单测
      `tests.rs:92/102/112` 覆盖只标尾格、窄文本为空、跨软换行行首不算）

## 4. 研究报告附录与校验

- [x] 4.1 在研究报告 §19 收敛 2 处口径差异并新增 §20 五组复验证据，验证每项均有证据位置（`docs/research/终端模拟器生态深度研究报告.md` §19 收敛 7 处含 MAX_MATCHES 与 kotlin-app 两处纠正，§20 列五组复验证据，各项均附 `文件:行` 位置）
- [x] 4.2 运行 openspec validate 与 markdownlint 并通过验证
- [x] 4.3 运行相关单测并通过验证（`cargo test -p native --lib -- search session pty wide_char surrogate backspace` 131 全过；`cargo test -p native --lib` 573 全过；`:app:testDebugUnitTest` 503 全过）
