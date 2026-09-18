package terminal.emulator.ui

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
import terminal.emulator.UxTestUtils
import terminal.emulator.settings.SettingsRepository
import terminal.emulator.waitForSession

/**
 * 字号设置值与实际渲染尺寸的对照（对标 sylirre TerminalUiTest.fontSizeChangeReflowsGridAndSession）。
 *
 * 驱动与 pinch 收尾相同的路径（onZoomChanged → viewModel.setFontSize → 全量应用 +
 * 单次网格重排）；手势数学本身是框架代码，此处驱动落地路径：字号翻倍后列数收缩、
 * 单元格高按比例增长、应用字号与设置值一致。字号持久化在 SharedPreferences，
 * finally 恢复原值防污染其他测试。
 */
@RunWith(JUnit4::class)
class FontSizeReflowInstrumentedTest {
    @get:Rule
    val notificationPermission =
        GrantPermissionRule.grant(android.Manifest.permission.POST_NOTIFICATIONS)

    @get:Rule val composeTestRule = createAndroidComposeRule<MainActivity>()

    private fun readRuntimeMetrics(): Triple<Float, Int, Float> {
        var sizeSp = 0f
        var cols = 0
        var cellHeight = 0f
        composeTestRule.activityRule.scenario.onActivity { activity: MainActivity ->
            val runtime = activity.runtime
            sizeSp = runtime.appliedFontSizeSp()
            val bridge = runtime.bridge()
            val packed = requireNotNull(bridge).getGridRowsColsPacked()
            cols = (packed and 0xffffffffL).toInt()
            cellHeight = requireNotNull(bridge).getCellHeight()
        }
        return Triple(sizeSp, cols, cellHeight)
    }

    @Test
    fun fontSizeChangeReflowsGridAndScalesCellHeight() {
        composeTestRule.waitForSession()
        // 桥单次读取：会话孵化中为 null，由调用方轮询重试（getBridge 契约）。
        UxTestUtils.pollUntilTrue(timeoutMs = 30_000, intervalMs = 200) {
            var ready = false
            composeTestRule.activityRule.scenario.onActivity { activity: MainActivity ->
                ready = activity.runtime.bridge() != null
            }
            ready
        }
        try {
            val (originalSizeSp, colsBefore, cellHeightBefore) = readRuntimeMetrics()
            android.util.Log.i("FontSizeReflow", "before sizeSp=$originalSizeSp cols=$colsBefore cellH=$cellHeightBefore")
            assertTrue("应用字号必须为正, 实际: $originalSizeSp", originalSizeSp > 0)
            assertTrue("网格列数必须为正, 实际: $colsBefore", colsBefore > 0)
            assertTrue("单元格高必须为正, 实际: $cellHeightBefore", cellHeightBefore > 0)

            // 目标字号钳制到手势路径同界 (14..48)：翻倍越界会被钳制导致“未落地”误报；
            // 若已处上限则改走减半，保证尺寸真实变化（变化本身是后续断言的前提）。
            val doubled = (originalSizeSp * 2f).coerceIn(14f, 48f)
            val targetSizeSp =
                if (doubled > originalSizeSp + 0.01f) {
                    doubled
                } else {
                    (originalSizeSp / 2f).coerceIn(14f, 48f)
                }
            // 与 pinch 收尾同路径：onZoomChanged → viewModel.setFontSize（协程异步全量应用）。
            composeTestRule.activityRule.scenario.onActivity { activity: MainActivity ->
                activity.terminalViewModel.setFontSize(targetSizeSp)
            }
            val applied =
                UxTestUtils.pollUntilTrue(timeoutMs = 30_000, intervalMs = 100) {
                    val (sizeSp, _, _) = readRuntimeMetrics()
                    kotlin.math.abs(sizeSp - targetSizeSp) < 0.01f
                }
            val (landedSizeSp, _, _) = readRuntimeMetrics()
            android.util.Log.i("FontSizeReflow", "landed sizeSp=$landedSizeSp expected=$targetSizeSp")
            assertNotNull("设置字号必须落地: $targetSizeSp, 实际: $landedSizeSp", applied)
            composeTestRule.waitForIdle()
            val (_, colsAfter, cellHeightAfter) = readRuntimeMetrics()
            android.util.Log.i("FontSizeReflow", "after cols=$colsAfter cellH=$cellHeightAfter")
            // 实测比例断言而非假设翻倍：钳制/减半路径同样覆盖。
            val ratio = landedSizeSp / originalSizeSp
            assertTrue("字号必须真实变化 (前=$originalSizeSp 后=$landedSizeSp)", kotlin.math.abs(ratio - 1f) > 0.01f)
            if (ratio > 1f) {
                assertTrue(
                    "字号增大后列数必须收缩 (前=$colsBefore 后=$colsAfter)",
                    colsAfter < colsBefore,
                )
            } else {
                assertTrue(
                    "字号减小后列数必须扩张 (前=$colsBefore 后=$colsAfter)",
                    colsAfter > colsBefore,
                )
            }
            // 字体设置值与实际渲染尺寸的对照：单元格高按实测比例缩放（±10% 度量方差）。
            val expectedCellHeight = cellHeightBefore * ratio
            assertTrue(
                "单元格高必须按比例缩放 (前=$cellHeightBefore 后=$cellHeightAfter 期望≈$expectedCellHeight)",
                cellHeightAfter > expectedCellHeight * 0.9f && cellHeightAfter < expectedCellHeight * 1.1f,
            )
        } finally {
            // 恢复规范默认值（持久化在 SharedPreferences，防污染其他测试与后续复跑）。
            composeTestRule.activityRule.scenario.onActivity { activity: MainActivity ->
                activity.terminalViewModel.setFontSize(SettingsRepository.DEFAULT_FONT_SIZE)
            }
        }
    }
}
