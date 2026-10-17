# 任务

顺序固定为 `STATUS.md` 的行序，**严格串行**：每条先读代码与真机日志定根因，
再改代码、加可运行的检查、更新 spec、一次提交。未定根因前不动代码。

## 一、Shell 启动前偶发黑屏（性能）

- [x] 定根因（真机日志实证，`native/target/logcat-1.txt` 时间线：进程启动
      `19:02:35.062` → 首帧 `19:02:35.915`
      `W/Runtime: SLOW_FRAME session=1 render=58.840923 count=1`，相隔约 0.85s）：
      系统启动屏的默认撤下时机是**内容视图首次预绘制**，而原生渲染器此刻还没向
      Surface 提交过任何缓冲；`MainActivity` 又把窗口底色置为
      `ColorDrawable(Color.TRANSPARENT)`，于是 SurfaceView 的**开孔区**在首帧到达前
      透出黑底。首帧与预绘制谁先到取决于 GPU 初始化与 Compose 首绘的相对速度，
      故表现为「偶发」。此前已存在的 `Theme.Terminal.Splash` 只覆盖到预绘制那一刻，
      够不到这 0.85s。
- [x] 逐条排除另两类解释：
      ① `W/TerminalSurface: applySurfaceResize: surface not valid yet, deferring`
      （35.391，早于 GPU 初始化 35.460/35.521）只是「Surface 未就绪即 resize」，
      尺寸由 `surfaceCreated` 末尾的 `applyPendingSurfaceResize()` 重放，
      日志中其后无任何 resize 失败或网格异常；
      ② `E/SurfaceSyncer: Failed to find sync for id=0`（35.407）与
      `E/mali_gralloc: Requested R8 format is not supported…`（35.815，×4）
      都属厂商侧：后者是系统启动屏自身图标动画的单通道缓冲回退（格式 0x38），
      本应用 Surface 走 `holder.setFormat(PixelFormat.RGBA_8888)`，
      且紧接着 35.915 就正常出帧；
      ③ 「GPU 初始化阻塞主线程」不成立：`native::render::wgpu_backend` 日志的
      tid 是 12237，同期主线程（12189）继续跑窗口与主题日志，全程无 ANR、
      无 Skipped frames。
- [x] 按根因修：`MainActivity` 用 `installSplashScreen()` 返回的对象设
      `setKeepOnScreenCondition`，判据是终端首帧是否已呈现
      （`TerminalRuntime.hasPresentedFrame`，由渲染线程在 `count > 0` 时置位——
      空闲帧 `render_inner` 返回 0、失败帧为负，故 `> 0` 即「本帧真的渲染并提交了缓冲」）；
      配 5s 上限，使引导未装/会话起不来时不会永久遮挡界面（判据抽为纯函数
      `shouldKeepSplashOnScreen`）。闭包在首次预绘制时才求值，故读 `runtime` 晚于 Hilt 注入。
- [x] 加检查（`ShouldKeepSplashOnScreenTest`）：`keepsUntilFirstFramePresented`、
      `givesUpAtDeadline`——首帧未呈现则遮挡、呈现即撤下、上限到点必撤。
- [x] spec：`openspec/specs/render-stability/spec.md` 的「启动首帧及时呈现」下补
      Scenario「启动屏遮挡到终端首帧呈现」（本变更不放同名 delta，避免归档时重复添加）。
- [x] 验证：`./gradlew :app:testDebugUnitTest --tests "terminal.emulator.ShouldKeepSplashOnScreenTest"`
      → 2 tests / 0 failures / 0 skipped；`openspec validate --specs` → 31 passed / 0 failed；
      markdownlint 0 issues。

## 二、文档提供者三项（无法创建文件/文件夹「路径已存在」；复制/移动「路径已存在」；无法保存修改文件「超时」）

- [x] 定根因（真机日志实证，`native/target/logcat-1.txt` 两条未捕获
      `java.lang.RuntimeException: java.io.IOException: Failed to delete
      '/data/user/0/com.termux/files/home/EXECUTABLES.txt' (exists=false, …)`
      出自 `DocumentMutations.deleteWithoutFollowingSymlinks`）：
      ① 删已不存在的目标抛裸 `IOException`，跨 Binder 被
      `DocumentsProvider.call` 包成 `RuntimeException`，客户端只能报整次操作失败
      ——「覆盖」＝先删再拷，幽灵行（终端侧 `rm` 后在客户端缓存里）必然走到这里；
      ② `renameDocument` 对重名硬失败，而框架注释明确允许提供者「为避开重名改写名字」，
      「先新建、后改名」的客户端（建文件/建文件夹、另存为）因此报「路径已存在」。
- [x] 修 `deleteWithoutFollowingSymlinks` / `deleteDocument`：抽 `deleteOrFail`，
      已不存在即成功；判存在用 `LinkOption.NOFOLLOW_LINKS`（悬空链接仍算存在，
      MUST NOT 被误判成已删除）；真删不掉抛 `FileNotFoundException`（框架只原样转交这一种）。
- [x] 修 `renameDocument`：重名改走与 create/copy/move 同一 `uniqueChild`；
      名字未变时按框架约定返回 `null`（`renameDocument` 返回类型随之可空）。
- [x] 加检查（`DocumentsProviderTest`）：`deleteDocument_of_missing_document_is_a_no_op`、
      `deleteDocument_of_dangling_symlink_removes_the_link`、
      `renameDocument_conflict_appends_suffix_like_termux`、
      `renameDocument_with_unchanged_name_is_a_no_op`。
- [x] 验证：`./gradlew :app:testDebugUnitTest --tests "terminal.emulator.DocumentsProviderTest"`
      → 61 tests / 0 failures / 0 skipped；`spotlessCheck`、`detekt` 通过；
      `openspec validate --specs` → 31 passed / 0 failed；markdownlint 0 issues。
- [x] 「无法保存修改文件：超时」：写回通道已由 `openDocument` 单测
      （`w` 截断 / `wa` 追加 / `rw` 创建 / `rwt` / 未知 mode 回退 / 关闭时广播）
      与真机向的 `DocumentsProviderInstrumentedTest#external_process_write_back_lands_on_disk`
      覆盖；两份真机日志 grep 不到 timeout/ANR 证据。按客户端侧现象登记，
      不臆造兜底代码；复现需要客户端名与新日志。
- [x] 排除项（避免下次重查）：客户端观察的可能不是树 URI 形式——
      AOSP `DocumentsUI` 的 `CopyJob` 与 `AbstractActionHandler` 都用
      `buildChildDocumentsUri(authority, getDocumentId(dirDocUri))` 这一**无树前缀**
      形式注册观察者，与本提供者 `notifyChildren` 的形式一致，故通知未被漏送，
      不需要「两种形式都发」的兜底。
- [x] spec：`openspec/specs/documents-provider/spec.md` 直接补入
      「删除按幂等语义覆盖」「重名冲突在全部变更入口取唯一名」「重命名到自身是空操作」
      三个 Requirement 与六条 Scenario（本变更不放同名 delta，避免归档时重复添加）。

## 三、会话 `exit 0` 时不关闭会话/应用

- [x] 读会话生命周期并与 termux-app 对照，判定是设计选择还是漏实现：
      **退出码语义已对等，缺的是「会话表清空后关闭应用」**。
      `TerminalRuntime.maybeShowProcessCompletedPrompt` 对 `exitCode == 0` 返回 false，
      `handleSessionExit` 随即移除会话（DESIGN.md:145「正常退出时关闭会话」已实现）；
      termux 侧 `TermuxTerminalSessionActivityClient#onSessionFinished` 对退出状态 0/130
      走 `removeFinishedSession`，而该方法在 `service.getTermuxSessionsSize() == 0` 时
      调 `mActivity.finishActivityIfNotFinishing()`——**这一步 torvox 从未实现**
      （全仓无任何 `finish()`）：会话没了，应用却停在无会话的空界面，
      用户看到的正是「exit 后什么都没关」。
- [x] 不照抄 termux 的「130 也自动关闭」：两者的退出码空间不同——termux 把信号死亡记为
      **负值**（`-WTERMSIG`），130 是真实的退出码；torvox 按 shell 惯例记为
      `128 + 信号号`（`native/src/terminal/session.rs` 的 `SIGNAL_EXIT_BASE`），
      故 130 在 torvox 里就是 SIGINT 死亡，属于 DESIGN.md 要「保留现场」的崩溃。
      照抄会把被 Ctrl-C 杀死的 shell 静默关掉，与 termux 对信号死亡的处理（保留提示）相反。
- [x] 修：`MainActivity` 在 `lifecycleScope` 里收集 `runtime.state`，会话表由非空转为空
      即 `finish()`（termux 的 `finishActivityIfNotFinishing` 对等；Activity 不在时
      无事可做，会话表空即无前台服务）。判据抽为 `SessionTableWatcher`：
      冷启动时 state 先以空表发布（会话由运行期异步创建），直接判空会在启动瞬间关掉应用，
      故必须记住「曾有过会话」，并只触发一次。
- [x] 加检查（`SessionTableWatcherTest`）：`coldStartWithNoSessionsYetDoesNotFinish`、
      `lastSessionClosedFinishesOnce`、`backgroundSessionStillCountsAsASession`
      —— 分别钉住冷启动不误关、清空即关且只关一次、多会话时最后一个消失才关。
- [x] spec：`openspec/specs/shell-entry/spec.md` 的「非正常退出保留现场」Requirement 补
      「最后一个会话消失时关闭应用」，并加 Scenario「最后一个会话结束后关闭应用」
      （含冷启动空表不得触发的断言）。
- [x] 验证：`./gradlew :app:testDebugUnitTest --tests "terminal.emulator.SessionTableWatcherTest"`
      → 3 tests / 0 failures / 0 skipped；spotlessCheck、detekt 通过；
      `openspec validate --specs` → 31 passed / 0 failed；markdownlint 0 issues。

## 四、会话列表项未显示会话标题

- [x] 定根因（真机实证，`test_avd` Android 15 + `adb`）：标题**管道本身是通的**——
      在会话里跑 `printf '\033]2;LIVETITLE\007'` 后打开抽屉，条目里确实出现了
      `LIVETITLE`（uiautomator dump：`SessionItem` 高 178px，两个 TextView
      `会话 1` / `LIVETITLE`），说明 OSC 0/2 → ghostty → JNI 批量取回 → 强制刷新
      全链路无误。问题在**条目版式不合规范**：主文案是占位串「会话 N」，标题被降级
      成第二行 12sp 小字；DESIGN 的示例是 `[1] ~`（序号与标题同行），termux 在无会话名
      （用户新建会话没有 `mSessionName`）时也是 `numberPart + sessionTitlePart` 同行
      （`TermuxSessionsListViewController.getView`），只有存在会话名时才换行。
      torvox 没有会话名，因此标题必须跟在序号后面——现状既丢了规范里的序号格式，
      又让标题退成次要信息，读起来就是「没显示会话标题」。
- [x] 按根因修：`SessionDrawer` 条目改为单行 `[序号] 标题`，序号加粗、标题斜体
      （`AnnotatedString` 分段样式，与 termux 的 bold/italic span 一致）；标题为空时
      只留序号，不占第二行、不用占位文案顶替。序号格式落 `R.string.session_index`
      （`[%1$d]`），与 app bar 仍在用的「会话 N」区分开。
- [x] 加检查：`SessionItemLabelTest`（`numberAndTitleShareOneLine`、`titleIsItalic`、
      `emptyTitleLeavesNumberOnly`）断言标题与序号同行、样式分段正确；
      `SessionDrawerNarrowTest.narrow_drawer_session_item_renders` 在 Robolectric 里断言
      条目**整条文案**为 `[1] VT_TITLE`（旧版两行渲染下该节点不存在，故非空断言）；
      `SessionDrawerInstrumentedTest.sessionItemShowsNumberAndTerminalTitleOnOneLine`
      用 `feedTerminal` 直写 OSC 2 走完整管道（不依赖 shell 就绪）。
- [x] spec：`openspec/specs/session-list/spec.md` 的「会话条目内容」改为单行
      `[序号] 终端标题` 口径，Scenario 同步为「未设置标题时仅显示序号」「设置标题后
      抽屉显示（同一行斜体）」。
- [x] 验证：`./gradlew :app:testDebugUnitTest --tests "terminal.emulator.ui.SessionItemLabelTest"
      --tests "terminal.emulator.ui.SessionDrawerNarrowTest"` → 6 tests / 0 failures；真机（重装 debug APK 后）uiautomator dump 显示
      `TextView text="[1] LIVETITLE"`（单行，条目高 126px，改前为两行 178px）；
      `openspec validate --specs` → 31 passed / 0 failed；markdownlint 0 issues。
- [x] 仪器测试本机跑不动，如实记录：`connectedDebugAndroidTest` 两次都在**用例执行前**
      失败（`UiAutomationService … already registered!` / `Error granting runtime permission`，
      本机 mobile-mcp 占着 UiAutomation），**既有用例 `addSwitchAndCloseSession` 同样失败**，
      与本次改动无关；新增的 `sessionItemShowsNumberAndTerminalTitleOnOneLine` 只做了编译
      验证（`:app:compileDebugAndroidTestKotlin`），留给 CI 的干净环境执行。

## 五、搜索匹配无高亮、关闭搜索后反色未消失

- [x] 定根因：命中区间如何传给渲染、反色状态在何处设置与清理。
      「无高亮」不成立：`applySearchHighlights` 把 `search_highlights.session` 设为真会话号，
      下一帧 `enter_session` 判定为同一会话（空操作），命中行照常标脏并画入累积纹理；
      真机像素解码可见命中块。「反色未消失」成立且根因在清理路径：`clearSearchHighlights`
      把 `session` 置 0，下一帧 `enter_session(1)` 走「会话切换」分支，把 `current` 与
      **`drawn`** 一起清空，于是 `highlight_rows_to_mark` 无行可标脏，部分帧只重画光标行，
      旧高亮留在累积纹理上。附带缺陷：计数把「上游尚未选中」渲染成「第 1 个」。
- [x] 修高亮与反色清理。`SessionScoped::enter_session` 会话切换时只清 `current`、保留
      `drawn` 一帧并写入真会话号；`clearSearchHighlights` 不再动 `session`；计数改
      `currentResultIndex: Int?`，未选中时报 `共 N 个`。
- [x] 验证：`text-search` / `text-selection` 相关 spec 与检查同步更新。
      `text-search` 补「置脏须配合上一帧已画出的高亮行记录」与计数口径；原生新增
      `search_highlight_repaint_tests::discarded_highlights_stay_dirty_for_one_frame`
      （已确认对旧行为失败）；Kotlin 新增 `TextSearchBarCounterRobolectricTest`（3 条）。
      真机：搜索 `MARKER` 得 3 命中且三行反色，关闭搜索后反色立即消失。

## 六、简体中文退格缓慢

- [x] 定根因（判别实验，非推断）：同一台模拟器上交替采样「输入法已请求光标监视」
      与「未请求监视」两组（`CjkComposingLatencyTest`，20 样本/组，grow → sleep 120ms →
      timed retract）：**开启**时回退一键 p50 17.96ms / p95 38.55ms，**关闭**时
      p50 0.19ms / p95 0.54ms。差距即锚点回报里的同步 VT 查询等待——组字逐键发生，
      每一次都落在主线程上（≈1–2 帧）。
- [x] 根因二（同一次采样顺带实证）：每个样本都以
      `IllegalArgumentException: Coordinate transformation matrix is required when
      positional parameters are specified.` 收场（`CursorAnchorInfo$Builder.build()` ←
      `reportComposingCursorAnchor`）。锚点**一次也没送到**输入法，而每次组字都要付一次
      异常构造。AOSP（android15-release）`CursorAnchorInfo.Builder.build()` 在给出任何
      定位参数而未给矩阵时必抛。
- [x] 修（根因一）：锚点坐标改取**渲染线程已消费的那一帧**的光标。native 在渲染阶段 2
      消费帧处把 `CursorInfo` 打包发布到会话条目上的 `AtomicI64`
      （`SessionEntry::rendered_cursor`，`Release` 写 / `Acquire` 读），新增 JNI
      `getRenderedCursorPacked`：一次原子读，不取会话锁、不发查询。口径不变——与同步
      查询同出 `cursor_in_viewport`，隐藏与滚出视口都表现为「无光标」；同步查询
      `getCursorViewportPacked` 保留给「刚写进去的数据立刻反映」的断言。
- [x] 修（根因二）：锚点补 `setMatrix(视图局部 → 屏幕)`（视图当前屏幕位置，不缓存成
      常量——输入法弹出时本视图被自身 `translationY` 上移，`getLocationOnScreen` 每次都
      算进去）。
- [x] 加检查（native 单测）
      `session_registry_tests::rendered_cursor_reads_published_cell_and_separates_unknown_session`：
      未发布 → 无值、可见格 → 该格、隐藏格 → 无值、未登记会话 → `None`（「未知会话」与
      「已知会话但暂时无光标」必须分得开）。
- [x] 加检查（仪器测试）
      `CjkComposingLatencyTest#composing_backspace_does_not_wait_on_cursor_anchor_query`：
      监视开/关两组的 p50 差不超过 8ms 预算。非恒真条件齐备——组字字节必须落格
      （否则样本无效）、每次调用必须受理且不以异常收场、帧内光标必须取得到
      （否则锚点整段跳过、两组都「漂亮」）。修正后两轮实测：on p50 0.36/0.47ms、
      p95 0.93/0.67ms；off p50 0.19/0.16ms。
- [x] spec：`openspec/specs/ime-editor-contract/spec.md` 的「组字光标位置回报给输入法」
      改写坐标来源与矩阵要求（原「视口坐标是一次同步 VT 查询…故查询 MUST 限频」的口径
      连同护栏一并更正），并补三个场景：逐键回报不等待 VT、锚点携带局部到屏幕的矩阵、
      帧内光标未就绪时不回报。
- [x] 验证：`./gradlew :app:connectedDebugAndroidTest
      -Pandroid.testInstrumentationRunnerArguments.class=terminal.emulator.ui.CjkComposingLatencyTest`
      → `BUILD SUCCESSFUL`，指标 `composing_edit_{on,off}_{p50,p95}_us` 见上；
      `cargo test --package native --lib -- rendered_cursor` → `1 passed`。
- [x] 证据留档：修正前的两组耗时与异常文本取自 `/tmp/cjk-prefix6.log`（同一用例、同一
      环境，仅坐标来源未改），修正后取自 `/tmp/cjk-fixed1.log`、`/tmp/cjk-fixed2.log`。

## 七、文本选择不同行间有空隙、卡顿、不跟手

- [ ] 定根因：选择坐标取整/节流/重绘范围。
- [ ] 修。
- [ ] 验证：`text-selection` spec 与检查。

## 八、输入法弹出时上滑不能到顶部

- [ ] 按 `docs/specification/TESTING.md:36-38` 的验收口径（弹出时内容上移、
      底部像素不动、无闪烁撕裂卡顿）定根因并修。
- [ ] 验证：可运行检查。

## 九、上下滑动到边界跳动/闪烁

- [ ] 定根因：回滚缓冲边界处的重绘。
- [ ] 修。
- [ ] 验证：可运行检查。

## 十、滑动未按像素滚动

- [ ] 定根因：滚动手势与滚动位置的换算。
- [ ] 修。
- [ ] 验证：可运行检查。

## 十一、启动动画用图标而非完整图案

- [ ] 按 `docs/specification/DESIGN.md` 的启动画面约定定差异。
- [ ] 修。
- [ ] 验证：可运行检查。

## 十二、收尾

- [ ] 全仓门禁 `TMPDIR=/tmp ./scripts/check.nu` 一次性跑通，逐阶段核对
      （该脚本不因外部命令失败而中断，须逐段确认）。
- [ ] 逐条回读 12 条，确认验收口径被测试或 spec 覆盖；仍依赖真机的项写明缺口。
- [ ] 连续三轮审阅无问题后归档本变更（归档前不删 `STATUS.md` 条目，需维护者点头）。

## 十三、CI 失败用例甄别（`run 38888880442`，不属于 STATUS 行序）

定时跑失败的两条不在 STATUS 的 11 条里，一并按根因-修复收口。

- [x] `FontFileOverrideInstrumentedTest#termux_font_file_becomes_primary_font`：
      本机原样复跑即复现（`AssertionError: Roboto 为比例字体：主字体切换后等宽标记必须
      翻转为假`），是用例自身的前提过期而非 CI 抖动。把两侧 `FontInfo` 写进失败消息后
      取到实证：两侧**逐字段相同**（`activeName=Roboto, activeMonospaced=false,
      cellWidthPx=12.420898, cellHeightPx=17.0, fontSizeSp=14.0`）。根因是基线读晚了：
      `loadFontFile` 当场就 `rebuild_font_pipeline` 再 `set_font_family`
      （`native/src/android/ffi.rs`，`cjk-rendering` spec 早已记「`loadFontFile` 在重建后
      再设字体族」），而用例把「默认基线」取在该调用**之后**，于是
      `default.activeMonospaced` 已经是 Roboto 的 `false`，两条判据（等宽翻转为假、
      单元格度量变化）恒假。修：基线移到 `loadFontFile` 之前读，并在注释里写明为何
      不能再挪回去。复跑该用例 `tests="1" failures="0"`。
- [x] `BehaviorInstrumentedTest#behavior_settings_shell_entry_empty_until_saved`：
      本机复跑通过（`tests="1" failures="0"`），整套 `BehaviorInstrumentedTest` 也通过
      （`tests="10" failures="0"`），即 CI 上的间歇失败，无确定性可复现 bug。失败面是
      `TestUtils.openSettings()` 用尽重试梯子后由 `waitForSettingsScreen(60_000)` 抛出的
      裸 `Condition still not satisfied after 60000 ms`；梯子里每一击的错误都被
      `runCatchingCancellable` 吃掉，故消息不含任何线索（隐藏错误）。按「不得隐藏错误」
      补两处：① 权限对话框清理抽成 `dismissBlockingSystemDialog`，重试梯子**每轮**都看
      一眼——它可能晚于入口那次清理才弹出，之后每一击都打在对话框上；② 兜底失败改写为
      `设置页始终未出现（前台包=…，系统对话框=…）`，下一次 CI 失败自带定性线索。本机复跑
      通过。
- [x] 空跑陷阱留档：`-Pandroid.testInstrumentationRunnerArguments.class=A#b,C#d`
      （多组「类#方法」逗号拼接）**不匹配任何用例**，任务照样 `BUILD SUCCESSFUL` 且报
      `tests="0"`。甄别时按每个用例单跑，并核对 JUnit XML 的 `tests="N"` 而不是只看
      Gradle 的退出码。
