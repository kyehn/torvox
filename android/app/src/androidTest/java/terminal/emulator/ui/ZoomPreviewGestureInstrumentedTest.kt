package terminal.emulator.ui

import androidx.compose.ui.test.junit4.v2.createAndroidComposeRule
import androidx.test.rule.GrantPermissionRule
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.junit.runners.JUnit4
import terminal.emulator.MainActivity
import terminal.emulator.UxTestUtils
import terminal.emulator.bridge.NativeBridge
import terminal.emulator.findTerminalSurface
import terminal.emulator.waitForSession

/**
 * 手势缩放诊断（pinch tick 模拟）：逐 tick 驱动与手势同一回调
 *（surface.onZoomPreview → 预览度量，surface.onZoomChanged → 定稿全量应用），
 * 断言预览即时落地、网格全程非退化、定稿后列数按方向变化、Shell 在
 * SIGWINCH 风暴后仍可交互。只覆盖已声明行为，不锁定预览是否重排网格。
 */
@RunWith(JUnit4::class)
class ZoomPreviewGestureInstrumentedTest {
    companion object {
        private const val GRID_TIMEOUT_MS = 30_000L
        private const val OUTPUT_TIMEOUT_MS = 20_000L
        private const val SIZE_EPSILON_SP = 0.15f
        private const val TICK_COUNT = 6
    }

    @get:Rule
    val notificationPermission =
        GrantPermissionRule.grant(android.Manifest.permission.POST_NOTIFICATIONS)

    @get:Rule val composeTestRule = createAndroidComposeRule<MainActivity>()

    private fun readGridState(): Triple<Float, Int, Int> {
        var appliedSize = 0f
        var rowCount = 0
        var colCount = 0
        composeTestRule.activityRule.scenario.onActivity { activity: MainActivity ->
            val runtime = activity.runtime
            appliedSize = runtime.appliedFontSizeSp()
            val packed = requireNotNull(runtime.bridge()).getGridRowsColsPacked()
            rowCount = (packed shr 32).toInt()
            colCount = (packed and 0xffffffffL).toInt()
        }
        return Triple(appliedSize, rowCount, colCount)
    }

    private fun tickPreview(sizeSp: Float) {
        composeTestRule.activityRule.scenario.onActivity { activity: MainActivity ->
            val surface = findTerminalSurface(activity) as TerminalSurface
            val previewCallback = surface.onZoomPreview
            assertNotNull("手势预览回调必须已接线", previewCallback)
            requireNotNull(previewCallback).invoke(sizeSp)
        }
    }

    private fun pumpedText(): String {
        runCatching { NativeBridge.pollEvent() }
        var text = ""
        composeTestRule.activityRule.scenario.onActivity { activity: MainActivity ->
            text = activity.runtime.bridge()?.getTerminalText().orEmpty()
        }
        return text
    }

    @Test
    fun zoomGestureTicksPreviewThenFinalizeReflowsAndShellSurvives() {
        composeTestRule.waitForSession()
        val bridgeReady =
            UxTestUtils.pollUntilTrue(timeoutMs = GRID_TIMEOUT_MS, intervalMs = 200) {
                var ready = false
                composeTestRule.activityRule.scenario.onActivity { activity: MainActivity ->
                    ready = activity.runtime.bridge() != null
                }
                ready
            }
        assertNotNull("运行时桥必须就绪", bridgeReady)
        val (baselineSize, _, baselineCols) = readGridState()
        assertTrue("基准字号必须为正, 实际: $baselineSize", baselineSize > 0)
        assertTrue("基准列数必须为正, 实际: $baselineCols", baselineCols > 0)
        // 手势界内取目标：上行空间不足则下行，保证尺寸真实变化。
        val growing = baselineSize < 30f
        val targetSize =
            if (growing) {
                (baselineSize + 8f).coerceIn(14f, 40f)
            } else {
                (baselineSize - 6f).coerceIn(14f, 48f)
            }
        assertTrue(
            "目标字号必须真实变化 (基准=$baselineSize 目标=$targetSize)",
            kotlin.math.abs(targetSize - baselineSize) > 1f,
        )
        try {
            // 模拟手势 tick（生产节流 60ms，此处 100ms 足够 Native 落地）。
            repeat(TICK_COUNT) { tickIndex ->
                val previewSize = baselineSize + (targetSize - baselineSize) * (tickIndex + 1) / TICK_COUNT
                tickPreview(previewSize)
                val landed =
                    UxTestUtils.pollUntilTrue(timeoutMs = 10_000, intervalMs = 100) {
                        kotlin.math.abs(readGridState().first - previewSize) < SIZE_EPSILON_SP
                    }
                val (landedSize, tickRows, tickCols) = readGridState()
                assertNotNull("预览字号必须即时落地: $previewSize, 实际: $landedSize", landed)
                assertTrue("预览中网格行数不得退化, 实际: $tickRows", tickRows > 0)
                assertTrue("预览中网格列数不得退化, 实际: $tickCols", tickCols > 0)
                Thread.sleep(100)
            }
            // 手势定稿：与真实 onScaleEnd 同回调（持久化 + 全量应用 + 单次重排）。
            composeTestRule.activityRule.scenario.onActivity { activity: MainActivity ->
                val surface = findTerminalSurface(activity) as TerminalSurface
                val finalizeCallback = surface.onZoomChanged
                assertNotNull("手势定稿回调必须已接线", finalizeCallback)
                requireNotNull(finalizeCallback).invoke(targetSize)
            }
            val finalized =
                UxTestUtils.pollUntilTrue(timeoutMs = GRID_TIMEOUT_MS, intervalMs = 100) {
                    kotlin.math.abs(readGridState().first - targetSize) < SIZE_EPSILON_SP
                }
            composeTestRule.waitForIdle()
            val (_, finalRows, finalCols) = readGridState()
            assertNotNull("定稿字号必须落地: $targetSize", finalized)
            assertTrue("定稿后网格行数必须为正, 实际: $finalRows", finalRows > 0)
            if (growing) {
                assertTrue("字号增大后列数必须收缩 (前=$baselineCols 后=$finalCols)", finalCols < baselineCols)
            } else {
                assertTrue("字号减小后列数必须扩张 (前=$baselineCols 后=$finalCols)", finalCols > baselineCols)
            }
            // Shell 在缩放风暴后仍可交互（mksh 遇 SIGWINCH 清提示符，不得卡死会话）。
            val marker = "ZOOMALIVE_${System.currentTimeMillis() % 100000}"
            composeTestRule.activityRule.scenario.onActivity { activity: MainActivity ->
                activity.terminalViewModel.writeToPty("echo $marker\n".toByteArray(Charsets.UTF_8))
            }
            val echoed =
                UxTestUtils.pollUntilTrue(timeoutMs = OUTPUT_TIMEOUT_MS, intervalMs = 100) {
                    pumpedText().contains(marker)
                }
            assertNotNull("缩放后 Shell 必须回显标记 $marker", echoed)
        } finally {
            composeTestRule.activityRule.scenario.onActivity { activity: MainActivity ->
                activity.terminalViewModel.setFontSize(baselineSize)
            }
            val restored =
                UxTestUtils.pollUntilTrue(timeoutMs = GRID_TIMEOUT_MS, intervalMs = 200) {
                    kotlin.math.abs(readGridState().first - baselineSize) < SIZE_EPSILON_SP
                }
            assertNotNull("字号必须恢复基准 $baselineSize", restored)
        }
    }
}
