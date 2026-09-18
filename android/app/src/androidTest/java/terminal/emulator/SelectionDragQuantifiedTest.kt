package terminal.emulator

import androidx.compose.ui.test.junit4.v2.createAndroidComposeRule
import androidx.test.platform.app.InstrumentationRegistry
import androidx.test.rule.GrantPermissionRule
import androidx.test.uiautomator.By
import androidx.test.uiautomator.UiDevice
import androidx.test.uiautomator.Until
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Rule
import org.junit.Test
import terminal.emulator.bridge.Bridge

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
class SelectionDragQuantifiedTest {
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

    /** The TerminalSurface view, resolved through the house TestUtils helper. */
    private fun surfaceView(): android.view.View = findTerminalSurface(composeTestRule.activity)

    /**
     * 物理单元格（运行时触摸数学同口径：桥逻辑值 × density；直接拿桥值当 px
     * 会小 2~3 倍——历史拖拽测试在该错尺度上“恰好”自洽，绝对点击则整体漂移）。
     */
    private fun cellPx(): Pair<Float, Float> {
        val density = composeTestRule.activity.resources.displayMetrics.density
        val cw = bridge().getCellWidth() * density
        val ch = bridge().getCellHeight() * density
        assertTrue("cell metrics unavailable ($cw x $ch)", cw > 0f && ch > 0f)
        return Pair(cw, ch)
    }

    /** surface 本地坐标的单元格锚点：第 [col] 列、视口 [row] 行底边（控制柄悬挂处）。 */
    private fun cellAnchorLocal(col: Int, row: Int): Pair<Float, Float> {
        val (cw, ch) = cellPx()
        return Pair(col * cw, (row + 1) * ch)
    }

    private fun currentText(): String? {
        runCatching { terminal.emulator.bridge.NativeBridge.pollEvent() }
        return composeTestRule.getBridge()?.getTerminalText()
    }

    /**
     * 经 shell 真实执行打印 [words]，返回词内点击的 surface 本地坐标与视口行。
     * prompt 门控 + 回显轮询：冷启动 shell 未消费 stdin 前的输入会丢失（粘贴案），
     * 盲 sleep 后按绝对屏坐标点是双重不可靠——行列由落格位置算出。
     * 含键盘预热（MultiTap 同因）：单击/双击会拉起 IME，若手势期发生 inset 翻转，
     * 刚建的选择会被清掉；预热后手势期无翻转。
     */
    private fun prepareWordTarget(words: String, tapWordOffset: Int = 2): Triple<Float, Float, Int> {
        val promptSeen =
            UxTestUtils.pollUntilTrue(timeoutMs = 60_000, intervalMs = 200) {
                val text = currentText()
                text != null && (text.contains("$") || text.contains("#"))
            }
        assertNotNull("shell prompt 必须先就绪", promptSeen)
        settleKeyboard()
        assertTrue(
            "printf 送显失败",
            bridge().writeToPty("printf '$words\\n'\n".toByteArray(Charsets.UTF_8)),
        )
        val echoed =
            UxTestUtils.pollUntilTrue(timeoutMs = 15_000, intervalMs = 100) {
                currentText()?.contains(words) == true
            }
        assertNotNull("shell 必须执行并回显: $words", echoed)
        val depth = bridge().scrollbackLength()
        val lines = currentText().orEmpty().lines()
        val index = lines.indexOfFirst { it.contains(words) }
        assertTrue("输出行定位失败: $words", index >= 0)
        val viewportRow = index - depth
        assertTrue("输出必须在可见视口内 (行=$viewportRow)", viewportRow >= 0)
        val col = lines[index].indexOf(words) + tapWordOffset
        val (cw, ch) = cellPx()
        val surface = surfaceView()
        val tapX = (col + 0.5f) * cw
        val tapY = (viewportRow + 0.5f) * ch
        assertTrue("点击必须在 surface 内 (x=$tapX w=${surface.width})", tapX > 0f && tapX < surface.width)
        assertTrue("点击必须在 surface 内 (y=$tapY h=${surface.height})", tapY > 0f && tapY < surface.height)
        return Triple(tapX, tapY, viewportRow)
    }

    /**
     * 键盘预热：首击 surface 中部把 IME 拉起并等动画落定，否则首个点选手势的
     * inset 翻转会清掉刚建的选择（MultiTap 同根因；单击本身不建选择）。
     */
    private fun settleKeyboard() {
        val surface = surfaceView()
        injectTap(surface, surface.width / 2f, surface.height / 2f)
        val shown =
            UxTestUtils.pollUntilTrue(timeoutMs = 20_000, intervalMs = 200) {
                var visible = false
                InstrumentationRegistry.getInstrumentation().runOnMainSync {
                    visible = composeTestRule.activity.window.decorView.rootWindowInsets
                        ?.isVisible(android.view.WindowInsets.Type.ime()) == true
                }
                visible
            }
        assertNotNull("IME 必须弹起（20s 未可见）", shown)
        Thread.sleep(1_500)
    }

    private fun waitForMenuText(text: String, timeoutMs: Long = 4_000) =
        device.wait(Until.findObject(By.text(text)), timeoutMs)

    private fun menuVisible(text: String): Boolean = device.findObject(By.text(text)) != null

    private fun resetSelection() {
        injectTap(
            surfaceView(),
            (device.displayWidth / 2).toFloat(),
            (device.displayHeight - 120).toFloat(),
        )
        Thread.sleep(600)
    }

    // ── 1. menu content matrix ──────────────────────────────────────────

    @Test
    fun whitespace_longpress_shows_paste_only_menu() {
        val b = bridge()
        b.writeToPty("clear\n".toByteArray(Charsets.UTF_8))
        Thread.sleep(1_200)

        // Long-press the blank region right of the prompt: guaranteed
        // whitespace target regardless of font metrics (prompt never fills
        // the whole line).
        val x = device.displayWidth - 160
        val y = device.displayHeight - 260
        injectLongPress(surfaceView(), x.toFloat(), y.toFloat())

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
        val (tapX, tapY, _) = prepareWordTarget("targetword targetword targetword")
        injectLongPress(surfaceView(), tapX, tapY)

        // 应用仅简体中文：菜单为中文 PopupWindow（复制/分享/全选），英文 COPY 永不出现。
        assertNotNull("COPY item missing for word long-press", waitForMenuText("复制"))
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
        val (tapX, tapY, tappedRow) = prepareWordTarget("dragstart dragend dragend dragend")
        // Double-tap selects the word under the finger; its END handle then
        // anchors at that word's right cell edge.
        injectDoubleTap(surfaceView(), tapX, tapY)
        Thread.sleep(900)
        assertNotNull("double-tap did not open the selection menu", waitForMenuText("复制"))

        // Grab the END handle: ~2 cells right of the tap (the selected word
        // spans about one cell per 5-6 chars at default metrics; 2 cells is
        // safely past its right edge) and exactly on the row-bottom anchor.
        // 全 surface 本地坐标（injectDrag 直达 dispatchTouchEvent）：屏坐标在此整体漂移。
        val (cw, _) = cellPx()
        val tappedCol = (tapX / cw).toInt()
        val (grabX, grabY) = cellAnchorLocal(col = tappedCol + 2, row = tappedRow)
        val before = UxTestUtils.screenshot(device)

        var liveUpdates = 0
        var previous = before
        val cwInt = cw.toInt().coerceAtLeast(20)
        var currentX = grabX
        repeat(4) {
            currentX += cwInt
            UxTestUtils.injectDrag(
                surfaceView(),
                x0 = currentX - cwInt / 2,
                y0 = grabY,
                x1 = currentX,
                y1 = grabY,
                steps = 2,
                stepDelayMs = 110,
            )
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
        val (_, _, markerRow) = prepareWordTarget("growme growme growme")
        val (cw, ch) = cellPx()
        val surface = surfaceView()
        // prompt 行空白 far-right：标记行下一行是 prompt（"$ "占前两列），
        // 取末列前二格必为空白——长按落点与原测试“prompt 右空白”等价但可定位。
        val cols = (surface.width / cw).toInt()
        val blankCol = (cols - 2).coerceAtLeast(10)
        val blankRow = markerRow + 1
        val blankLine = currentText().orEmpty().lines().getOrNull(bridge().scrollbackLength() + blankRow).orEmpty()
        assertTrue("长按行尾必须空白 (行=$blankRow 内容=[$blankLine])", blankLine.drop(blankCol).isBlank())
        val blankX = (blankCol + 0.5f) * cw
        val blankY = (blankRow + 0.5f) * ch
        injectLongPress(surface, blankX, blankY)
        assertNotNull("precondition: PASTE-only menu missing", waitForMenuText("粘贴"))
        assertTrue("precondition: COPY must be absent on blank selection", !menuVisible("复制"))

        // Grab the stacked END handle of the pressed cell itself: anchor at
        // that cell's bottom-right corner where the END handle hangs.
        // 全 surface 本地坐标；终点落入标记词内（约第 10 列、标记行中部）。
        val (handleX, handleY) = cellAnchorLocal(col = blankCol + 1, row = blankRow)
        val targetX = (10 + 0.5f) * cw
        UxTestUtils.injectDrag(
            surfaceView(),
            x0 = handleX,
            y0 = handleY,
            x1 = targetX,
            y1 = (markerRow + 0.5f) * ch,
            steps = 6,
            stepDelayMs = 100,
        )

        // The upgrade is observable exactly through the menu transition:
        // paste-only {PASTE} → full {COPY,...} with non-empty text.
        val copy = waitForMenuText("复制", 4_000)
        assertNotNull(
            "D7.5 failed: dragging a blank-selection handle did not grow a text range",
            copy,
        )
        assertTrue("grown range has no selectable text", requireNotNull(copy).isEnabled)
        assertTrue("full menu still shows PASTE after growth to text", !menuVisible("粘贴"))
        UxTestUtils.metric("d75_blank_drag_upgrade", 1)
        resetSelection()
    }
}
