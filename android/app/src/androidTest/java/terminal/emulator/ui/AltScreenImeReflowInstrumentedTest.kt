package terminal.emulator.ui

import android.view.WindowInsets
import androidx.compose.ui.test.junit4.v2.createAndroidComposeRule
import androidx.test.platform.app.InstrumentationRegistry
import androidx.test.rule.GrantPermissionRule
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.junit.runners.JUnit4
import terminal.emulator.MainActivity
import terminal.emulator.TerminalLogcatTest
import terminal.emulator.UxTestUtils
import terminal.emulator.awaitBridge
import terminal.emulator.bridge.NativeBridge
import terminal.emulator.findTerminalSurface
import terminal.emulator.getBridge
import terminal.emulator.waitForSession
import terminal.emulator.waitForTerminalPixels

/**
 * 备用屏（全屏 TUI）在输入法弹出时**随窗口自适应**（DESIGN：输入法弹出时终端
 * 包括 helix 等 tui 应用正确匹配窗口大小不溢出）。
 *
 * 为何必须实测网格行数：helix/vim/less 都进备用屏并按整屏行数布局，键盘遮挡的
 * 下半屏永远不可见——状态行消失、光标可能落在被遮住的几行里，而应用收不到
 * SIGWINCH 也不会重排。历史上只断言「不位移」，等于替「保持不可见」背书。
 *
 * 网格断言读的是 PTY 网格（`getGridRowsColsPacked`），预期值则由平台量得的输入法
 * 高度、导航条高度、Surface 高度与单元格高按网格公式独立算出——不是复述被测函数。
 */
@RunWith(JUnit4::class)
class AltScreenImeReflowInstrumentedTest : TerminalLogcatTest() {
    companion object {
        private const val IME_TIMEOUT_MS = 10_000L
        private const val GRID_TIMEOUT_MS = 20_000L
        private const val SETTLE_MILLIS = 800L

        /** 进入备用屏：DECSET 1049 + 清屏 + 光标归位。 */
        private const val ENTER_ALT_SCREEN = "\u001B[?1049h\u001B[2J\u001B[H"

        /** 离开备用屏：DECRST 1049。 */
        private const val LEAVE_ALT_SCREEN = "\u001B[?1049l"
    }

    @get:Rule
    val notificationPermission =
        GrantPermissionRule.grant(android.Manifest.permission.POST_NOTIFICATIONS)

    @get:Rule val composeTestRule = createAndroidComposeRule<MainActivity>()

    private fun bridge() = composeTestRule.getBridge() ?: throw AssertionError("bridge null")

    /** 活动会话 id：备用屏开关必须作用在**当前可见**的会话上，故不用临时会话。 */
    private fun sessionId(): Long {
        var id = 0L
        InstrumentationRegistry.getInstrumentation().runOnMainSync {
            id = composeTestRule.activity.runtime.inputTargetSessionId
        }
        return id
    }

    /** 网格行列（原生 PTY 口径）。 */
    private fun gridRowsCols(): Pair<Int, Int> {
        val packed = bridge().getGridRowsColsPacked()
        return ((packed shr 32).toInt()) to (packed and 0xffffffffL).toInt()
    }

    /** 网格公式用到的三个量：已按 sp→px 放大的单元格高、键栏高、Surface 高。 */
    private fun gridInputs(): Triple<Float, Int, Int> {
        var cellHeight = 0f
        var barPx = 0
        var surfaceHeight = 0
        InstrumentationRegistry.getInstrumentation().runOnMainSync {
            val runtime = composeTestRule.activity.runtime
            cellHeight = runtime.cellHeight
            barPx = runtime.modifierBarHeightPx
            surfaceHeight = findTerminalSurface(composeTestRule.activity).height
        }
        return Triple(cellHeight, barPx, surfaceHeight)
    }

    private fun imeHeightPx(): Int {
        var height = 0
        InstrumentationRegistry.getInstrumentation().runOnMainSync {
            height =
                findTerminalSurface(composeTestRule.activity)
                    .rootWindowInsets
                    ?.getInsets(WindowInsets.Type.ime())
                    ?.bottom ?: 0
        }
        return height
    }

    /** 底部导航条高度（px）：输入法遮挡高度会扣除它，两者重叠不重复计入。 */
    private fun navigationHeightPx(): Int {
        var height = 0
        InstrumentationRegistry.getInstrumentation().runOnMainSync {
            height =
                findTerminalSurface(composeTestRule.activity)
                    .rootWindowInsets
                    ?.getInsets(WindowInsets.Type.navigationBars())
                    ?.bottom ?: 0
        }
        return height
    }

    /** 失败诊断：把重排依赖的三个状态源一并带出（备用屏流、IME 内边距、网格）。 */
    private fun diagnostics(): String =
        "备用屏流=${altScreenPublished()} 原生备用屏=${NativeBridge.getAltScreenState(sessionId())} " +
            "imeInsets=${imeHeightPx()} navInsets=${navigationHeightPx()} 网格=${gridRowsCols()}"

    /** 运行期逐帧发布的备用屏状态（网格重排读的就是它）。 */
    private fun altScreenPublished(): Boolean {
        var published = false
        InstrumentationRegistry.getInstrumentation().runOnMainSync {
            published = composeTestRule.activity.runtime.altScreenActiveFlow.value
        }
        return published
    }

    /**
     * 弹出输入法并等高度定居。
     *
     * 走 `WindowInsetsController.show(Type.ime())`——与终端轻点、抽屉键盘按钮同一条
     * 路径（见 `TerminalScreen.toggleKeyboard` 的注释）：`InputMethodManager`
     * 的 `SHOW_IMPLICIT` 在 Android 12+ 会被静默拒绝（输入法可见性需要受信任手势）。
     */
    private fun showImeAndSettle() {
        val surface = findTerminalSurface(composeTestRule.activity)
        composeTestRule.activity.runOnUiThread {
            surface.requestFocus()
            surface.windowInsetsController?.show(WindowInsets.Type.ime())
        }
        val shown =
            UxTestUtils.pollUntilTrue(timeoutMs = IME_TIMEOUT_MS, intervalMs = 200) {
                var visible = false
                InstrumentationRegistry.getInstrumentation().runOnMainSync {
                    visible =
                        surface.rootWindowInsets?.isVisible(WindowInsets.Type.ime()) == true
                }
                visible
            }
        assertNotNull("输入法必须弹出", shown)
        // 高度必须定居：键盘动画可延续数秒，未稳即量会读到中间高度。
        var stableReads = 0
        var last = -1
        val deadline = android.os.SystemClock.uptimeMillis() + 15_000L
        while (android.os.SystemClock.uptimeMillis() < deadline) {
            val height = imeHeightPx()
            stableReads = if (height == last && height > 0) stableReads + 1 else 1
            last = height
            if (stableReads >= 3) break
            Thread.sleep(500)
        }
        assertTrue("输入法高度必须定居 (末次=$last)", stableReads >= 3)
        Thread.sleep(SETTLE_MILLIS)
    }

    private fun hideImeAndAwaitRows(rows: Int) {
        composeTestRule.activity.runOnUiThread {
            val imm =
                composeTestRule.activity.getSystemService(android.content.Context.INPUT_METHOD_SERVICE)
                    as android.view.inputmethod.InputMethodManager
            imm.hideSoftInputFromWindow(
                findTerminalSurface(composeTestRule.activity).windowToken,
                0,
            )
        }
        val restored =
            UxTestUtils.pollUntilTrue(timeoutMs = GRID_TIMEOUT_MS, intervalMs = 200) {
                gridRowsCols().first == rows
            }
        // 只记日志不报错：收尾发生在 `finally` 里，主体已失败时它会顶掉真正的
        // 失败原因（读者只剩一个脱离上下文的 AssertionError）。
        android.util.Log.i(
            "AltScreenIme",
            "cleanup: grid back to ${gridRowsCols().first} (want $rows), restored=$restored",
        )
    }

    /**
     * 键盘已展开时启动 helix 也必须重排：此时输入法高度未变、平台不会再次派发
     * insets，故只有备用屏状态翻转这一个触发点——漏掉它，TUI 会按整屏行数布局，
     * 状态行留在键盘底下。
     */
    @Test
    fun startingTuiWhileImeIsUpStillShrinksGrid() {
        composeTestRule.waitForSession()
        composeTestRule.waitForTerminalPixels()
        composeTestRule.awaitBridge()
        val (rowsBefore, _) = gridRowsCols()
        try {
            showImeAndSettle()
            val rowsWithImeOnPrimary = gridRowsCols().first
            assertTrue(
                "主屏下输入法不得改变行数（前 $rowsBefore，键盘展开后 $rowsWithImeOnPrimary）",
                rowsWithImeOnPrimary == rowsBefore,
            )

            NativeBridge.feedTerminal(sessionId(), ENTER_ALT_SCREEN.toByteArray(Charsets.UTF_8))
            assertNotNull(
                "必须进入备用屏",
                UxTestUtils.pollUntilTrue(timeoutMs = GRID_TIMEOUT_MS, intervalMs = 100) {
                    NativeBridge.getAltScreenState(sessionId())
                },
            )
            assertNotNull(
                "备用屏状态必须随帧发布到运行期流（${diagnostics()}）",
                UxTestUtils.pollUntilTrue(timeoutMs = GRID_TIMEOUT_MS, intervalMs = 100) {
                    altScreenPublished()
                },
            )
            assertNotNull(
                "键盘已展开时进入备用屏，网格必须收缩（$rowsBefore → ${gridRowsCols().first}；${diagnostics()}）",
                UxTestUtils.pollUntilTrue(timeoutMs = GRID_TIMEOUT_MS, intervalMs = 200) {
                    gridRowsCols().first < rowsBefore
                },
            )
        } finally {
            NativeBridge.feedTerminal(sessionId(), LEAVE_ALT_SCREEN.toByteArray(Charsets.UTF_8))
            hideImeAndAwaitRows(rowsBefore)
        }
    }

    /**
     * 键盘保持展开时离开备用屏：网格必须按整屏高度复原。
     *
     * 反向路径同样要钉住——它与「进入时收缩」共用一次防抖重排，但扣减量来自运行期的
     * 单一值；漏掉离开这一侧时，PTY 会停留在被输入法缩小后的行数，且此后没有任何
     * 自愈触发点（要等下一次旋转或改字号才复原）。
     */
    @Test
    fun leavingAltScreenWhileImeIsUpRestoresFullHeightRows() {
        composeTestRule.waitForSession()
        composeTestRule.waitForTerminalPixels()
        composeTestRule.awaitBridge()
        val (rowsBefore, colsBefore) = gridRowsCols()
        try {
            NativeBridge.feedTerminal(sessionId(), ENTER_ALT_SCREEN.toByteArray(Charsets.UTF_8))
            assertNotNull(
                "必须进入备用屏",
                UxTestUtils.pollUntilTrue(timeoutMs = GRID_TIMEOUT_MS, intervalMs = 100) {
                    NativeBridge.getAltScreenState(sessionId())
                },
            )
            assertNotNull(
                "备用屏状态必须随帧发布到运行期流（${diagnostics()}）",
                UxTestUtils.pollUntilTrue(timeoutMs = GRID_TIMEOUT_MS, intervalMs = 100) {
                    altScreenPublished()
                },
            )
            showImeAndSettle()
            assertNotNull(
                "备用屏下弹出输入法后网格必须收缩（$rowsBefore → ${gridRowsCols().first}；${diagnostics()}）",
                UxTestUtils.pollUntilTrue(timeoutMs = GRID_TIMEOUT_MS, intervalMs = 200) {
                    gridRowsCols().first < rowsBefore
                },
            )
            val rowsInAltScreen = gridRowsCols().first

            // 键盘仍展开，直接离开备用屏。
            NativeBridge.feedTerminal(sessionId(), LEAVE_ALT_SCREEN.toByteArray(Charsets.UTF_8))
            assertNotNull(
                "必须离开备用屏",
                UxTestUtils.pollUntilTrue(timeoutMs = GRID_TIMEOUT_MS, intervalMs = 100) {
                    !NativeBridge.getAltScreenState(sessionId()) &&
                        !altScreenPublished()
                },
            )
            assertNotNull(
                "键盘仍展开时离开备用屏，网格必须复原到 $rowsBefore 行（仍为 $rowsInAltScreen；" +
                    diagnostics() + "）",
                UxTestUtils.pollUntilTrue(timeoutMs = GRID_TIMEOUT_MS, intervalMs = 200) {
                    gridRowsCols().first == rowsBefore
                },
            )
            assertTrue(
                "列数不得变化（前 $colsBefore，后 ${gridRowsCols().second}）",
                gridRowsCols().second == colsBefore,
            )
        } finally {
            NativeBridge.feedTerminal(sessionId(), LEAVE_ALT_SCREEN.toByteArray(Charsets.UTF_8))
            hideImeAndAwaitRows(rowsBefore)
        }
    }

    @Test
    fun altScreenGridShrinksToVisibleHeightWhileImeIsUp() {
        composeTestRule.waitForSession()
        composeTestRule.waitForTerminalPixels()
        composeTestRule.awaitBridge()
        val (rowsBefore, colsBefore) = gridRowsCols()
        try {
            assertTrue("前置网格行列必须为正，实际 ${rowsBefore}x$colsBefore", rowsBefore > 0 && colsBefore > 0)

            // 经 feedTerminal 直接喂 VT 序列：写 PTY 只有 TUI 应用才会响应，
            // shell 会原样吞掉，备用屏永远不会激活。
            NativeBridge.feedTerminal(sessionId(), ENTER_ALT_SCREEN.toByteArray(Charsets.UTF_8))
            val entered =
                UxTestUtils.pollUntilTrue(timeoutMs = GRID_TIMEOUT_MS, intervalMs = 100) {
                    NativeBridge.getAltScreenState(sessionId())
                }
            assertNotNull("必须进入备用屏", entered)
            // 必须等到逐帧发布的备用屏状态也翻转：网格重排读的是运行期发布的值，
            // 只等原生原子（更早更新）会让重排在旧状态下算出行数。
            val published =
                UxTestUtils.pollUntilTrue(timeoutMs = GRID_TIMEOUT_MS, intervalMs = 100) {
                    altScreenPublished()
                }
            assertNotNull("备用屏状态必须随帧发布到运行期流（${diagnostics()}）", published)

            showImeAndSettle()
            val (cellHeight, barPx, surfaceHeight) = gridInputs()
            val imeHeight = imeHeightPx()
            assertTrue("输入法高度必须为正，实际 $imeHeight", imeHeight > 0)
            val shrank =
                UxTestUtils.pollUntilTrue(timeoutMs = GRID_TIMEOUT_MS, intervalMs = 200) {
                    gridRowsCols().first < rowsBefore
                }
            val (rowsWithIme, colsWithIme) = gridRowsCols()
            assertNotNull(
                "备用屏下弹出输入法后网格必须收缩以让全屏 TUI 重排（$rowsBefore → $rowsWithIme；" +
                    diagnostics() + "）",
                shrank,
            )
            assertTrue(
                "列数不得随输入法变化（$colsBefore → $colsWithIme）",
                colsWithIme == colsBefore,
            )
            // 精确对照网格公式：行数 = floor((Surface − 键栏 − 键盘) / 单元格高)。
            // 容差 ±1 行来自减去导航条后的亚像素取整。
            val navigationBottom = navigationHeightPx()
            val expectedRows =
                (
                    (surfaceHeight - barPx - (imeHeight - navigationBottom).coerceAtLeast(0)) /
                        cellHeight
                    ).toInt()
            assertTrue(
                "行数必须等于可见高度容纳的行数（期望≈$expectedRows 实际 $rowsWithIme；" +
                    "cellH=$cellHeight bar=$barPx ime=$imeHeight nav=$navigationBottom surface=$surfaceHeight）",
                rowsWithIme >= expectedRows - 1 && rowsWithIme <= expectedRows + 1,
            )
            android.util.Log.i(
                "AltScreenIme",
                "rows $rowsBefore -> $rowsWithIme (cellH=$cellHeight bar=$barPx ime=$imeHeight)",
            )
        } finally {
            // 顺序与另两个用例一致：先离开备用屏，再收起键盘（`hideImeAndAwaitRows`
            // 顺带确认网格真的复原）。
            NativeBridge.feedTerminal(sessionId(), LEAVE_ALT_SCREEN.toByteArray(Charsets.UTF_8))
            hideImeAndAwaitRows(rowsBefore)
        }
    }

    /**
     * 收起输入法（仍在备用屏）后网格必须复原。
     *
     * 为何这条必须独立成例：另三个用例的 `finally` 都把「收起键盘」当收尾，
     * 而 `hideImeAndAwaitRows` 只记日志不报错——于是「输入法收起 → 遮挡归零 → 重排
     * 复原整屏」这条链路上一次断言都没有。漏掉它时 PTY 会停在被输入法缩小后的
     * 行数，而备用屏 TUI 仍按旧行数布局：状态行回到键盘底下，且没有任何自愈
     * 触发点（要等下一次旋转或改字号才复原）。
     */
    @Test
    fun hidingImeWhileOnAltScreenRestoresFullHeightRows() {
        composeTestRule.waitForSession()
        composeTestRule.waitForTerminalPixels()
        composeTestRule.awaitBridge()
        val (rowsBefore, colsBefore) = gridRowsCols()
        try {
            NativeBridge.feedTerminal(sessionId(), ENTER_ALT_SCREEN.toByteArray(Charsets.UTF_8))
            assertNotNull(
                "必须进入备用屏",
                UxTestUtils.pollUntilTrue(timeoutMs = GRID_TIMEOUT_MS, intervalMs = 100) {
                    NativeBridge.getAltScreenState(sessionId())
                },
            )
            assertNotNull(
                "备用屏状态必须随帧发布到运行期流（${diagnostics()}）",
                UxTestUtils.pollUntilTrue(timeoutMs = GRID_TIMEOUT_MS, intervalMs = 100) {
                    altScreenPublished()
                },
            )
            showImeAndSettle()
            assertNotNull(
                "前置条件：备用屏下弹出输入法后网格必须收缩（$rowsBefore → ${gridRowsCols().first}；" +
                    diagnostics() + "）",
                UxTestUtils.pollUntilTrue(timeoutMs = GRID_TIMEOUT_MS, intervalMs = 200) {
                    gridRowsCols().first < rowsBefore
                },
            )

            // 仍在备用屏，只收起键盘：唯一的触发点是 insets 派发把遮挡归零。
            composeTestRule.activity.runOnUiThread {
                val imm =
                    composeTestRule.activity.getSystemService(android.content.Context.INPUT_METHOD_SERVICE)
                        as android.view.inputmethod.InputMethodManager
                imm.hideSoftInputFromWindow(
                    findTerminalSurface(composeTestRule.activity).windowToken,
                    0,
                )
            }
            assertNotNull(
                "收起输入法后网格必须复原到 $rowsBefore 行（仍为 ${gridRowsCols().first}；" +
                    diagnostics() + "）",
                UxTestUtils.pollUntilTrue(timeoutMs = GRID_TIMEOUT_MS, intervalMs = 200) {
                    gridRowsCols().first == rowsBefore
                },
            )
            assertTrue(
                "列数不得变化（前 $colsBefore，后 ${gridRowsCols().second}）",
                gridRowsCols().second == colsBefore,
            )
        } finally {
            NativeBridge.feedTerminal(sessionId(), LEAVE_ALT_SCREEN.toByteArray(Charsets.UTF_8))
            hideImeAndAwaitRows(rowsBefore)
        }
    }
}
