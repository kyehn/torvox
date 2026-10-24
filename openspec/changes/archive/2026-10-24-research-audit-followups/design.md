# Design

## 一、复查方法

逐份通读 `docs/research/` 六份文档的「可参考点 / 行动建议 / 已核实无缺口」条目，
每条在仓库里找三样证据：实现位置、自动化判据、规范落点。三者齐全记「已落地」；
属评审口径或本仓架构不适用记「无需改动」并写理由；缺证据且值得做记「本轮补齐」。
不以文档自述为依据——所有「已实现」均以源码与用例为凭。

## 二、逐条对照

### redterm

| 条目 | 结论 | 证据 |
| --- | --- | --- |
| wake lock `isHeld` 自检重取 | 已落地 | `TerminalForegroundService.renewWakeLock` 在 `!isHeld` 时重走 `acquireWakeLockIfNeeded`，半程续期 + 自检双保险 |
| 启动路径单一事实源 | 已核实，不合并 | PTY 会话只有一条：`ShellEnv` → `pty::base_env`。安装器的 `prefixEnvironment` / postinst 环境面向 dpkg 维护脚本（无 `PATH`、带 `DPKG_*`），消费者不同，强行合并会让 dpkg 脚本看到无关变量 |
| 假对端测试 | 已有 | `native/src/terminal/mock_pty.rs` 即同思路；新增边界不依赖对端行为 |
| 错误注释纪律 | 评审口径 | 与 `STYLE.md`「注释极简、不得留死代码」同源 |

### termlib

| 条目 | 结论 | 证据 |
| --- | --- | --- |
| 真机基准操作规程 | 未落地，保留观察 | 需真机与固定环境，本仓 CI 不做性能门禁（`check.nu` 只编译 benchmark） |
| 真实 PTY 捕获重放 | 未落地，保留观察 | 需采集脚本与本地语料；语料不得 vendor（`BUILD.md`），合成负载仍覆盖键入路径 |
| 帧循环 trace counter | 未落地，保留观察 | 现有 `SLOW_FRAME` / 时序窗口已能定位帧内分段；input-to-present 关联需设备侧 Perfetto |
| opt-in 微基准门控 | 已等价 | criterion 基准不进测试套件，`cargo bench --quick` 由 `check.nu` 显式调用 |
| p95/p99 判据 | 已落地 | `FrameTimingStats` 输出 avg/p95/max 与 fps，慢帧按 500ms 限频 |
| bounded PerfettoConfig | 不适用 | 本仓不采集 Perfetto trace |
| 增量 vs 全新逐像素一致 | 未落地，保留观察 | 现有 `gpu_scroll_offset_full_redraw_has_no_stale_pixels` 覆盖滚动残留；脏带路径的像素等价用例尚未建立（需 GPU 脚手架扩展） |
| 旋转 resize 循环双堆采样 | 未落地，保留观察 | 需设备侧内存采样；Rust 侧无长生命周期堆增长点（图集/缓冲按需扩容且可复用） |
| JNI 回调所有权 | 已核实无缺口 | `native/src/android` 不建任何 JNI 全局引用（无 `Global` / `new_global_ref`），不存在回调环 |
| 宏基准边界文档化 | 已落地 | `docs/research/termlib.md` 的「macrobenchmark 评估」已写明三层能力边界 |

### termux-app

| 条目 | 结论 | 证据 |
| --- | --- | --- |
| KeyHandler 映射完整性 | **本轮补齐** | 逐键对照 `KeyHandler`：仅 `KEYCODE_BREAK` 缺映射（`KEYCODE_BACK` 归系统返回手势，不在键表职责内）；现映射到 `key::Key::Pause` |
| auto-repeat 无障碍联动 | **本轮补齐** | 此前固定 400ms；现取 `viewConfiguration.longPressTimeoutMillis`，越界退回 400ms，重复间隔 80ms（Termux 同名常量语义） |
| 双 client 生命周期 | 已核实 | 会话归属 Rust；Kotlin 侧不持有 Activity 引用，`LeakCanaryIntegrationTest` 在位 |
| 主线程消息泵 | 架构印证 | 渲染线程 latch 与合帧唤醒等价，无需改动 |
| `TerminalTestCase` 式直测 | 已落地 | `ghostty_terminal/tests.rs` 直接驱动 VT 并断言网格 |

### terminal-emulators

| 条目 | 结论 | 证据 |
| --- | --- | --- |
| 锁让渡（ghostty `yieldToDemand`） | 已核实无死锁面 | VT 线程从不取 `RENDER_STATE`（`native/src/terminal` 零引用）；渲染线程只在取会话锁后立刻释放，再取渲染锁 |
| damage 语义清单 | 已核实 | 当前光标行与上一光标行**都**标脏；滚动偏移非零或变化即强制全量重绘，故不存在「滚动后旧脏行偏移」漏画 |
| 输入三档压测 | 已等价 | 突发无损 `bulk_output_is_processed_without_loss` + 有界通道弃帧限频 `cell_frame_drop_warning_is_throttled`；三档命名仅作设计参照 |
| quirks 集中层 | 无需改动 | 现无新增平台 workaround；已有 workaround 均就地注释（surface 失效、acquire 超时） |
| xterm 仲裁基准 | 已声明 | `terminal-state-regression` 明确「VT 语义以上游为准，不以自建语料自验证」 |

### ghostty-android-terminal

| 条目 | 结论 | 证据 |
| --- | --- | --- |
| mksh SIGWINCH 不变量 | 已落地 | `startup-grid-resize` 规定首次 resize 前不把提示符喂给 shell |
| grapheme 行级标志门控 | 不适用 | 本仓 `CellData` 直接携带 `grapheme_extra`，无「带外溢出缓冲」这一层 |
| key encoder 前同步 setopt | 已落地 | VT 线程每次编码前调用 `set_options_from_terminal` |
| 文档质量（写清为什么不这么做） | 评审口径 | 本变更的 `design.md` 即按此写法记录 |

### reference-verification

| 条目 | 结论 | 证据 |
| --- | --- | --- |
| #23 缩窄重排保留文本 | **本轮补齐** | `shrink_reflow_keeps_wrapped_text` |
| RS-001 备用屏缩窄 | **本轮补齐** | `shrink_alt_screen_clips_without_reflow`（备用屏只裁剪、不重排，且不 panic） |
| #52 输出增量后搜索重扫 | 无需行动 | 需求置于上游 `search::Search`；本仓每次查询前先 `snapshot`，命中集随 feed 更新 |
| #26 BEL 事件 | 明确不实现 | `DESIGN.md` 要求忽略 bell |

## 三、本轮实现取舍

- **Pause/Break 只补映射，不自造序列**：`key-input-encoding` 已规定编码归上游；补表
  即修复，`ESC [ 34 ~` 由上游按 keypad/修饰位决定。
- **长按阈值不做常量搬运**：直接把系统值接进既有 `pointerInput` 分支，粘滞锁定与
  自动重复共用同一阈值（Termux 亦如此），不再保留第二个 400ms 常量。
- **缩窄重排的断言只钉可观察事实**：主屏断言「全文仍含原串」；备用屏断言「可见内容
  是原串的连续片段且长度等于新列宽」——不断言上游具体保留哪一段窗口（那是上游自由），
  但足以区分「重排」与「裁剪」。
