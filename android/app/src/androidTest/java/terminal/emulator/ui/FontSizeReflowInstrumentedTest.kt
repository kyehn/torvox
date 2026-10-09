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
import terminal.emulator.TerminalLogcatTest
import terminal.emulator.UxTestUtils
import terminal.emulator.settings.SettingsRepository
import terminal.emulator.waitForSession

/**
 * 调节条可划到的**每一个端点**都必须是真正生效的字号（对标 sylirre
 * TerminalUiTest.fontSizeChangeReflowsGridAndSession）。
 *
 * 为什么必须跨端点：调节条范围在 Kotlin 定义，字号是否真的落地由原生
 * `setFontSizeInPlace` 决定。两端各有一份上限常量时，中间档全部正常、只有上端
 * 被原生拒收——历史缺陷正是如此（Kotlin 侧 100sp 与原生 `4.0..=100.0` 两份魔数，
 * 低密度设备上 Termux 允许的 256sp 被截断）。只测中间档的用例对该缺陷完全无感。
 *
 * 断言读到的是原生回读的生效字号（`appliedFontSizeSp`）与单元格度量，
 * 故失败必然指向本仓代码，而不是复述被测函数自身。
 */
@RunWith(JUnit4::class)
class FontSizeReflowInstrumentedTest : TerminalLogcatTest() {
    @get:Rule
    val notificationPermission =
        GrantPermissionRule.grant(android.Manifest.permission.POST_NOTIFICATIONS)

    @get:Rule val composeTestRule = createAndroidComposeRule<MainActivity>()

    /** 设备宽度（dp）：字体缺省字号按它自适应，恢复设置时必须用同一口径。 */
    private val widthDp: Float by lazy {
        val metrics = InstrumentationRegistry.getInstrumentation().targetContext.resources.displayMetrics
        metrics.widthPixels / metrics.density
    }

    private fun spToPxScale(): Float {
        val resources = InstrumentationRegistry.getInstrumentation().targetContext.resources
        return resources.displayMetrics.density * resources.configuration.fontScale
    }

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

    /** 提交目标字号并等待原生回读落地，返回 (请求值, 生效值, 列数, 单元格高)。 */
    private fun applyAndAwait(targetSizeSp: Float): Triple<Float, Int, Float> {
        composeTestRule.activityRule.scenario.onActivity { activity: MainActivity ->
            activity.terminalViewModel.setFontSize(targetSizeSp)
        }
        val landed =
            UxTestUtils.pollUntilTrue(timeoutMs = 30_000, intervalMs = 100) {
                val (sizeSp, _, _) = readRuntimeMetrics()
                kotlin.math.abs(sizeSp - targetSizeSp) < 0.01f
            }
        val (landedSizeSp, _, _) = readRuntimeMetrics()
        assertNotNull(
            "调节条可划到的字号 $targetSizeSp 必须被原生接受并生效，实际生效 $landedSizeSp",
            landed,
        )
        composeTestRule.waitForIdle()
        val (_, cols, cellHeight) = readRuntimeMetrics()
        return Triple(landedSizeSp, cols, cellHeight)
    }

    @Test
    fun sliderEndpointsAreActuallyApplied() {
        composeTestRule.waitForSession()
        UxTestUtils.pollUntilTrue(timeoutMs = 30_000, intervalMs = 200) {
            var ready = false
            composeTestRule.activityRule.scenario.onActivity { activity: MainActivity ->
                ready = activity.runtime.bridge() != null
            }
            ready
        }
        try {
            val scale = spToPxScale()
            val minSizeSp = SettingsRepository.FONT_SIZE_MIN_SP
            val maxSizeSp = SettingsRepository.fontSizeMaxSp(scale)
            val (_, colsAtMin, cellHeightAtMin) = applyAndAwait(minSizeSp)
            assertTrue("最小字号下列数必须为正，实际 $colsAtMin", colsAtMin > 0)
            assertTrue("最小字号下单元格高必须为正，实际 $cellHeightAtMin", cellHeightAtMin > 0)
            val (_, colsAtMax, cellHeightAtMax) = applyAndAwait(maxSizeSp)
            assertTrue("最大字号下列数必须为正，实际 $colsAtMax", colsAtMax > 0)
            assertTrue("最大字号下单元格高必须为正，实际 $cellHeightAtMax", cellHeightAtMax > 0)
            assertTrue(
                "字号由 ${minSizeSp}sp 升到 ${maxSizeSp}sp，单元格高必须变大（前 $cellHeightAtMin 后 $cellHeightAtMax）",
                cellHeightAtMax > cellHeightAtMin,
            )
            android.util.Log.i(
                "FontSizeReflow",
                "scale=$scale min=$minSizeSp cols=$colsAtMin cellH=$cellHeightAtMin " +
                    "max=$maxSizeSp cols=$colsAtMax cellH=$cellHeightAtMax",
            )
        } finally {
            // 恢复规范默认值（持久化在 SharedPreferences，防污染其他测试与后续复跑）。
            composeTestRule.activityRule.scenario.onActivity { activity: MainActivity ->
                activity.terminalViewModel.setFontSize(SettingsRepository.defaultFontSizeFor(widthDp))
            }
        }
    }

    @Test
    fun fontSizeChangeReflowsGridAndScalesCellHeight() {
        composeTestRule.waitForSession()
        UxTestUtils.pollUntilTrue(timeoutMs = 30_000, intervalMs = 200) {
            var ready = false
            composeTestRule.activityRule.scenario.onActivity { activity: MainActivity ->
                ready = activity.runtime.bridge() != null
            }
            ready
        }
        try {
            val (originalSizeSp, colsBefore, cellHeightBefore) = readRuntimeMetrics()
            android.util.Log.i(
                "FontSizeReflow",
                "before sizeSp=$originalSizeSp cols=$colsBefore cellH=$cellHeightBefore",
            )
            assertTrue("应用字号必须为正, 实际: $originalSizeSp", originalSizeSp > 0)
            assertTrue("网格列数必须为正, 实际: $colsBefore", colsBefore > 0)
            assertTrue("单元格高必须为正, 实际: $cellHeightBefore", cellHeightBefore > 0)

            // 目标字号取调节条区间内的相邻档：翻倍越界会被钳制导致「未落地」误报；
            // 若已处上限则改走减半，保证尺寸真实变化（变化本身是后续断言的前提）。
            val doubled = (originalSizeSp * 2f).coerceIn(
                SettingsRepository.FONT_SIZE_MIN_SP,
                SettingsRepository.fontSizeMaxSp(spToPxScale()),
            )
            val targetSizeSp =
                if (doubled > originalSizeSp + 0.01f) {
                    doubled
                } else {
                    (originalSizeSp / 2f).coerceIn(
                        SettingsRepository.FONT_SIZE_MIN_SP,
                        SettingsRepository.fontSizeMaxSp(spToPxScale()),
                    )
                }
            val (landedSizeSp, colsAfter, cellHeightAfter) = applyAndAwait(targetSizeSp)
            android.util.Log.i("FontSizeReflow", "landed sizeSp=$landedSizeSp expected=$targetSizeSp")
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
            composeTestRule.activityRule.scenario.onActivity { activity: MainActivity ->
                activity.terminalViewModel.setFontSize(SettingsRepository.defaultFontSizeFor(widthDp))
            }
        }
    }
}