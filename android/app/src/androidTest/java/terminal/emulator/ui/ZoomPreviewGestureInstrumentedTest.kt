package terminal.emulator.ui

import org.junit.Assert.assertNotNull
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith
import org.junit.runners.JUnit4
import terminal.emulator.ComposeTerminalLogcatTest
import terminal.emulator.MainActivity
import terminal.emulator.UxTestUtils
import terminal.emulator.bridge.NativeBridge
import terminal.emulator.findTerminalSurface
import terminal.emulator.settings.SettingsRepository
import terminal.emulator.util.runCatchingCancellable
import terminal.emulator.waitForSession

/**
 * 手势缩放诊断（pinch tick 模拟）：逐 tick 驱动与手势同一回调
 *（surface.onZoomPreview → 预览度量，surface.onZoomChanged → 定稿全量应用），
 * 断言预览即时落地、网格全程非退化、定稿后列数按方向变化、Shell 在
 * SIGWINCH 风暴后仍可交互。只覆盖已声明行为，不锁定预览是否重排网格。
 */
@RunWith(JUnit4::class)
class ZoomPreviewGestureInstrumentedTest : ComposeTerminalLogcatTest() {
    companion object {
        private const val GRID_TIMEOUT_MS = 30_000L
        private const val OUTPUT_TIMEOUT_MS = 20_000L
        private const val SIZE_EPSILON_SP = 0.15f
    }

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

    /** 本机可选字号上界：与调节条、捏合共用同一入口（`SettingsRepository.fontSizeMaxSp`）。 */
    private fun fontSizeMaxSp(): Float {
        var maxSp = 0f
        composeTestRule.activityRule.scenario.onActivity { activity: MainActivity ->
            val runtime = activity.runtime
            maxSp = SettingsRepository.fontSizeMaxSp(runtime.spToPxScale, runtime.screenWidthDp)
        }
        return maxSp
    }

    /**
     * 把字号收敛到本机可设档位：与生产收敛入口（`SettingsRepository.clampFontSize`）同一条。
     *
     * 手势与调节条给出的每个字号都落在档位网格上，故断言「精确落在请求值上」的前提是
     * 请求值本身就在网格上——否则收敛入口会把它挪到最近档位，断言将永远不成立。
     */
    private fun clampToDetent(sizeSp: Float): Float {
        var clamped = 0f
        composeTestRule.activityRule.scenario.onActivity { activity: MainActivity ->
            val runtime = activity.runtime
            clamped = SettingsRepository.clampFontSize(
                sizeSp,
                runtime.spToPxScale,
                runtime.screenWidthDp,
            )
        }
        return clamped
    }

    private fun tickPreview(sizeSp: Float) {
        composeTestRule.activityRule.scenario.onActivity { activity: MainActivity ->
            val surface = findTerminalSurface(activity) as TerminalSurface
            val previewCallback = surface.onZoomPreview
            assertNotNull("手势预览回调必须已接线", previewCallback)
            requireNotNull(previewCallback).invoke(sizeSp)
        }
    }

    /**
     * 基准与目标之间的档位序列（含目标）：生产手势给出的每个字号都落在档位网格上
     * （4sp 起、步长 [SettingsRepository.FONT_SIZE_STEP_SP]），故预览 tick 逐档走。
     *
     * 步数有上界保护：目标若不是从基准走整档能到的值（区间钳位把它截断过），
     * 最后一档直接落到目标，不得死循环。
     */
    private fun detentSequence(baselineSize: Float, targetSize: Float, growing: Boolean): List<Float> {
        val step =
            if (growing) SettingsRepository.FONT_SIZE_STEP_SP else -SettingsRepository.FONT_SIZE_STEP_SP
        val sizes = mutableListOf<Float>()
        var size = baselineSize
        var remaining = 64
        while (size != targetSize && remaining-- > 0) {
            size = if (growing) minOf(size + step, targetSize) else maxOf(size + step, targetSize)
            sizes.add(size)
        }
        return sizes
    }

    private fun pumpedText(): String {
        runCatchingCancellable { NativeBridge.pollEvent() }
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
        // 生产手势给出的每一个字号都落在档位网格上（4sp 起、步长 2sp），故这里也
        // 逐档走而不是线性插值：插值出的非档位值会被收敛入口挪到最近档位，
        // 断言「精确落在请求值上」将永远不成立。
        val maxSizeSp = fontSizeMaxSp()
        val growing = baselineSize < maxSizeSp
        val startSize = clampToDetent(baselineSize)
        val targetSize =
            clampToDetent(
                if (growing) {
                    baselineSize + 8f
                } else {
                    baselineSize - 6f
                },
            )
        assertTrue(
            "目标字号必须真实变化 (基准=$baselineSize 目标=$targetSize)",
            kotlin.math.abs(targetSize - startSize) > 1f,
        )
        assertTrue(
            "目标字号必须落在可设区间内 (上界=$maxSizeSp 目标=$targetSize)",
            targetSize <= maxSizeSp,
        )
        val previewSizes = detentSequence(startSize, targetSize, growing)
        try {
            // 模拟手势 tick（生产节流 60ms，此处 100ms 足够 Native 落地）。
            previewSizes.forEach { previewSize ->
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
            assertNotNull("定稿字号必须落地: $targetSize", finalized)
            // 全量应用经 IO 协程互斥异步重排网格：字号落地不等于重排完成，单次读列在过载机上
            // 恒采到旧值。轮询至列数按方向变化，超时才大声失败，不放宽方向断言。
            val reflowed =
                UxTestUtils.pollUntilTrue(timeoutMs = GRID_TIMEOUT_MS, intervalMs = 200) {
                    val cols = readGridState().third
                    if (growing) cols < baselineCols else cols > baselineCols
                }
            val (_, finalRows, finalCols) = readGridState()
            assertTrue("定稿后网格行数必须为正, 实际: $finalRows", finalRows > 0)
            assertNotNull("定稿后网格必须按方向重排 (前=$baselineCols 后=$finalCols)", reflowed)
            // Shell 在缩放风暴后仍可交互（mksh 遇 SIGWINCH 清提示符，不得卡死会话）。
            val marker = "ZOOMALIVE_${System.currentTimeMillis() % 100000}"
            composeTestRule.activityRule.scenario.onActivity { activity: MainActivity ->
                activity.terminalViewModel.writeToPty(
                    activity.terminalViewModel.runtime.inputTargetSessionId,
                    "echo $marker\n".toByteArray(Charsets.UTF_8),
                )
            }
            val echoed =
                UxTestUtils.pollUntilTrue(timeoutMs = OUTPUT_TIMEOUT_MS, intervalMs = 100) {
                    pumpedText().contains(marker)
                }
            assertNotNull("缩放后 Shell 必须回显标记 $marker", echoed)
        } finally {
            composeTestRule.activityRule.scenario.onActivity { activity: MainActivity ->
                activity.terminalViewModel.setFontSize(startSize)
            }
            val restored =
                UxTestUtils.pollUntilTrue(timeoutMs = GRID_TIMEOUT_MS, intervalMs = 200) {
                    kotlin.math.abs(readGridState().first - startSize) < SIZE_EPSILON_SP
                }
            assertNotNull("字号必须恢复基准 $startSize", restored)
        }
    }
}
