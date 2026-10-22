# 任务

顺序固定为 `docs/STATUS.md` 的行序。每节先量后改：没有当次数字就不动代码。
每次提交只做一节（或一节内的一个独立修复），证据写在本节，带命令原文与结果。

## 一、Shell 启动缓慢

- [x] 度量：冷启动（`am force-stop` 后）从 `logcat` 同一时钟的
      `Start proc <pid>:com.termux` 到新增标记 `Runtime: first frame presented`——
      该行在 `hasPresentedFrame` 翻转时打印，翻转正是系统启动屏撤下的条件，
      故这就是用户看到的启动时长。3 次取中位数（AVD 15 / 1080×2400 / debug APK）。
- [x] 定根因（两处，均有当次证据）：
      1. **主线程读 .so**：`ThemeRepository.defaultDarkName()` → `NativeBridge.<clinit>` 的
         `System.loadLibrary("native")` 落在首帧前的组合里，StrictMode 记
         `DiskReadViolation ~duration=471 ms`，调用栈起点是
         `SettingsRepository$SettingsState.<init>` 的构造参数默认值。
      2. **空闲闭锁即输出延迟**：见一·附，静置后命令输出最坏等满 500 ms 才上屏。
- [x] 修根因：
      1. `TerminalApp.onCreate` 起守护线程 `native-preload` 触碰 `NativeBridge.warmUp()`，
         让类初始化（装载）与 Hilt/Compose 启动并行；`warmUp` 的文档写明
         MUST 在后台线程触碰，主线程调用只是把装载挪早。
         注：中间某个渲染提交把这次调用连同注释一起误删（`warmUp` 成为死代码），
         证据核对时发现后已恢复调用，并由 `NativeBridge` 的用法保证不再无声失效。
      2. 渲染唤醒改为条件变量（一·附）。
- [x] 复测（2026-10-20，同一 AVD、同一源码，A/B 对照）：
      - 不预装载：2270 / 2486 / 2440 ms，中位数 **2440 ms**，每次 1 条
        `DiskReadViolation ~duration=471 ms`。
      - 预装载：2498 / 2539 / 2566 ms，中位数 **2539 ms**，违规 **0** 条。
      - 结论：预装载把 471 ms 主线程 I/O 移出主线程（违规 1→0），但在 2 核
        SwiftShader 模拟器上与主线程争 CPU，墙钟未降（+99 ms，落在噪声内）；
        真机多核才可能摊薄，故**不以它为启动改善的依据**。
      - 启动时间分解（预装载构建）：进程启动 → `Native library loaded` 1225 ms →
        `start()` 1921 ms → 首帧 2498 ms。前两段是 debug 构建的类加载与 Compose
        首组合（release 不付这份代价），本仓代码可动的部分只有上面两处。
- [x] 复测输出延迟（一·附）：`IdleOutputWakeTest` 7 样本均值 19 ms / 中位数 7 ms /
      最大 107 ms，旧实现为 0..500 ms 均匀分布。
- [x] baseline profile 已由既有 `:baselineprofile` 模块（`BaselineProfileRule` +
      `includeInStartupProfile = true`）覆盖，无需新增：`app` 侧
      `baselineProfile { saveInSrc = true; automaticGenerationDuringBuild = false }`，
      采集产物 `app/src/release/generated/baselineProfiles/{baseline,startup}-prof.txt`
      与手写规则 `app/src/main/baselineProfiles/baseline-prof.txt` 由 AGP 合并进
      `assets/dexopt/baseline.prof|profm`。此前的「未做」记录已过时，本轮核实后改写；
      生成入口 `./gradlew :app:generateBaselineProfile`（需真机/模拟器，不进门禁）。

### 一·附、静置后的后台输出（渲染唤醒）

- [x] 根因：渲染循环的驻留点是 `Bridge.waitOutput` 里的 `LockSupport.parkNanos(timeout)`
      ——纯定时睡眠。Kotlin 侧信号（输入、vsync、滚动、选区）能 `unpark`，但 **PTY 输出
      本身没有任何通知源**：命令自己睡醒后打印、后台任务打印，都只能等循环按 17ms/500ms
      节拍自己醒来。闭锁落到 500ms 后，探针实测 `latchMs=500`、`genChanged=false`
      （探针已删除），即最坏要等满半秒才上屏。
- [x] 修：原生 `RenderWake`（代数 + 条件变量）与 `waitForOutput`/`wakeRender` JNI；
      `Session::spawn` 接 `OutputWake` 钩子，读线程每把一块输出放进队列就 `notify`；
      Kotlin 侧全部宿主信号统一走 `bridge.wakeRender()`；拆卸路径先 `notify` 再
      `interrupt`（条件变量等待不受中断影响，否则 join 白等满超时）。
- [x] 机制测试：`native/src/android/ffi.rs::render_wake_tests` 3 tests——期间已通知则
      立即返回、无通知等满超时且代数不变、跨线程通知唤醒等待者。
- [x] 仪器测试：`IdleOutputWakeTest` 用 7 个「静置 6s 后打印 `date +%s%3N`」样本量
      「打印 → 文本可读」。2026-10-20 AVD 15 / 1080×2400：均值 19ms、中位数 7ms、
      最大 107ms（预算均值 120ms / 中位数 100ms）。同一度量的旧实现是 0..500ms
      均匀分布（闭锁相位随机），7 个样本的均值判别漏报率 < 1%。
- [x] spec：`openspec/specs/render-loop-scroll-cadence/spec.md` 增「空闲闭锁不得成为
      输出的投递延迟」Requirement（含通知不持锁、拆卸先通知两个 Scenario）。

## 二、简体中文退格稍慢

- [x] 度量：`BackspaceLatencyTest` 走真实通道（`InputConnection.deleteSurroundingText`），
      终点取 `getRenderedCursorPacked`——最近一帧绘制取走的光标，即「少一个字的那帧
      已经交给合成器」。每次退格另记回调自身的主线程耗时。
- [x] 定根因：退格回调本身是微秒级（p50 76µs / p95 79µs），慢的是**上屏**：PTY 回显
      早已就绪，渲染循环却睡在 500ms 空闲闭锁上（见一·附），于是每次退格最多再等
      半秒。中文与 ASCII 在 PTY 上同为一条 `0x7F`，故「中文比英文慢」只是同一延迟
      在逐字删除汉字时的感知放大。
- [x] 修根因：一·附的帧唤醒。补充一处**残余竞态**：唤醒原先只在 PTY 字节到达时发出，
      那一拍 VT 线程可能还没解析完、帧通道里没有新帧，渲染线程醒来扑空后重新睡满闭锁；
      现在 VT 线程在帧**真正入队**后再叫一次（`RunConfig::frame_wake`），上屏只差一帧。
- [x] 复测（2026-10-20，AVD 15 / 1080×2400 / debug）：上屏 p50 **24ms**、p95 **101ms**
      （预算 300/700）；回调 p50 **76µs**、p95 **79µs**（预算 50ms）；
      中文 p50 **4ms** vs ASCII p50 **5ms**（比值 0.8，判据 ≤2.0）。
- [x] 顺带修掉度量工具自身的两处失真（否则数字不可信）：
      1. `scrollViewportToBottom` 原先向网格注入 `BOTTOM-<nanos>` 标记行来证明「视口在
         底部」。它会往用户屏幕写垃圾，并把真实内容一行行挤出视口——回滚为空的会话里
         连 shell 提示符都会被挤丢，导致后续用例等提示符 30s 超时（两类连跑时实测
         「shell prompt 未出现」）。改为读会话**已应用**的滚动偏移（新增
         `NativeBridge.getSessionScrollOffset`）：一次原子读，不写终端。
      2. 等提示符前先向 PTY 送一个空行，让 shell 重画提示符，不再赌上一用例残留的那一行。
- [x] 复跑验证：`SelectionDragQuantifiedTest` + `BackspaceLatencyTest` 同一
      instrumentation 进程连跑，6+2 个用例全绿（此前第二类两个用例必挂）。

## 三、文本选择性能与另一侧指针漂移

- [x] 度量：`SelectionDragQuantifiedTest` 拖动 END 柄若干整格，量**像素级**左缘：
      反色区域左边界由原生渲染器画出，是用户真正看到的那块匹配区域；同时数一次手势
      期间的选区写入次数。
- [x] 定根因：快速拖动路径刻意不写 `_state`（每帧一次 Compose 往返就是瓶颈），于是
      `applyHandleDrag` 看到的仍是拖动开始时的端点。越过另一侧手柄后归属已翻转，下一帧
      若拿旧端点当锚点，未拖动的那一侧就会被算到错误位置——与用户描述的
      「只拖一侧，另一侧的匹配区域会漂移」一致。
- [x] 修根因：`SelectionState.withPendingDrag` 把上一次未提交的拖动结果叠加到已提交
      选区上，每次计算都以上一次的实时边界为基准（代码在 `TerminalViewModel.kt`）。
- [x] 复测（2026-10-20，AVD 15）：`drag_start_edge_drift_px=0`（左缘一格未动）、
      `drag_selection_writes_per_12_moves=2`（12 次移动只写 2 次状态，其余走快速路径）、
      `drag_live_highlight_steps=4`（每步都有新反色，未退化为抬手才刷新）、
      `d75_blank_drag_upgrade=1`（空白处长按后拖柄能升级为真选区）。
- [x] 新增回归用例 `dragging_end_keeps_start_highlight_edge_fixed`：不依赖手柄 overlay
      或菜单几何，直接量网格反色左缘，钉住「拖一端，另一端不动」。

## 四、上下滑动滚动卡顿

- [x] 度量：`ScrollBehaviorQuantifiedTest`（已有）三个用例——回车归位是否在预算内、
      视口在 PTY 洪流下是否塌陷、拖动期整循环窗口的帧节奏。
      2026-10-20，AVD 15 @1080×2400（SwiftShader）：
      `drag_loop_pre_max_ms=0`（循环自身工作 ≤1ms，预算 8ms）、
      `drag_loop_p95_ms=123`、`drag_loop_fps=27`、
      `scroll_collapses_under_flood=0`。
      另有一次性探针（**无自动化钉**，仅供定位方向）：fling 采到 38 个不同偏移、
      单步 ≤3 行、总行程 60 行。
- [x] 定根因：三条真缺陷，均已单独提交（提交标题依次为「删除生产不可达的纵向平移路径」、
      「绝对行查询改用 Screen 空间」、「滚动取整对称化防漂移」）。余下两项是本轮新发现：
      ① 每帧把滚动偏移以 **uniform 写入** 下发，`native` 侧要拿会话锁，
      PTY 洪流时锁被写端持有，循环在渲染前排队（真机日志 `pre` 段 100ms+）——
      改为**只登记待用偏移**，由下一帧 `begin_frame` 自己按值重写，循环段不再碰锁
      （提交「消除滚动期每帧的重复 uniform 写入与锁等待」，并把 `setScrollOffset`
      改成 `try_lock` + 布尔回报，推迟时保留旧值下一帧重试，绝不静默丢更新）；
      ② 惯性滚动走厂商 `OverScroller`，其衰减曲线被 ROM 改写，
      同一手势在真机与模拟器上轨迹不同——改为自建指数衰减
      （提交「惯性滚动改为自建指数衰减，摆脱厂商改写的 OverScroller」）。
- [x] 复测：`drag_loop_pre_max_ms=0` 恒在预算内；
      分辨率对照实验 @540×1200（像素 ÷4）得 `drag_loop_p95_ms=85`，
      即单帧时长随像素量下降（123→85ms），说明余下耗时在**光栅化**
      （SwiftShader 单帧上百毫秒是设备能力），不是循环调度或锁等待。
      故断言只钉生产代码控制得了的 `pre_max ≤ 8ms`，帧率与整窗 p95 只作指标记录。

## 五、切换应用返回闪烁

- [x] 度量：`SurfaceLossRecoveryInstrumentedTest`
      的 `noBufferWindowIsCoveredByThemeBackground`：注入 surface 失效 →
      宿主换新 `SurfaceView` → 首帧未到的瞬间整屏截图，量开孔区域两件事：
      区域内相对中位色的最大偏差（`cover_spread`）与中位色相对终端底色的偏差
      （`cover_median_deviation`）。
- [x] 定根因：`SurfaceView` 的原生 surface 位于**窗口之下**，窗口那块是透明开孔。
      缓冲区缺失时（冷启动首帧前、切后台回来重建窗口时）开孔直接透出后方桌面，
      就是「返回一闪」。启动屏与窗口底色帮不上：闪的是开孔，不是窗口背景。
- [x] 修：无缓冲区期间由**窗口层**在开孔矩形上涂当前解析出的终端底色，
      一有帧提交（`TerminalRuntime.surfaceBufferPresent`）立即撤
      （提交「开孔无缓冲区时由窗口层盖底色，消除切应用返回透出桌面」）。
      配套：surface 重建改为「快试用尽后慢重试」，不再永久放弃自愈
      （提交「surface 重建在快试用尽后转慢重试」）。
- [x] 复测：`cover_spread=0`、`cover_median_deviation=0`
      （与按生产解析链算出的主题底色逐通道相等，说明遮的就是终端底色本身）。
      同时把测试里写死的 `0x1E1E2E` 换成
      `resolveAppDarkMode` → `resolveTerminalThemeName` → `ThemeRepository` 三件套，
      与 `TerminalScreen` 同一条链，换主题不再误判。

## 六、文件提供者响应较慢

- [x] 度量：`DocumentsProviderInstrumentedTest.large_directory_listing_stays_within_budget`
      建 300 项目录，冷查一次后连测 3 次：
      `large_listing_p50_ms=13`、`large_listing_max_ms=14`（预算 250ms）。
- [x] 定根因：每行三处重复——`encodeDocId` 每行重算 `rootDir.canonicalPath`、
      每行 1~2 次 `canonicalPath`、再各一次 `length()` / `lastModified()` stat。
      300 行目录旧实现实测 600ms（`slow listing` 告警的来源），
      且全在 Binder 线程上串行，客户端看到的就是「提供者反应慢」。
- [x] 修：目录先规范化一次；非链接子项全部取自一次 `lstat`；排序键用
      `Locale.ROOT` 折叠，不受系统区域影响（上一轮提交）。
- [x] 复测：同上 3 次采样，p50 13ms / max 14ms，比旧实现快 46 倍。

### 六·附、提供者停止时释放目录监视

- [x] 根因：`DocumentWatcher` 的监视表与广播线程只增不减；进程继续服务其他组件时
      每个看过的目录白占一个 inotify 实例，逼近应用级上限后**全部**监视静默失效。
- [x] 修：`DocumentWatcher.shutdown()` 注销全部观察者、丢弃待播事件、结束线程并置空，
      下次事件按需重启；`TerminalDocumentsProvider.shutdown()` 转调。
- [x] 检查：`DocumentWatcherTest` 6 tests / 0 failures、`DocumentsProviderTest`
      70 tests / 0 failures（`--rerun-tasks`，2026-10-20；§11.5 的权限位用例加入后
      该类为 72 tests / 0 failures）。
- [x] spec：`openspec/specs/documents-provider/spec.md` 的「终端侧改动即时广播」
      增加 shutdown 契约与两个 Scenario。

## 七、预览截图仪器测试

- [x] `PreviewScreenshotTest`：安装 bootstrap（官方 x86_64 产物，可经
      `-e test.bootstrapUrl file://…` 覆盖）→ `pkg update && pkg install fastfetch`
      → `clear; fastfetch` 截图 → 打开会话抽屉截图 → 打开设置截图。
- [x] 断言：每张 PNG 为 1080×2400、颜色数 ≥ 16。命令的**完成**由退出码文件判定
      （`runCommand` 原子重命名，不读屏幕）；截图前只确认终端上已出现 ≥3 行非空内容——
      CI 的 40 列屏会把 `Kernel:`/`Uptime:` 折成 `Ke`+`rnel:`，按整词判据必然假失败
      （CI 实证）。截图复制到 `/data/local/tmp/termux-preview-screenshots` 便于 `adb pull`。
- [x] 真机证据（2026-10-20，AVD 15 / 1080×2400）：`BUILD SUCCESSFUL in 2m 17s`；
      `BootstrapFixture: installing bootstrap from …/bootstrap-x86_64.zip` 16s 后
      `Success()`；三张 PNG 均 1080×2400、颜色 16，人工查看内容正确。
- [x] 记录截图产物路径与运行命令：产物在设备侧 `/data/local/tmp/termux-preview-screenshots/`
      （`adb pull` 取回），用例类 `terminal.emulator.ui.PreviewScreenshotTest`，
      随 `nix develop --command nu scripts/test-emulator.nu` 一并运行。
      `README.md` 属 AGENTS.md 禁改清单（未经用户明确要求不改），故记录在本变更。

## 八、质量门禁与规范

- [x] `nix develop --command bash -c 'nu scripts/check.nu'` 全绿（Rust fmt/clippy/test、
      Kotlin spotless/detekt/lint/test、semgrep、markdownlint、openspec validate）。
      2026-10-21：EXIT=0（/tmp/opencode/gate-r7.log 与本轮多次复跑）。
- [x] `openspec/specs` 与实现同步；本变更 `tasks.md` 每节都有当次证据。
      2026-10-21 同步的能力：`render-stability`（桌面图标安全区、系统栏图标跟随
      页面底色）、`session-list`（抽屉面板避开状态栏）、`documents-provider`
      （复制保留执行位）、`render-loop-scroll-cadence`、`cjk-rendering`。
- [x] 收口记录写在本变更（`docs/STATUS.md` 与 `.cargo/config.toml` 属 AGENTS.md
      禁改清单，未经用户同意不改；六项现象的最终证据见本文件各节）。
- [x] 已知且**不修**的一条告警，附理由：`cargo` 每次链接打印
      `warning: the gold linker is deprecated and has known bugs with Rust`。来源是
      禁改文件 `.cargo/config.toml` 的 `-fuse-ld=gold`。实测把该行改为默认链接器会让
      本仓在 Nix 工具链下链接失败（`compiler_rt` 符号未定义），改 `bfd` 亦同，故保留
      现状并记录，而不是删警告掩盖。

## 八·附、鼠标上报中的滚动转发：合并往返 + 无锁判据

- [x] 度量：`MouseReportingInstrumentedTest` 记录一次多行拖动的滚轮档位与成对性。
- [x] 根因：`onScroll` 对每一次滚过的行**逐档**调用 `encodeMouseEvent`（press 一次、
      release 一次），每次都是一次跨线程查询往返；一次拖动十余行即十余次 VT 应答，
      UI 线程按在应答上——这是「上下滑动滚动卡顿」在 TUI（yazi/vim）里的另一半来源。
      同时「是否启用上报」此前只能靠编码器返回空来猜（`isAltScreenActive` 之外无判据）。
- [x] 修：新增 `encodeMouseWheel(repeat)`，一次往返编完全部档位、PTY 只写一次；
      上报判定改用 VT 线程维护的无锁镜像 `mouseTrackingActive`（DECSET 1000/1002/1003）。
      外接鼠标滚轮同一 API（repeat=1），顺带把原来 press/release 两次往返并成一次。
- [x] 测试：Rust `encode_mouse_wheel_batches_every_notch`（repeat 2/5/12 等于单档整数倍、
      逐档逐字节相等；未上报时整批丢弃）；仪器测试新增
      `multiRowDragForwardsOneReleasePerRow`（≥3 档、按下数==抬起数）与
      `mouseTrackingMirrorFollowsDecsetState`（装载/退出两次跃迁）。整类 4 例全绿（48s）。
- [x] 顺带修：仪器测试的 `disarmMouseReporting` 原先只发 `stty sane`，而 DECSET 1000 是
      终端级模式，`stty` 不碰它——用例之间会凭空继承「应用在接收鼠标上报」。现在显式
      `printf '\033[?1000l\033[?1006l'`。
- [x] spec：`openspec/specs/key-input-encoding/spec.md` 增两个 Scenario（一次往返编完
      全部档位、上报状态由镜像读取）；另把 `scroll-physics-drift` 的超长 Requirement
      拆成「像素行程单位」与「衰减曲线由本仓实现」，补两个 Scenario，
      `openspec validate --all --strict` 36/36 通过（写下时为 34/34；§9.3 与 §11.1 新增/改写的
      两条 Requirement 正文超 500 字符，已在 2026-10-21 拆分为「Requirement + Scenario」，
      拆分后 36/36 全绿）。

## 九、三项追加问题（kitty 图像 / 会话标题 / CJK 后备提示）

### 9.1 kitty 图像「有图不显示」

- [x] 定位链路：VT 解析（上游 libghostty-vt）→ `ffi.rs` 取放置帧 →
      `render/kitty.rs` 打包图集 → `render/pass.rs` 绘制。
- [x] 确认能力探测正常：设备上实测应答为 `ESC_Gi=278941603;OK ESC\`（回显被
      查询的 id，与 yazi 的解析口径一致），`ESC_Gi=916472805;EINVAL: unsupported
      medium`（共享内存不支持，明确报错），DA1 应答 `ESC[?62;22c`。
- [x] 复现「图像不显示」并定根因（两处，均有当次证据）：
      1. **单元格像素几何从未下发**：kitty 放置的网格尺寸由上游
         `t.width_px / t.cols` 推出（上游 `graphics_storage.zig` 的 `gridSize`），
         该值为 0 时放置被判为「不可见」——协议解析、解码、图集打包全部正常，
         画面上什么都没有。设备实测 `PROB（日志行已移除）
         VT 的 `width_px` 只由 `SetCellPixelSize` 决定，而该命令只有一条来路：
         视图侧 `applyGridResize`（布局早于会话存在 → 空操作）与运行期
         `recomputeGridFromFontMetrics`（「重算结果与当前网格相同」时早退——
         会话带着默认网格诞生，重算结果与之相同）。两处都落空 → 几何从未下发。
      2. **叠加层存在即强制整帧重绘**：一张静止的图片把每一帧都钉在全量帧的
         代价上（SwiftShader 全量帧约 500 ms → 1-2fps），yazi 这类全屏 TUI
         几乎不可用。
- [x] 修根因：①`recomputeGridFromFontMetrics` 在网格尺寸未变时也下发像素
      几何；②渲染门控从「有图像」改成「叠加层本帧发生变化」（新增/替换/删除
      图像时旧像素需被清掉），叠加层未变时允许脏带增量；③叠加层按「被擦掉的
      行」重画（部分帧里按像素裁剪），避免半透明图反复混合越叠越实。
- [x] 复测：新增仪器测试 `KittyGraphicsOverlayTest`（经 PTY 送出 kitty 传输
      序列，用 PixelCopy 读回 Surface 像素确认上屏，再量渲染循环帧率）：
      `UX_METRIC kitty_overlay_pixels=462`、`kitty_overlay_fps=65`、
      `kitty_overlay_p95_ms=27`。
- [x] 回归钉：原生测试 `kitty_graphics_query_echoes_the_requested_image_id`
      （能力探测应答必须回显被查询的 id）；渲染门控的表驱动单测改为
      「叠加层变化才全量、静止允许部分」。

### 9.2 会话标题（sh/bash 无标题，自带上报的 shell 正常）

- [x] 定根因：标题来自 shell 发出的 OSC 0/2。自带上报的 shell 直接可用；mksh
      与 bash **自己都不发**（bash 只有用户自己设了 `PROMPT_COMMAND` 才发）。
      `ENV`/`.mkshrc` 机制是**生产**机制（它修 mksh 长提示符的横滚问题），
      不是测试专用设置，与标题缺失无因果关系。
- [x] 修根因：①应用写的 mksh rc 在提示符里带 OSC 0 序列（当前目录名）——
      mksh 不解释提示符里的反斜杠转义，故写**字面量** ESC/BEL；实测 mksh
      计算提示符宽度时会跳过转义序列，可见宽度仍是两字符，命令行不提前折行；
      ②bash 经**环境变量**注入 `PROMPT_COMMAND`（唯一不碰用户 rc 文件的
      做法：bash 启动时若 shell 内未定义该变量就用环境值，用户 rc 自己设过
      的以用户为准）；注入只针对确需钩子的 shell（按可执行文件名判定），
      `build_env` 的白名单本身不变。
- [x] 复测：新增仪器测试 `SessionTitleInstrumentedTest`，默认会话（mksh）的
      真实标题经桥接读回并等于当前目录名：`UX_METRIC session_title=home`。

### 9.3 CJK 后备「跳过」提示

- [x] 定根因：提示写「主字体支持 CJK」，而实际判据是「主字体覆盖**本 locale
      的 CJK 代表字符**」（zh/ja 为「中」、ko 为「가」、非 CJK locale 退到
      「中」）这一枚探针——覆盖它不等于覆盖整套 CJK 字符。
- [x] 修根因：文案改为「CJK 后备已跳过（主字体已覆盖本语言的代表字符）」，
      与 native 侧 `primary_covers_cjk_probe` 的判据同源。
- [x] 规范同步：`openspec/specs/cjk-rendering` 增补「状态上报与提示必须如实
      反映判据」；新增 `openspec/specs/kitty-graphics`（能力探测应答、单元格
      像素几何先于放置就位、叠加层不得强制整帧重绘）；`openspec/specs/shell-env`
      增补「会话标题按 shell 族注入上报钩子」。

## 十、预览截图暴露的三处视觉缺陷（2026-10-21）

预览截图的旧样本（`origin/screenshots` 分支）暴露三处缺陷，逐一取证后修复。

### 10.1 桌面启动图标被蒙版裁掉

- [x] 度量：设备桌面抽屉截图（1080×2400）上 Terminal 图标只剩终端窗口的一角与一个
      绿点，橙色星形与尾迹方块全部消失；启动屏（同一份图案）却完整。
- [x] 定根因（平台几何 + 设备实证）：`AdaptiveIconDrawable` 把图层区域撑到图标边界的
      1.5 倍且居中，蒙版圆半径只有图标边界的一半，安全区再乘 `66/72`；而
      `launcher_foreground` 的 `<item>` 写死 108dp，图案 1:1 画在图层区左上角，
      整体左移 27 单位——星形（视口 x 77.65..91.57）落到子层自身范围外，被裁。
      仪器侧独立复现：48px 渲染时橙色星形纯色像素为 **0**，108px 才有 43px。
- [x] 修根因（平台标准做法：artwork 本身画在安全区内）：`logo_art` 的 108 视口内
      加 0.65 预缩放（图案到画布中心最大距离 48.51 → 31.53 单位 ≤ 安全半径 33），
      `launcher_foreground` 改用 `gravity="fill"`（不再写死 dp，随图标大小缩放），
      启动屏包装按 0.65 的倒数放大到 148dp 以保持视觉大小不变；
      `launcher_monochrome` 同步预缩放，并补上点迹路径缺失的 `moveTo`
      （原为弧起头，画成一整块色斑）。
- [x] 复测：Robolectric 判据（前景墨迹占图层区域的比例 ≤ `66/72 ÷ 1.5`）在
      48/108/216dp 三档全过；星形纯色像素 > 0；设备桌面图标与启动屏均完整。
      回归测试 `LauncherAdaptiveIconRobolectricTest`（3 例，含 monochrome 覆盖）、
      `SplashIconRobolectricTest`（2 例）。
- [x] 规范同步：`render-stability` 的启动屏条目改写为「图案按安全区预缩放 +
      桌面前景以 `fill` 呈现 + 启动屏以固定尺寸收缩包装呈现」。

### 10.2 系统栏图标在浅色页面上不可读

- [x] 度量：抽屉展开时状态栏区一半是浅色面板 (250,248,255)、一半是深色终端，
      白色时钟压在浅色面板上不可读；设置页整片浅色底 + 浅色时钟（预览截图
      `03-settings.png` 实证）。
- [x] 定根因：系统栏图标明暗只有一个全局开关，而此前只跟随**终端**底色；
      抽屉面板的**背景**又铺到屏幕顶端（Material3 把 `windowInsetsPadding` 施加在
      面板的内容而非 Surface 上），于是浅色面板盖住状态栏区。
- [x] 修根因：①抽屉面板整体按状态栏高度下移，状态栏区统一露出遮罩下的终端底色；
      ②系统栏图标明暗抽成共享件 `SystemBarsFollowBackground`，**每个页面按自己的
      底色**判定（终端页用终端底色，设置页用设置页底色）。
- [x] 复测：设备实证——抽屉展开时状态栏整行 (22,22,22)、时钟白字清晰；
      设置页状态栏最暗像素 (100,99,102) 压浅底，深色图标可读。
      回归测试 `DrawerSheetInsetRobolectricTest`（1 例，去掉内缩即红）、
      `SystemBarIconInstrumentedTest`（1 例，读 `isAppearanceLightStatusBars`）。

### 10.3 设置页 CJK 提示重复且误报缺失

- [x] 度量：预览截图 `03-settings.png` 上「未检测到 CJK 备用字体」出现两次。
- [x] 定根因：同一事实有两个发射点——`SystemFontSelector` 与 `FontInfoSection`
      各自发一次；且原生尚未上报时占位数据伪造成 `NONE`，把「还没结论」说成
      「确实缺失」。
- [x] 修根因：警告的唯一发射点收敛到 `CjkFallbackStatusLine`（状态行与警告同出一处）；
      占位数据改用新增的 `NOT_REPORTED` 状态（原生不产出该状态，解包时拒绝出现）。
- [x] 复测：预览截图上该行变为「CJK 后备已跳过（系统语言不是 CJK，未装语言回退族）」
      ——与原生判据同源，且只出现一次。回归测试
      `CjkFallbackStatusRobolectricTest`（4 例）。

### 10.4 存储权限未显式声明

- [x] 定根因：应用清单**一个存储权限都没声明**；合并清单里的
      `READ/WRITE/MANAGE_EXTERNAL_STORAGE` 全部来自 leakcanary 传递依赖——
      声明是偶然的，不是意图的。
- [x] 修根因：清单显式声明 `WRITE_EXTERNAL_STORAGE` 与 `MANAGE_EXTERNAL_STORAGE`
      （与上游 termux-app 一致）。`targetSdk` 由规范固定为 28，故
      `WRITE_EXTERNAL_STORAGE` 安装即授予；`MANAGE_EXTERNAL_STORAGE` 是用户在
      系统设置里自行打开的「所有文件访问」开关。两者都**不在运行期索要**
      （规范要求「声明文件权限（不提示，允许用户自行授权）」）。
- [x] 复测：仪器测试 `StorageAccessInstrumentedTest`（2 例）——合并清单带声明，
      且应用进程内 `/sdcard` 可列可写（`external_storage_root_is_reachable_when_authorized`
      0.017s 通过）。不新建 `~/storage` 软链：凭空造目录属未定义行为。

### 10.5 可执行文件经文档提供者修改后丢失权限

- [x] 度量：红测——复制后执行位从 `rwxr-xr-x` 变成 `rw-r--r--`。
- [x] 定根因：`copyTreeInto` 用 `File.copyTo` 造新文件，按 umask 给默认权限，
      执行位必然丢失。
- [x] 修根因：复制后按源文件的权限位恢复（`Files.setPosixFilePermissions`）。
- [x] 复测：`DocumentsProviderTest` 72 例全绿（含「write cycles must not strip
      permission bits」）；仪器测试走真实跨进程 `ContentResolver.call` 路径，
      22 例全过。

## 十一、三轮独立审阅

审阅按下列三种视角各做一次，每次只修真实问题；三次都无新问题方可宣告完成。

- [x] 证据核对：逐节对照「度量 → 根因 → 修 → 复测」，找假证据与漏测。
- [x] 全仓语义审阅：边界、并发、生命周期、错误路径。
- [x] 门禁与规范复核：全仓门禁、openspec 规范与实现的一致性。

### 11.1 证据核对视角的发现与处置

- [x] 假证据（§4）：台账称三件度量含「fling 轨迹是否推进」，并给出「38 个不同偏移 /
      单步 ≤3 行 / 总行程 60 行」。实际 `ScrollBehaviorQuantifiedTest` 只有三个用例
      （回车归位 / 洪流塌陷 / 拖动期帧节奏），全树无任何 fling 轨迹度量，那组数字
      没有任何断言能产生。改为真实的三个用例名，并把那组数字明确标注为「一次性探针
      数据，无自动化钉」。
- [x] 假证据（§8·附）：台账称 `openspec validate --all --strict` 34/34 通过。写下时确为
      34/34，但 §9.3 与 §11.1 新增/改写的两条 Requirement 正文超 500 字符，strict 下
      判失败（33/35）。已把两条拆成「Requirement + Scenario」并把平台几何下沉到
      Scenario，拆分后 36/36 全绿；台账改为记拆分后的现状。
- [x] 假证据（§7）：台账称「断言每张 PNG 为 1080×2400」。代码只断言颜色数 ≥ 16，
      尺寸仅打日志（分辨率由设备决定）。台账改为「断言颜色数 ≥ 16；尺寸只记录不断言」。
- [x] 台账与代码不一致（§11.1）：测试例数写反（称 Launcher 2 例 / Splash 3 例且
      monochrome 在 Splash）。实际 Launcher 3 例（含 monochrome）、Splash 2 例，已改。
- [x] 过期数字（§6·附 / §8·附）：`DocumentsProviderTest` 70 → 72 例；鼠标上报「四例」
      改为「整类 4 例」。
- [x] 漏测（§1）：原生预装载曾被整段删掉而没有任何用例变红。已在 `NativeBridge` 记录
      装载线程名，并加仪器测试 `NativePreloadInstrumentedTest` 钉住「库由
      `native-preload` 线程装载，不得落在主线程」。

### 11.2 语义与边界视角的发现与处置

- [x] H1（主线程被整帧 GPU 工作持有的全局渲染状态锁挡住）：`setRenderPaused` 原生只设
      一个全局标志、并不用会话 id，而 Kotlin 侧按会话逐个调用——每次都要抢锁，且
      **零会话时标志永远清不掉**（下一个会话渲染纯黑）。改为：原生导出去掉未用的
      会话 id，Kotlin 只调一次；`Bridge` 的转发层同步删除。
- [x] H2（持 `sessionLock` 时做最长约 1 秒的阻塞式 VT 标题查询）：`updateState()` 改为
      不再取标题（只做 CAS 状态更新），标题改由 `refreshActiveSessionTitle()` 在**锁外**
      刷新（会话切换后与关闭后立即刷新，另有渲染线程侧 500ms 周期刷新兜底）。
- [x] H3（停泵与启泵分处两条线程 → 恢复后留下永不退出的重复推进泵）：停泵移入
      `surfaceTransitionExecutor`，与「暂停块末尾的启泵」严格有序。
- [x] M1（被丢弃的输入返回成功）：非 Enter 的输入在等待进程结束期间被丢弃，如实返回
      `false`，不再把静默吞掉的输入报告为已写入。
- [x] M2（移到同一目录被改名）：同目录移动原样返回源 docId，不做去重改名。
- [x] M3（复制符号链接后相对目标失效）：复制链接时按新位置重算相对目标；新父目录仍在
      原树内则保持相对，否则写绝对路径，两种情形都仍指向原来那个文件。
- [x] M4（`closeNotifyThread` 永不退出）：`shutdown()` 里 `quitSafely()` 并置空，下一次
      使用时重建。
- [x] M5（`Bridge` 的可变去重缓存非线程安全）：改为原子引用整体替换（路径列表用一个
      引用，键与族名用一对值的引用，避免撕裂）。
- [x] M6（原生库缺失时 `UnsatisfiedLinkError` 逃出回调）：`searchQueryMaxLength` 加
      装载守卫与 `runCatching`（`UnsatisfiedLinkError` 是 `Error` 不是 `Exception`）。
- [x] M7（`pack_font_info` 的 u16 计数静默截断）：改用 `u16::try_from` 明确报错，回绕不再
      静默发生（回绕会让 Kotlin 侧校验抛异常，表现为设置页字体信息崩溃）。
- [x] L1（`start()` 失败未清 `pendingSurface`）：失效分支置空，避免算出与任何真实
      Surface 都不匹配的网格并多推一次 SIGWINCH。
- [x] L2（兜底推导的行列可能溢出成 `Int.MAX_VALUE`）：给推导值加上界。
- [x] L3（`RenderPauseLedger` 未声明线程约束）：标 `@MainThread`。
- [x] L4（锁内两次 join 的上限未写明）：在注释里写明上限。

### 11.3 门禁与规范复核视角的发现与处置

- [x] 门禁未校验 openspec 规范（`scripts/check.nu` 全文无 openspec 调用，而台账却把
      「openspec validate」写进门禁清单）：已在门禁里加入 `openspec validate --all
      --strict`——这正是它能悄悄漂移的原因。
- [x] P4（系统栏图标明暗有两个写入者，被遮住的终端页也在写）：`TerminalScreen` 仅在
      没有覆盖层时才写系统栏；并加仪器测试「设置页打开后终端页重组不得改写系统栏」。
- [x] P5（复制保留 POSIX 权限位的规范比实现宽——符号链接未搬权限）：Linux 上符号链接
      权限位恒为 0777、`lchmod` 不支持，规范收窄为「普通文件与目录」并写明原因。
- [x] P6（台账章节编号倒置、proposal 的 What Changes 与实际章节脱节）：章节重排为
      一…十、十一（独立审阅）；proposal 按实际章节重写。
- [x] P7（上一轮遗留的 change 未归档、且有一条 `[~]` 不是合法勾选标记）：`[~]` 改为
      正常文字行，避免与 `- [x]/- [ ]` 混用造成计数歧义。
- [x] P8（proposal 的前提写「六条现象」，而当前 `docs/STATUS.md` 是 5 条）：proposal
      改为同时交代两版清单，并写明「STATUS.md 的 5 条已全部由本变更覆盖，请维护者勾销」
      ——`docs/STATUS.md` 属禁改清单，本变更不改它。
- [x] P10（门禁不 lint Android 侧原生代码）：已在 `scripts/check.nu` 加入
      `cargo ndk -t arm64-v8a clippy --all-targets -- --deny warnings`——约 1100 行
      `cfg(target_os = "android")` 代码此前从不被任何 lint 看到；当前代码在 Android
      目标上零告警。
- [x] P9/P11（门禁不跑仪器测试、APK 含 `libnative.so`）：由 `.github/workflows/build.yml`
      覆盖——`scripts/build-apk.nu` 先构建双 ABI 原生库再打包，模拟器作业运行
      `scripts/test-emulator.nu`。`check.nu` 面向无设备的本地门禁，不重复这两步。
- [x] D5（存储权限的显式声明没有任何 openspec 覆盖）：新建 `openspec/specs/storage-access/`
      记「应用 MUST 显式声明外部存储读写权限」及三条 Scenario。
- [x] D3 的两处缺口（CJK 状态无「唯一发射点」约束、缺席原因次序里没有「尚未上报」）：
      已在 `cjk-rendering` 补齐。
- [x] S2（`native/src/lib.rs` 的模块文档指向不存在的 `tests/bdd/`）：改为实际路径
      （Cucumber 套件在 `android/app/src/androidTest/assets/features/`）。

### 11.4 全仓语义与边界审阅的发现与处置

- [x] H1（`setRenderPaused` 按会话逐个调用，而原生侧只设一个全局标志）：原生导出
      去掉未用的会话 id，Kotlin 侧改为单次调用。零会话时暂停标志同样能落下
      ——原先经 `onSession`（要求活会话）调用，会话全关时标志永远清不掉，
      下一个会话渲染纯黑。
- [x] H2（`updateState()` 在持有 `sessionLock` 时做阻塞式 VT 往返取标题，最长约
      1 秒）：标题查询移出锁——`updateState()` 只做纯状态更新，标题由
      `refreshActiveSessionTitle()` 在锁外刷新（会话切换与关闭后立即刷新，
      渲染线程另有周期刷新）。
- [x] H3（停泵在调用线程、启泵在执行器 → 竞态，恢复后留下与渲染线程重复推进
      会话的泵，且该泵永不退出）：停泵移入 `surfaceTransitionExecutor`，与「块 A
      末尾的启泵」严格有序。
- [x] M1（`writeToPty` 对被丢弃的输入返回 `true`，把静默吞掉的输入报成成功）：
      非 Enter 的输入返回 `false`。
- [x] M2（`moveDocument` 把文件移入其所在目录会改名成 `name (1)`）：同目录移动
      原样返回源 docId。
- [x] M3（复制符号链接时原样保留相对目标，换目录后链接语义被破坏）：按新位置
      重算相对目标；目标在新父目录之外时写绝对路径。
- [x] M4（`TerminalDocumentsProvider` 的写回通知线程永不退出）：`shutdown()` 里
      `quitSafely()` 并置空，下一次使用时重建。
- [x] M5（`Bridge` 的两个可变缓存字段非线程安全，去重失效会重建字体管线）：
      改用原子引用整体替换。
- [x] M6（原生库缺失时 `searchQueryMaxLength` 的 `UnsatisfiedLinkError` 逃逸崩进程）：
      先判 `isNativeLoaded()`，再 `runCatching` 兜底。
- [x] M7（`pack_font_info` 的 `u16` 计数静默截断，回绕会让设置页崩）：
      改用 `try_from` 明确报错。
- [x] L1（`start()` 失败路径未清 `pendingSurface`，会把错误网格下发给 PTY）：
      失效分支清空。
- [x] L2（兜底推导的行列可能饱和到 `Int.MAX_VALUE`）：加上界。
- [x] L3（`RenderPauseLedger` 未声明线程约束）：标注 `@MainThread`。
- [x] L4（`activateReplacementSession` 在锁内做两次各 1 秒的 join）：注释写明
      这一上限。
- [x] 补充回归：原生预装载此前没有任何用例会变红（该修复曾被整段删掉）。
      `NativeBridge` 记录装载线程名，新增仪器测试断言「原生库由 `native-preload`
      线程装载、不得落在主线程」。
- [x] 补充（`docs/persistence/libghostty.md` 四次复核的唯一成对缺口）：模式 2026
      此前只写不读（`GridSnapshot.sync_active` 零消费）。改为帧发布层门控——VT 循环
      在保持期间不发布 CellData，ESU 后发布最终帧；1s 超时强制解锁，避免程序崩溃
      永久冻结。删除 `sync_active`，新增两个回归测试；规范见 `render-stability`
      的「同步输出期间的帧发布门控」。

### 11.5 重启后的补充修复（2026-10-21 晚）

- [x] 标题刷新仍在**调用线程**做最长 1 秒的阻塞 VT 往返：`refreshActiveSessionTitle()`
      在 `switchSession`/`closeSession` 中被直接调用，而两者都可能跑在主线程；且它
      在锁外读普通 `HashMap` 的 `sessions`。改为 `scheduleActiveSessionTitleRefresh()`：
      锁内只取 `(activeSessionId, bridge)`，查询放 `scope`（IO），写回前按会话 ID 校验；
      活动会话退出并接替时同样刷新。回归：会话切换后标题跟着换、旧查询不覆盖新会话。
- [x] 原生预装载**不确定**：`TerminalApp` 只是起线程，主线程仍可能先触碰
      `NativeBridge`，471ms 的 `.so` 读取就回到主线程（装载线程名也不再是
      `native-preload`，仪器测试会红）。改为文件级 `CountDownLatch`：预装载线程进入
      类初始化即释放，主线程最多等一次调度（2 秒上限，超时大声告警），不等装载完成。
- [x] 门禁偶发（非代码问题）：`lintAnalyzeDebugUnitTest` / `lintAnalyzeDebugAndroidTest`
      曾以 `Unexpected failure during lint analysis ... this is a bug in lint` 崩溃；
      单独复跑 `:app:lintDebug` 即 EXIT=0。记录为 AGP/Kotlin FIR 的工具链缺陷，
      不以关闭规则或跳过 lint 的方式掩盖。
- [x] 全仓重复率与静态扫描（`npx jscpd --min-tokens 60`、`npx aislop@latest scan`）：
      重复行 1.66%（1371/82475），无 AI-slop/安全/lint 发现；仅有的 14 条
      code-quality 提示是「文件/函数过长」的启发式阈值，拆分属于无行为收益的搬迁，
      不做。
