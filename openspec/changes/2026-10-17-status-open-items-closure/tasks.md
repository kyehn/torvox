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
      调 `mActivity.finishActivityIfNotFinishing()`——**这一步本应用从未实现**
      （全仓无任何 `finish()`）：会话没了，应用却停在无会话的空界面，
      用户看到的正是「exit 后什么都没关」。
- [x] 不照抄 termux 的「130 也自动关闭」：两者的退出码空间不同——termux 把信号死亡记为
      **负值**（`-WTERMSIG`），130 是真实的退出码；本应用按 shell 惯例记为
      `128 + 信号号`（`native/src/terminal/session.rs` 的 `SIGNAL_EXIT_BASE`），
      故 130 在本应用里就是 SIGINT 死亡，属于 DESIGN.md 要「保留现场」的崩溃。
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
      本应用没有会话名，因此标题必须跟在序号后面——现状既丢了规范里的序号格式，
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
- [x] 门禁补正（首轮全仓 `check.nu` 抓到，见十二节）：
      `session_scoped_tests::entering_another_session_drops_the_previous_highlights`
      断言的正是本次修掉的旧行为（切会话时 `drawn` 一并清），随修复转红。核对该断言的
      依据「半清会被读成高亮未变」不成立：`highlight_rows_to_mark` 恒取
      `current ∪ drawn`，没有判重分支；真正吃这一口的是空闲路径的门控
      `needs_repaint = current != drawn`——清掉 `drawn` 会让它当场为假、空闲帧静默跳过，
      旧高亮像素留在累加器里（与本节实测吻合）。用例改钉新契约
      `entering_another_session_drops_current_but_keeps_drawn_for_one_frame`，
      并做反证：把 `self.drawn.clear()` 加回 `enter_session` 后该用例转红，复原后转绿。

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

- [x] 定根因：三个症状各自独立——
      ① 行间空隙：脏带清除用 `floor(start*H)` / `ceil(end*H)` 向外取整，多覆盖相邻行的
      边界像素；相邻行在部分帧里通常不在脏带内、本帧不重绘，那一像素就保留清除色
      （主题背景），在选区/搜索高亮这类整行着色里表现为 1px 细缝。模拟器实测
      `cell_height=44.625`，缝隙只出现在小数部分 > 0.5 的行边界——与修正前抓到的帧吻合
      （`/tmp/d03.raw`：x=700 处反色区块内三条 1px 背景色细线，间距 134px = 3×44.625）。
      ②③ 卡顿、不跟手：拖动途中的原生推送节流 50ms（≈3 帧），高亮最多落后手指 3 帧，
      快速拖动时肉眼可见。
- [x] 修：`native/src/render/pass.rs` 新增 `band_clear_pixel_range`——只清除**像素中心**
      落在该行跨度内的像素行（与单元四边形同一光栅化规则），`band_clear_instances`
      改用它；`TerminalViewModel.DRAG_NATIVE_SYNC_INTERVAL_MS` 50 → 16（一帧）。
- [x] spec：`openspec/specs/text-selection/spec.md` 的「长按启动选择」补一段
      「反色 MUST 连续覆盖选区内全部像素……脏带重绘的清除范围 MUST 与单元四边形的
      光栅化像素范围一致，MUST NOT 侵入本帧未重绘的相邻行」，并补「空行反白」
      「选区行间无缝隙」两个场景。
- [x] 验证：`cargo test band_clear` → `2 passed`（新增
      `band_clear_range_does_not_bleed_into_neighbour_rows`：h=44.625 时行 1 的清除范围
      必须是 `[45,89)`，旧公式为 `[44,90)`）；`cargo fmt --check`、`cargo clippy
      --all-targets` 干净；`./gradlew :app:spotlessCheck :app:detekt` `BUILD SUCCESSFUL`；
      `openspec validate text-selection --type spec --strict` → valid。
- [x] 回归：`native/src/terminal/ghostty_terminal/tests.rs` 补
      `selection_highlight_covers_blank_row_inside_range`（选区内空行与内容行同色）。
- [x] 证据留档：修正前帧 `/tmp/d03.raw` 取自旧 `.so`，修正后同配方帧里同列不再出现细缝，
      只是该次拖动选区较短（2 行），单个边界不足以定论，故结论以旧帧的实测签名
      （缝隙位置与取整公式的预测逐条对上）+ 单元测试为准。
- [x] 构建陷阱留档：`:app:assembleDebug` **不会**重编 Rust，`jniLibs/` 需先跑
      `nix develop --command nu scripts/build-android-libs.nu`，否则装机跑的仍是旧 `.so`
      （本次曾因此把旧行为误判成「修了没用」）。

## 八、输入法弹出时上滑不能到顶部

- [x] 定根因：位移（`computeImeSurfaceShift`）只按键盘遮挡与内容下沿算，与滚动位置无关；
      于是被位移裁在屏幕上边界之外的那几行（本机 818px ≈ 18 行）根本不在上滑行程里
      ——回滚区上滑到偏移 = 回滚长度即到底，「到顶部」在像素上不可达。
      另一条思路（按剩余回滚量钳制位移）已否决：回滚不足一屏时提示符会被键盘吞掉，
      撞 `TESTING.md` 第 37 条。
- [x] 修：新增「显出量」（`imeShiftRevealPx`）——越过回滚区顶端的越界位移按容量
      （= 当前完整位移）1:1 喂给显出量，Surface 实际平移 = 位移 − 显出量；反向拖动
      先按同一比例回收显出量再下滚（1:1，不出现 2 倍行程）。显出量上界取
      `min(容量, 偏移 × 行高)`，偏移归 0（实时视图）时恒为 0 ⇒ 提示符永远钉在键盘
      上方；键盘收起（容量 0）、会话切换、字号变化都由该式自动收敛，不需额外复位点，
      也不产生跳变（跳变正是第九条的抱怨）。
- [x] 加检查（`ScrollDistanceTest`，7 例）：显出、容量封顶、1:1 回收、超出显出后滚动，
      以及边界外丢弃 3 例（见第九条）。22 tests / 0 failures。
- [x] spec：`openspec/specs/ime-animation-smoothness/spec.md` 补 Requirement
      「位移裁掉的网格顶部仍可达」（两向 1:1、不得越过回底行程、不得触发网格重排）。
- [x] 验证（真机 emulator-5554，api 35）：`clear; echo TOPMARK-0; seq 1 120` 后弹键盘、
      甩到回滚区顶端——`TOPMARK-0` 完整可见；再收起键盘，同一区域
      y∈[180,420] 逐像素比对 `differing_px=0`，即弹键盘时那一带没有任何裁剪，与无键盘
      时逐像素一致。反向对照：实时视图下收起键盘同区域差 23529 px，证明位移真实存在、
      不是「容量恰好为 0」的假通过。
- 已知残余：快速甩到顶端时 `OverScroller` 只能停在 `scrollbackLen`，显出量仍为 0，
      最早一行仍被裁掉；慢拖或「甩到底后再拖」可达。归第十条的按像素状态统一处理。
      （第十条已收口：惯性改为按像素推进、上界含完整位移容量后，甩到回滚区顶端继续
      过冲的位移同样喂给显出量，一次甩动即可把显出量吃满。）

## 九、上下滑动到边界跳动/闪烁

- [x] 定根因（`applyScrollDistance`）：边界外的余量在**亚行早退之后**才清零——偏移已在
      边界（0 或回滚长度）却继续向边界外拖时，`rawAmount == 0` 先原样返回，余量随后
      经 `setScrollRemainderPx` 下发渲染器，渲染器按投影矩阵把整幅网格亚行平移，边缘
      露出一条背景空带；余量再攒够一整行时偏移无处可去、余量被清零，内容整体弹回一行
      ——「跳动 + 闪烁」。整行级越界此前已被钳制清零，故只在亚行量级复现。
- [x] 修：三处边界判定提到亚行早退之前，越界位移就地丢弃（`ScrollStep.overflowPx`），
      既不进渲染器余量也不跨行回弹：底部（偏移 0 且继续向更新方向拖）、顶部
      （偏移 = 回滚长度且继续向更早方向拖，改喂显出量）、顶端之外显不出的部分同样丢弃。
      同一处改动顺带把 `scrollOffset` 赋值收敛到 `setScrollOffset`（位移显出量依赖它）。
- [x] 加检查（`ScrollDistanceTest`）：`边界外位移就地丢弃`（顶部/底部对称两例）、
      `边界内的亚行跟随不受影响`，并保留原有整行/钳制/漂移用例。
- [x] spec：`openspec/specs/scroll-physics-drift/spec.md` 补 Requirement
      「边界外位移就地丢弃」（含「边界判定先于亚行早退」）。
- [x] 验证（真机 emulator-5554，api 35，键盘弹出、实时视图）：同量级只改方向的对照——
      向上 75px/900ms（边界外）逐像素差 `differing_px=0`；向下 75px/900ms（边界内）
      差 11960 px（余量仍在工作，证明手势确实进了 `onScroll`）；再向上 75px 回基线，
      与初始帧差 0（无漂移）。
- 手势门槛留档（免得下次误用无效对照）：`adb input swipe` 总位移不足约 40px 时
      `onScroll` 收不到有效距离（触摸门槛），40px 与 30px 的上下对照都是 0 差、无区分力。

## 十、滑动未按像素滚动

- [x] 定根因（`onFling`/`doFlingStep`）：拖动路径早就在算亚行累加器
      （`applyScrollDistance`），惯性路径却把速度先除以行高换成「行/秒」
      （`flingRowsPerSecond`），再让 `OverScroller` 按**整行**跑，每帧把
      `currY` 直接写成 `setScrollOffset(target.coerceIn(0, 回滚长度))`。三重后果：
      ① 每帧位移被量化成整行，速度换算里 `roundToInt()` 还把 600px/s 压成 10 行/s，
      行高一大就明显变慢；② 边界判定只看「行」，越界过冲被 `coerceIn` 吞掉，
      显出量恒为 0——这正是第八条那条已知残余（甩到顶端仍有最早一行被裁）；
      ③ 惯性帧不经过共用的边界丢弃/显出量逻辑，与拖动路径两套实现各走各的。
      另：`onScroll` 在惯性的第一帧就被 `flingScroller.forceFinished` 前
      `scrollAccumulatorPx = 0f` 抹掉亚行余量，手指停下的位置与内容位置脱节。
- [x] 修：惯性改按像素推进，与拖动共用一条管道。
      ① 速度换算退化为恒等直传（`flingVelocityPxPerSecond`，行高不再参与），
      `OverScroller` 直接吃像素/秒；② 上界改为
      `回滚长度 × 行高 + 完整位移容量`（`maxPx`），越界过冲因此能进显出量；
      ③ 新增 `applyScrollStep(ScrollStep, 回滚长度)`，把「写偏移、写显出量、下发
      亚行余量」收成一处，`onScroll` 与 `doFlingStep` 都只喂增量——
      惯性增量取 `currY - lastFlingPx`、经同一个 `applyScrollDistance`
      （含边界丢弃、1:1 显出量回收），不再有第二份实现；
      ④ 去掉惯性帧里的 `forceRender()`（`setScrollRemainderPx` 已含 `notifyRender`，
      双唤醒让渲染线程空转、手势期间掉帧），去掉与 `setScrollOffset` 重复的
      `coerceIn`。
- [x] 加检查（`ScrollDistanceTest`，新增 4 例 / 共 23 例 0 失败）：
      `惯性速度按像素每秒直传`（600f 必须得 600，注释写明「绝不能是 10」——
      整行路径下 600/60 恰好是 10，是能真正区分新旧实现的判据）、
      方向与偏移方向一致、拖动与惯性共用符号、以及
      `慢甩按像素推进且不整行量化`（整行量化时第一帧就跨一整行）。
- [x] spec：`openspec/specs/scroll-physics-drift/spec.md` 补 Requirement
      「惯性滚动按像素推进」（速度单位不得含行高、上界含完整位移容量、
      与拖动共用边界丢弃与显出量），3 条场景。
      `openspec validate scroll-physics-drift --type spec --strict` 通过。
- [x] 验证（真机 emulator-5554，api 35，键盘弹出）：`echo TOPMARK-0; seq 1 60`
      后在实时视图向下甩（手势方向见下方留档），logcat 逐帧读数：偏移
      `0 → 1 → 18`（= 回滚长度，到底）而**亚行余量全程非整行**
      （30.375 / 37.5 / 14.75），即惯性帧确实按像素推进；
      继续在顶端过冲，显出量吃到 `803.25px` = 18 × 44.625 = 当前完整位移容量，
      截图里**回滚区最早一行 `$ echo TOPMARK-0;seq 1 60` 出现在屏幕第一行**
      （旧实现 `coerceIn` 到 `scrollbackLen` 时那一行必在裁剪区内），
      键盘保持弹出；随后回车，视图回到底部且提示符完整可见于键盘上方
      （`TESTING.md` 第 37 条不回归）。
- 手势方向留档（免得下次白跑）：实时视图下**向上**滑是往更新方向、已在边界，
      `onScroll` 收得到但位移恒为 0，看屏幕分不出「没滚动」与「滚了但内容全同」；
      进回滚区要**向下**滑。逐像素比对也须用**带编号**的内容
      （`seq 1 60`）：全同的 `yes` 行整行滚动后屏幕逐像素不变，会伪装成没滚动。

## 十一、启动动画用图标而非完整图案

- [x] 定根因（AOSP `SplashscreenContentDrawer` + `SplashscreenIconDrawableFactory`
      源码 + 像素取证）：主题把 `windowSplashScreenAnimatedIcon` 指到自适应应用图标
      `@mipmap/ic_launcher`。平台只在**该属性未设置**（图标取自应用）时才做自适应图标的
      「只取前景」后处理（`processAdaptiveIcon`）；属性已设置时直接 `makeIconDrawable`，
      自适应图标被整块画出——底板圆形一起上屏，这就是「完整图案」；且非自适应路径下
      图标被 `AdaptiveForegroundDrawable` 放大 1.5 倍再裁圆
      （`DEFAULT_VIEW_PORT_SCALE = 2/3`、`insetWidth = 0.75 × 边长`），
      可见区只剩 108 视口中心半径 36 单位的圆。
      实测冷启动截图（density 420）：底板 `#0D1117` 圆直径 504px = 192dp
      （`starting_surface_icon_size` 160dp × 无底色系数 1.2），星形整颗被裁掉、
      窗口两个角被切。
- [x] 修（同一根因两处）：启动屏图标换成非自适应的 `@drawable/launcher_foreground`，
      父主题由 `Theme.SplashScreen.IconBackground` 换成 `Theme.SplashScreen`
      （不声明 `windowSplashScreenIconBackgroundColor`，图标后不加圆底板），
      背景保持 `#1E1E2E`——平台于是按 Android 13 默认形态把图形放大 1.5 倍画在底色上。
      图形本身越出安全区（实测最远半径 48.5 单位 > 36），故在
      `launcher_foreground.xml` 与 `launcher_monochrome.xml` 外层加一个
      `pivot(54,54) scale 0.62 translate(2.79,2.17)` 的 group：缩到半径 33
      （= 官方 66dp 安全圆），并按实测包围盒 `[7.6,91.4]×[16.4,84.6]`、
      中心 `(49.5,50.5)` 回正到视口中心。启动屏与桌面图标同源一份图形，
      没有第二份路径副本。
- [x] 附修（同一次 SVG 转换的第二个缺陷）：三个圆点路径以 `A` 开头、缺首段 `M`，
      解析时起点落在原点、每段弧被重新放大（实测渲染成 80×95px 的大色块；
      半径 1.6 单位的点应为 11px）。补 `M` 并去掉退化的首段弧（两点对径，
      两段半弧即成整圆），实测三点各 10–11px。
- [x] 加检查（`SplashIconRobolectricTest`，`@GraphicsMode(NATIVE)` + `sdk = 34`）：
      从 `Theme.Terminal.Splash` 解析 `android:windowSplashScreenAnimatedIcon`，
      断言 ① 不是 `AdaptiveIconDrawable`（交自适应图标即「完整图案」）；
      ② 按 4px/单位渲染后确有图案（不透明像素 > 位图 1%）；
      ③ 最远半径 ≤ 36 单位（超出即被裁）；④ 最远半径 ≥ 30 单位（不得缩成小块）。
      改前跑：① 报图标是自适应图标，③ 报「实际最远 48.5，包围盒
      `[7.6,91.4]×[16.4,84.6]`」；改后 1 例 0 失败。
- [x] spec：`openspec/specs/render-stability/spec.md` 的「启动首帧及时呈现」后补
      Requirement「启动屏只画图标不用完整图案」，3 条场景，新增 Requirement 224 字、
      无告警。本文件 `--strict` 仍报改动前就有的那一条告警（`requirements[7]`：
      3223 字的「备用屏下输入法弹出不位移终端 Surface」超 500 字上限），非本次引入；
      全仓同类告警共 14 条、6 个 spec，见十二节留档。
- [x] 验证（真机 emulator-5554，api 35，density 420）：重装后冷启动逐帧截图——
      `(700,1200)` 由底板色 `(13,17,23)` 变为底色 `(30,30,46)`（底板消失），
      图案包围盒 `[364,724]×[1050,1349]`、中心 `(544,1199.5)`（屏幕中心
      `(540,1200)`），最远半径 234px < 可见圆 252px（= 96dp）→ 无裁切；
      星形 `#ffb467` 386px 出现在 `(693,1081)`（改前整颗被裁不可见），
      三点各 10–11px。桌面图标（应用抽屉）同样完整：星形 12×10px 可见、
      三点正常、窗口角不再被切。
- 口径留档：`values-v31` 下 `Theme.SplashScreen` 与
      `Theme.SplashScreen.IconBackground` 的差别仅是后者多映射
      `android:windowSplashScreenIconBackgroundColor`；本应用 `minSdk = 33`，
      兼容库 23–30 的启动屏路径（`Theme.SplashScreen.Common`）在设备上根本不走。

## 十二、收尾

- [ ] 全仓门禁 `TMPDIR=/tmp ./scripts/check.nu` 一次性跑通，逐阶段核对。
      **口径更正**：该脚本在**第一个失败的外部命令处即中断**，并以它的退出码结束
      （nushell 里外部命令非零退出会终止脚本：探针 `^sh -c "exit 3"` 之后的语句不执行、
      脚本退出码 3；内置 `false` 不算外部命令，故早前那条「不会中断」的结论测错了对象）。
      首轮实测正是如此：`cargo test` 报 1 例失败后脚本停在 687 行、退出码 101，
      clippy / doc / bench / semgrep / markdownlint / gradle 全部未跑。
      故「跑通」＝从 `cargo fmt` 段起重跑并逐段看到全部阶段通过。
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
