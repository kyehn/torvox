package terminal.emulator

import android.annotation.SuppressLint
import android.content.ClipData
import android.content.ClipboardManager
import android.content.Context
import androidx.compose.ui.test.junit4.v2.createAndroidComposeRule
import androidx.test.platform.app.InstrumentationRegistry
import androidx.test.rule.GrantPermissionRule
import androidx.test.uiautomator.By
import androidx.test.uiautomator.UiDevice
import androidx.test.uiautomator.Until
import org.junit.After
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Rule
import org.junit.Test
import terminal.emulator.TerminalLogcatTest
import terminal.emulator.bridge.Bridge
import terminal.emulator.cleanUpTerminalState
import terminal.emulator.terminalCellSizePx
import terminal.emulator.terminalGridColumns
import terminal.emulator.util.runCatchingCancellable

/**
 * Quantified verification of the selection/drag behaviors reported broken:
 *
 * 1. "paste 始终显示 / 菜单内容错误" — the long-press target decides the menu. Metric: exact ActionMode item
 *    set after a real long-press. Assert: whitespace → {PASTE} without COPY; word → {COPY,
 *    SELECT_ALL} without PASTE.
 * 2. "拖动卡顿漂移" live-highlight side: while a handle is dragged in slow steps, the inverted-cell
 *    highlight must change on screen BETWEEN the steps, not only at release. Metric: consecutive
 *    full-screen captures taken after each drag step must differ materially at >= 3 of 4 steps, and
 *    the final COPY action must be enabled (non-empty grown range).
 * 3. D7.5: grabbing a handle on a paste-only (blank cell) selection must upgrade it to a text
 *    selection and grow a real range — measured as the paste-only menu transitioning to the full
 *    menu with an enabled COPY.
 *
 * All measured values log as `UX_METRIC ...` lines for trend tracking.
 */
class SelectionDragQuantifiedTest : TerminalLogcatTest() {
    @get:Rule
    val notificationPermission =
        GrantPermissionRule.grant(android.Manifest.permission.POST_NOTIFICATIONS)

    @get:Rule val composeTestRule = createAndroidComposeRule<MainActivity>()

    private lateinit var device: UiDevice

    @Before
    fun setUp() {
        device = UiDevice.getInstance(InstrumentationRegistry.getInstrumentation())
        composeTestRule.waitForSession()
        // 桥单次读取：会话孵化中为 null，由调用方轮询重试（getBridge 契约）。
        terminal.emulator.UxTestUtils.pollUntilTrue(timeoutMs = 30_000, intervalMs = 200) {
            composeTestRule.getBridge() != null
        }
        assertNotNull("运行时桥必须就绪（30s 未孵化）", composeTestRule.getBridge())
    }

    private fun bridge(): Bridge = composeTestRule.getBridge() ?: throw AssertionError("bridge null")

    /** 本类用 `feedTerminal` 直写标记：不清场即污染共用会话。 */
    @After
    fun resetSession() = composeTestRule.cleanUpTerminalState()

    /** The TerminalSurface view, resolved through the house TestUtils helper. */
    private fun surfaceView(): android.view.View = findTerminalSurface(composeTestRule.activity)

    /**
     * 已挂载的 surface（轮询）：类内多测试共享 Activity 时，测试中途可能发生
     * Activity 重建——缓存/过早解析的旧 surface 已 detached，向其 post 触摸永不
     * 执行且无日志。每次注入前现取并确认 attached。
     */
    private fun attachedSurface(timeoutMs: Long = 10_000): android.view.View {
        val deadline = android.os.SystemClock.uptimeMillis() + timeoutMs
        var view = surfaceView()
        while (!view.isAttachedToWindow && android.os.SystemClock.uptimeMillis() < deadline) {
            Thread.sleep(100)
            view = surfaceView()
        }
        assertTrue("surface 必须已挂载", view.isAttachedToWindow)
        return view
    }

    /**
     * 物理单元格（运行时触摸数学同口径：桥逻辑值 × density；直接拿桥值当 px
     * 会小 2~3 倍——历史拖拽测试在该错尺度上“恰好”自洽，绝对点击则整体漂移）。
     */
    private fun cellPx(): Pair<Float, Float> {
        val (cellWidth, cellHeight) = terminalCellSizePx(composeTestRule.activity, bridge())
        assertTrue(
            "cell metrics unavailable ($cellWidth x $cellHeight)",
            cellWidth > 0f && cellHeight > 0f,
        )
        return Pair(cellWidth, cellHeight)
    }

    /** surface 本地坐标的单元格锚点：第 [col] 列、视口 [row] 行底边（控制柄悬挂处）。 */
    private fun cellAnchorLocal(col: Int, row: Int): Pair<Float, Float> {
        val (cellWidth, cellHeight) = cellPx()
        return Pair(col * cellWidth, (row + 1) * cellHeight)
    }

    private fun currentText(): String? {
        runCatchingCancellable { terminal.emulator.bridge.NativeBridge.pollEvent() }
        return composeTestRule.getBridge()?.getTerminalText()
    }

    /**
     * 经 parser 直写把 [words] 放到固定视口行（确定性放置）：不经
     * shell 行编辑，网格稳定。返回词内点击的 surface 本地坐标与视口行。
     * 行取 7（0 基）：长按后菜单在选择上方弹出，不会压住下方控制柄——
     * 行 0 选择的菜单被迫落下方，正压 END 柄位，柄抓变菜单点击（全选）。
     */
    private fun prepareWordTarget(
        words: String,
        tapWordOffset: Int = 5,
        warmKeyboard: Boolean = false,
        targetViewportRow: Int = 7,
    ): Triple<Float, Float, Int> {
        // 视口先归位：坐标换算只在滚动偏移为 0 时成立（见 scrollViewportToBottom）。
        scrollViewportToBottom(bridge(), composeTestRule.activity.terminalViewModel.runtime)
        // 标记必须单行放得下：网格列数随屏幕宽度与主字体变化（实测 25～38 列），
        // 超宽即折行，文本查询按整串匹配时恒不成立——外部表现是「标记不落格」，
        // 与送显、与查询超时都无关。前提不成立时在此直接失败，不留到后面误判。
        val columns = terminalGridColumns(composeTestRule.activity, bridge())
        assertTrue(
            "标记必须单行容得下 (列数=$columns 标记=[$words])",
            words.length <= columns,
        )
        // 经 parser 直写不经 shell 行编辑：不门控 shell prompt，以标记落格为就绪。
        // 共用会话的 shell 行状态（补全等待、未回车输入）与 parser 网格无关，
        // 门控 prompt 只会把 shell 污染误判成送显失败。
        // 仅点选手势需要预热（长按不拉键盘，预热反而增加失败面）。
        if (warmKeyboard) settleKeyboard()
        // 经 parser 直写幂等可重发：慢模拟器首帧可吞单次送显，与 ImePopup 同口径至多 3 次，每轮落盘不吞因。
        // 失败时附终端尾部文本：CI 上 scrollback 达 776+ 行时全量文本重建易越过查询超时，
        // 有尾部才能区分“送显丢失”与“查询超时读空”，不做静默断言。
        var landed: Long? = null
        var lastSeen: String? = null
        for (attempt in 1..3) {
            if (landed != null) break
            val fed =
                bridge().feedTerminal(
                    "\u001B[${targetViewportRow + 1};1H$words".toByteArray(Charsets.UTF_8),
                )
            assertTrue("标记送显失败", fed)
            landed =
                UxTestUtils.pollUntilTrue(timeoutMs = 15_000, intervalMs = 100) {
                    val text = currentText()
                    lastSeen = text?.takeLast(200)
                    text?.contains(words) == true
                }
            android.util.Log.i("SelectionDrag", "feed $words attempt=$attempt hit=${landed != null} tail=[$lastSeen]")
        }
        assertNotNull("标记必须落格: $words tail=[$lastSeen]", landed)
        val depth = bridge().scrollbackLength()
        val lines = currentText().orEmpty().lines()
        val index = lines.indexOfFirst { it.contains(words) }
        assertTrue("输出行定位失败: $words", index >= 0)
        val viewportRow = index - depth
        assertTrue("输出必须在可见视口内 (行=$viewportRow)", viewportRow >= 0)
        val col = lines[index].indexOf(words) + tapWordOffset
        val (cellWidth, cellHeight) = cellPx()
        val surface = attachedSurface()
        val tapX = (col + 0.5f) * cellWidth
        val tapY = (viewportRow + 0.5f) * cellHeight
        // surface 左侧 32dp 为抽屉边缘区（触摸直达丢弃）：点中词中部使其落在区外
        // （col 2 会落入区内被吞）。
        val density = composeTestRule.activity.resources.displayMetrics.density
        assertTrue(
            "点击必须在抽屉边缘区外 (x=$tapX)",
            tapX > 32f * density,
        )
        assertTrue("点击必须在 surface 内 (x=$tapX w=${surface.width})", tapX > 0f && tapX < surface.width)
        assertTrue("点击必须在 surface 内 (y=$tapY h=${surface.height})", tapY > 0f && tapY < surface.height)
        return Triple(tapX, tapY, viewportRow)
    }

    /**
     * 键盘预热：首击 surface 中部把 IME 拉起并等动画落定，否则首个点选手势的
     * inset 翻转会清掉刚建的选择（单击本身不建选择）。
     *
     * 点按至多 3 轮：首击可能只拿到窗口焦点而未弹键盘（本机复现 20s 不可见），
     * 焦点到手后次击即弹；每轮落盘不等即重试，不把焦点竞态误判成键盘缺失。
     */
    private fun settleKeyboard() {
        var shown: Long? = null
        for (attempt in 1..3) {
            if (shown != null) break
            val surface = attachedSurface()
            injectTap(surface, surface.width / 2f, surface.height / 2f)
            shown =
                UxTestUtils.pollUntilTrue(timeoutMs = 20_000, intervalMs = 200) {
                    var visible = false
                    InstrumentationRegistry.getInstrumentation().runOnMainSync {
                        visible = composeTestRule.activity.window.decorView.rootWindowInsets
                            ?.isVisible(android.view.WindowInsets.Type.ime()) == true
                    }
                    visible
                }
        }
        assertNotNull("IME 必须弹起（3 轮未可见）", shown)
        Thread.sleep(1_500)
    }

    /**
     * 真实手势滑动（屏坐标）：控制柄拖拽归属 surface 上方的 overlay 弹窗，
     * 直接 dispatch 到 surface 的 DOWN 会被当成非控制柄点击而清选择；
     * 经 UiAutomator 走窗口层级，overlay 消费拖拽（真实用户路径）。
     * 入参为 surface 本地坐标，内部换算屏坐标。
     */
    private fun realSwipe(x0Local: Float, y0Local: Float, x1Local: Float, y1Local: Float, steps: Int) {
        val loc = IntArray(2)
        attachedSurface().getLocationOnScreen(loc)
        device.swipe(
            (loc[0] + x0Local).toInt(),
            (loc[1] + y0Local).toInt(),
            (loc[0] + x1Local).toInt(),
            (loc[1] + y1Local).toInt(),
            steps,
        )
        Thread.sleep(150)
    }

    /**
     * 长按至选择出现再抬手（慢模拟器主线程卡顿会把固定 1200ms 长按吞成点击）：
     * DOWN 后轮询选择激活（4s），见选择才 MOVE/UP；无选择则抬手重试一轮。
     */
    private fun longPressUntilSelected(x: Float, y: Float, attempts: Int = 2) {
        repeat(attempts) {
            val surface = attachedSurface()
            val downTime = android.os.SystemClock.uptimeMillis()
            surface.post {
                surface.dispatchTouchEvent(
                    android.view.MotionEvent.obtain(
                        downTime,
                        downTime,
                        android.view.MotionEvent.ACTION_DOWN,
                        x,
                        y,
                        0,
                    ),
                )
            }
            val selected =
                UxTestUtils.pollUntilTrue(timeoutMs = 4_000, intervalMs = 100) {
                    var active = false
                    InstrumentationRegistry.getInstrumentation().runOnMainSync {
                        active = composeTestRule.activity.terminalViewModel.state.value.selection.active
                    }
                    active
                }
            val now = android.os.SystemClock.uptimeMillis()
            surface.post {
                surface.dispatchTouchEvent(
                    android.view.MotionEvent.obtain(
                        downTime,
                        now,
                        android.view.MotionEvent.ACTION_MOVE,
                        x + 1f,
                        y + 1f,
                        0,
                    ),
                )
                surface.dispatchTouchEvent(
                    android.view.MotionEvent.obtain(
                        downTime,
                        now,
                        android.view.MotionEvent.ACTION_UP,
                        x + 1f,
                        y + 1f,
                        0,
                    ),
                )
            }
            Thread.sleep(300)
            if (selected != null && menuVisible("粘贴")) return
        }
    }

    private fun waitForMenuText(text: String, timeoutMs: Long = 4_000) =
        device.wait(Until.findObject(By.text(text)), timeoutMs)

    /**
     * 抓柄不抬手（“抓柄即隐藏”的断言窗口）：经系统注入把 DOWN + 一步 MOVE 送到
     * overlay 柄位后停住（UiAutomator swipe 无法中途观测），返回 downTime 供
     * [liftGrabbedHandle] 收尾。必须走系统注入——柄位归属 overlay 窗口。
     */
    private fun grabHandleWithoutLifting(xLocal: Float, yLocal: Float): Long {
        val location = IntArray(2)
        attachedSurface().getLocationOnScreen(location)
        val downTime = android.os.SystemClock.uptimeMillis()
        val instrumentation = InstrumentationRegistry.getInstrumentation()
        instrumentation.sendPointerSync(
            android.view.MotionEvent.obtain(
                downTime,
                downTime,
                android.view.MotionEvent.ACTION_DOWN,
                location[0] + xLocal,
                location[1] + yLocal,
                0,
            ),
        )
        Thread.sleep(120)
        instrumentation.sendPointerSync(
            android.view.MotionEvent.obtain(
                downTime,
                android.os.SystemClock.uptimeMillis(),
                android.view.MotionEvent.ACTION_MOVE,
                location[0] + xLocal + 4f,
                location[1] + yLocal,
                0,
            ),
        )
        Thread.sleep(200)
        return downTime
    }

    /** 抬手收尾：结束 [grabHandleWithoutLifting] 开始的柄上按压（触发重锚重显）。 */
    private fun liftGrabbedHandle(downTime: Long, xLocal: Float, yLocal: Float) {
        val location = IntArray(2)
        attachedSurface().getLocationOnScreen(location)
        InstrumentationRegistry.getInstrumentation().sendPointerSync(
            android.view.MotionEvent.obtain(
                downTime,
                android.os.SystemClock.uptimeMillis(),
                android.view.MotionEvent.ACTION_UP,
                location[0] + xLocal,
                location[1] + yLocal,
                0,
            ),
        )
        Thread.sleep(150)
    }

    private fun menuVisible(text: String): Boolean = device.findObject(By.text(text)) != null

    /**
     * 粘贴项是否进菜单取决于剪贴板非空（pasteEnabled 门控）：测试必须自建
     * 剪贴板内容，不能依赖系统剪贴板历史（会被清，届时菜单无粘贴项）。
     */
    @SuppressLint("DeprecatedCall")
    private fun seedClipboard(text: String = "CLIPSEED") {
        InstrumentationRegistry.getInstrumentation().runOnMainSync {
            val clipboard =
                composeTestRule.activity.getSystemService(Context.CLIPBOARD_SERVICE) as ClipboardManager
            clipboard.setPrimaryClip(ClipData.newPlainText("test", text))
        }
    }

    private fun resetSelection() {
        injectTap(
            attachedSurface(),
            (device.displayWidth / 2).toFloat(),
            (device.displayHeight - 120).toFloat(),
        )
        Thread.sleep(600)
    }

    // ── 1. menu content matrix ──────────────────────────────────────────

    @Test
    fun whitespace_longpress_shows_paste_only_menu() {
        seedClipboard()
        val b = bridge()
        b.writeToPty("clear\n".toByteArray(Charsets.UTF_8))
        Thread.sleep(1_200)

        // Long-press the blank region right of the prompt: guaranteed
        // whitespace target regardless of font metrics (prompt never fills
        // the whole line).
        val x = device.displayWidth - 160
        val y = device.displayHeight - 260
        injectLongPress(attachedSurface(), x.toFloat(), y.toFloat())

        assertNotNull("PASTE item missing for whitespace long-press", waitForMenuText("粘贴"))
        assertTrue(
            "whitespace long-press must not offer COPY",
            !menuVisible("复制"),
        )
        UxTestUtils.metric("menu_whitespace_paste_only", 1)
        resetSelection()
    }

    @Test
    fun word_longpress_shows_copy_selectall_without_paste() {
        val (tapX, tapY, tapRow) = prepareWordTarget("targetword")
        longPressUntilSelected(tapX, tapY)

        // 应用仅简体中文：菜单为中文 PopupWindow（复制/分享/全选），英文 COPY 永不出现。
        val gridDbg = currentText()?.takeLast(400)
        assertNotNull(
            "COPY item missing for word long-press (tap=$tapX,$tapY row=$tapRow grid=[$gridDbg])",
            waitForMenuText("复制"),
        )
        assertTrue("SELECT_ALL item missing", menuVisible("全选"))
        assertTrue(
            "word long-press must NOT show PASTE (the reported 'paste always visible' bug)",
            !menuVisible("粘贴"),
        )
        UxTestUtils.metric("menu_word_full_set", 1)
        resetSelection()
    }

    // ── 2. live-highlight cadence during handle drag ────────────────────

    @Test
    fun handle_drag_updates_highlight_live_between_steps() {
        val (tapX, tapY, _) = prepareWordTarget("dragstart", warmKeyboard = true)
        // 长按选词：其 END 控制柄锚在该词右单元格边缘。慢机上固定时长按会被吞成点击，
        // 用轮询重试版直至选择激活，与 D75 同口径。
        longPressUntilSelected(tapX, tapY)
        Thread.sleep(900)
        assertNotNull("长按未打开选择菜单", waitForMenuText("复制"))

        // Grab the END handle at its real anchor: read the selection END from
        // state (grid rows → viewport), rather than guessing tap+2 (misses when
        // the word is longer than 2 cells past the tap).
        var endCol = -1
        var endGridRow = -1
        InstrumentationRegistry.getInstrumentation().runOnMainSync {
            val selection = composeTestRule.activity.terminalViewModel.state.value.selection
            endCol = selection.end?.col ?: -1
            endGridRow = selection.end?.row ?: -1
        }
        assertTrue("长按后必须有选择末端 (col=$endCol row=$endGridRow)", endCol >= 0 && endGridRow >= 0)
        val depthNow = bridge().scrollbackLength()
        val endViewportRow = endGridRow - depthNow
        // END 锚点 = (endCol+1, endRow+1) 格点（positionAllHandles：END 取 endCol+1），
        // 抓取必须落在命中矩形内，tap+2 猜测与少一格都会脱靶。
        val (grabX, grabY) = cellAnchorLocal(col = endCol + 1, row = endViewportRow)
        val before = UxTestUtils.screenshot(device)

        var liveUpdates = 0
        var previous = before
        val (stepCw, _) = cellPx()
        val cwInt = stepCw.toInt().coerceAtLeast(20)
        var currentX = grabX

        // 抓柄即隐藏（design 决策 3）：DOWN 抓住 END柄、未抬手期间菜单必须隐藏；
        // 抬手由 finishHandleDrag 按新几何重锚重显（同时防回退）。
        val downTime = grabHandleWithoutLifting(grabX, grabY)
        val hiddenDuringGrab =
            UxTestUtils.pollUntilTrue(timeoutMs = 1_500, intervalMs = 100) { !menuVisible("复制") }
        assertNotNull("抓柄期间菜单必须隐藏", hiddenDuringGrab)
        liftGrabbedHandle(downTime, grabX + cwInt / 2f, grabY)
        assertNotNull("抬手后菜单必须重锚重显", waitForMenuText("复制", 3_000))

        repeat(4) {
            currentX += cwInt
            // 每步一次真实滑动（down/move/up）：选择在步间保持，控制柄随末端走，
            // 下一步 DOWN 落在新柄位重新抓住（与原注入步进语义一致）。
            realSwipe(
                x0Local = currentX - cwInt / 2,
                y0Local = grabY,
                x1Local = currentX,
                y1Local = grabY,
                steps = 4,
            )
            Thread.sleep(110)
            val capture = UxTestUtils.screenshot(device)
            if (UxTestUtils.changedPixelCount(previous, capture) > 150) liveUpdates++
            previous = capture
        }

        UxTestUtils.metric("drag_live_highlight_steps", liveUpdates)
        assertTrue(
            "highlight changed at only $liveUpdates/4 steps — drag updates are not live",
            liveUpdates >= 3,
        )

        val copy = waitForMenuText("复制", 3_000)
        assertNotNull("selection menu vanished after handle drag", copy)
        assertTrue("COPY disabled after drag — range did not grow to real text", requireNotNull(copy).isEnabled)
        resetSelection()
    }

    // ── 3. D7.5: paste-only upgrade on handle grab ──────────────────────

    @Test
    fun paste_only_handle_drag_upgrades_selection_and_grows_D75() {
        val (_, _, markerRow) = prepareWordTarget("growme")
        seedClipboard()
        val (cellWidth, cellHeight) = cellPx()
        val surface = attachedSurface()
        // prompt 行空白中部：取中列（两侧 ~32dp 皆为系统/抽屉手势区，从边缘起笔
        // 会被系统 edge-swipe 夺走并触发返回、手势流中断、Activity 销毁）。
        // 原测试“prompt 右空白”等价但落点须可定位，取中列同样为空白。
        val cols = (surface.width / cellWidth).toInt()
        val blankCol = (cols / 2).coerceAtLeast(10)
        val blankRow = markerRow + 1
        val blankLine = currentText().orEmpty().lines().getOrNull(bridge().scrollbackLength() + blankRow).orEmpty()
        assertTrue("长按行尾必须空白 (行=$blankRow 内容=[$blankLine])", blankLine.drop(blankCol).isBlank())
        val blankX = (blankCol + 0.5f) * cellWidth
        val blankY = (blankRow + 0.5f) * cellHeight
        longPressUntilSelected(blankX, blankY)
        assertNotNull("precondition: PASTE-only menu missing", waitForMenuText("粘贴"))
        assertTrue("precondition: COPY must be absent on blank selection", !menuVisible("复制"))

        // Grab the stacked END handle of the pressed cell itself: anchor at
        // that cell's bottom-right corner where the END handle hangs.
        // 全 surface 本地坐标；终点落入标记词内（约第 10 列、标记行中部）。
        val (handleX, handleY) = cellAnchorLocal(col = blankCol + 1, row = blankRow)
        val targetX = (10 + 0.5f) * cellWidth
        realSwipe(
            x0Local = handleX,
            y0Local = handleY,
            x1Local = targetX,
            y1Local = (markerRow + 0.5f) * cellHeight,
            steps = 12,
        )

        // The upgrade is observable exactly through the menu transition:
        // paste-only {PASTE} → full {COPY,...} with non-empty text.
        val copy = waitForMenuText("复制", 4_000)
        var endDbg = "?"
        var startDbg = "?"
        var draggingDbg = "?"
        composeTestRule.activityRule.scenario.onActivity { activity ->
            val selection = activity.terminalViewModel.state.value.selection
            startDbg = "${selection.start?.row},${selection.start?.col}"
            endDbg = "${selection.end?.row},${selection.end?.col}"
            draggingDbg = "${selection.dragging} active=${selection.active} pasteOnly=${selection.pasteOnly}"
        }
        assertNotNull(
            "D7.5 failed: dragging a blank-selection handle did not grow a text range " +
                "(start=[$startDbg] end=[$endDbg] $draggingDbg)",
            copy,
        )
        assertTrue("grown range has no selectable text", requireNotNull(copy).isEnabled)
        assertTrue("full menu still shows PASTE after growth to text", !menuVisible("粘贴"))
        UxTestUtils.metric("d75_blank_drag_upgrade", 1)
        resetSelection()
    }
}
