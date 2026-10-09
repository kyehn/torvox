package terminal.emulator.ui

import androidx.compose.ui.test.junit4.v2.createAndroidComposeRule
import androidx.test.platform.app.InstrumentationRegistry
import androidx.test.rule.GrantPermissionRule
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.junit.runners.JUnit4
import terminal.emulator.MainActivity
import terminal.emulator.TerminalLogcatTest
import terminal.emulator.UxTestUtils
import terminal.emulator.findTerminalSurface
import terminal.emulator.runtime.coerceSpToPxScale
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
 * 判据一律取**原生回读**的单元格度量（`getCellWidth` / `getCellHeight`），不是 Kotlin
 * 自己刚推送的 `appliedFontSizeSp()`——后者对本次缺陷完全无感（见 [applyAndAwait]）。
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

    /**
     * sp→px 系数：走生产用的同一个函数，不自己乘一遍。
     *
     * `TerminalRuntime.coerceSpToPxScale` 会把乘积钳到原生接受的区间（0.5..=8.0）；
     * 测试直接用 `density * fontScale` 时，落在区间外的设备会去测一个调节条
     * 根本划不到的字号。资源取 activity 的：`fontScale` 可被 Activity 的
     * configuration 覆写，target context 上读到的未必是屏幕上生效的那个。
     */
    private fun spToPxScale(): Float {
        val resources = composeTestRule.activity.resources
        return coerceSpToPxScale(
            resources.displayMetrics.density,
            resources.configuration.fontScale,
        )
    }

    /**
     * 原生侧的网格读数：请求字号、列数、单元格宽/高（后两者取自渲染器的字体管线）。
     *
     * 比例判据 MUST 用 `cellWidth`：它是字号的**严格线性**量
     * （`cell_width = advance × font_size / upem`，等宽分支无取整；原生日志实测
     * 4sp → 2.4、96sp → 57.6，正好 24 倍）。`cellHeight` 不线性——同一组数据是
     * 5.0 → 113.0（22.6 倍），因为行高做了 `.ceil()`，跨量级时偏差累积成百分之几。
     * 注意这两个读数的量纲是**字号单位**，物理像素还要乘 `spToPxScale`。
     */
    private data class GridMetrics(val sizeSp: Float, val cols: Int, val cellHeight: Float, val cellWidth: Float)

    /**
     * 该单元格宽度下应有的列数（`floor(视图宽 / 单元格像素宽)`）。
     *
     * 网格宽度是终端 Surface 的宽度而非整屏，故取自视图实测值；查不到时返回 null，
     * 退化为只等宽度。
     */
    private fun expectedColsFor(cellWidth: Float): Int? {
        if (cellWidth <= 0f) return null
        var surfaceWidthPx = 0
        composeTestRule.activityRule.scenario.onActivity { activity: MainActivity ->
            surfaceWidthPx = findTerminalSurface(activity).width
        }
        if (surfaceWidthPx <= 0) return null
        return (surfaceWidthPx / (cellWidth * spToPxScale())).toInt().coerceAtLeast(1)
    }

    private fun readRuntimeMetrics(): GridMetrics {
        var metrics = GridMetrics(0f, 0, 0f, 0f)
        composeTestRule.activityRule.scenario.onActivity { activity: MainActivity ->
            val runtime = activity.runtime
            val bridge = requireNotNull(runtime.bridge())
            val packed = bridge.getGridRowsColsPacked()
            metrics =
                GridMetrics(
                    sizeSp = runtime.appliedFontSizeSp(),
                    cols = (packed and 0xffffffffL).toInt(),
                    cellHeight = bridge.getCellHeight(),
                    cellWidth = bridge.getCellWidth(),
                )
        }
        return metrics
    }

    /**
     * 提交目标字号并等待**原生回读**落地，返回落地后的读数。
     *
     * 判据 MUST 是原生侧 `getCellWidth()` 的线性比例（见 [GridMetrics]）：Kotlin 的
     * `appliedFontSizeSp()` 只是自己刚推送的值，与原生是否接受无关——修复前原生对
     * 超限字号静默 `return Ok(())`，该值照样变成请求值，拿它当判据的用例对本次缺陷
     * 完全无感。原生真被拒收时单元格宽高都不变，故宽度比例是最直接的判别量。
     *
     * @param expectWidthFromMin 由调用方按「本次调用前的单元格宽 → 目标宽度」的线性
     *   关系算出的期望宽度函数，入参是调用前的 `cellWidth`；为 null 时只要求宽度真的
     *   变了（单调同向即可）。
     */
    private fun applyAndAwait(targetSizeSp: Float, expectWidthFromMin: ((Float) -> Float)? = null): GridMetrics {
        val before = readRuntimeMetrics()
        composeTestRule.activityRule.scenario.onActivity { activity: MainActivity ->
            activity.terminalViewModel.setFontSize(targetSizeSp)
        }
        val expectedWidth = expectWidthFromMin?.invoke(before.cellWidth)
        // 列数必须一并等：字号推送后单元格度量立刻变，而网格列数要等 resize 经
        // PTY 往返才反映到运行期状态。只等宽度会在两者之间取样，把「尚未重排」
        // 读成「重排后列数没变」。
        val landed =
            UxTestUtils.pollUntilTrue(timeoutMs = 30_000, intervalMs = 100) {
                val metrics = readRuntimeMetrics()
                val widthOk =
                    if (expectedWidth != null) {
                        kotlin.math.abs(metrics.cellWidth - expectedWidth) <=
                            kotlin.math.max(0.5f, expectedWidth * 0.02f)
                    } else {
                        // 已经在目标字号上时不要求宽度变化——否则轮询会空等满超时，
                        // 把「环境本来就设成该字号」误报成「原生拒收」。
                        kotlin.math.abs(metrics.cellWidth - before.cellWidth) > 0.1f ||
                            kotlin.math.abs(before.sizeSp - targetSizeSp) < 0.01f
                    }
                val expectedCols = expectedColsFor(metrics.cellWidth)
                widthOk && (expectedCols == null || metrics.cols == expectedCols)
            }
        val settled = readRuntimeMetrics()
        // `pollUntilTrue` 返回「条件成立时的耗时」（毫秒），null 即超时。
        assertNotNull(
            "调节条可划到的字号 $targetSizeSp 必须被原生接受并生效：原生单元格宽仍是 " +
                "${before.cellWidth}，期望 ${expectedWidth ?: "变化"}",
            landed,
        )
        android.util.Log.i("FontSizeReflow", "landed after ${landed}ms")
        composeTestRule.waitForIdle()
        return settled
    }

    /**
     * 调节条两端点都必须真的被原生接受。
     *
     * 覆盖面说明：本用例验证的是**本设备**上的端点落地；跨密度的上界不变量
     * （原生上界 ≥ 任一系数下由 Termux 换算出的可选上界）由 Rust 侧
     * `font_size_cap_tests::cap_never_rejects_a_selectable_font_size` 覆盖——它才能
     * 遍历 0.5..=8.0 的全系数区间。真机上若把原生上界退回旧的 100sp 魔数，本设备
     * （density 2.625 ⇒ 上界 96sp）仍会通过该用例，所以两端都必须保留。
     */
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
            // 先落到下端量出原生单元格宽，再以「字号比 × 该宽度」预测上端的宽度。
            // 判据完全落在原生回读量上：上端被原生拒收时宽度不会按比例变大。
            val min = applyAndAwait(minSizeSp)
            val colsAtMin = min.cols
            assertTrue("最小字号下列数必须为正，实际 $colsAtMin", colsAtMin > 0)
            assertTrue("最小字号下单元格宽必须为正，实际 ${min.cellWidth}", min.cellWidth > 0f)
            val expectedWidthAtMax = min.cellWidth * (maxSizeSp / minSizeSp)
            val max = applyAndAwait(maxSizeSp) { expectedWidthAtMax }
            val colsAtMax = max.cols
            assertTrue("最大字号下列数必须为正，实际 $colsAtMax", colsAtMax > 0)
            assertTrue("最大字号下单元格宽必须为正，实际 ${max.cellWidth}", max.cellWidth > 0f)
            // 调节条末端必须真的划得到原生接受的上界：被原生静默丢弃时，
            // 宽度会停在当前字号的值，而不是按比例放大到 $expectedWidthAtMax。
            assertEquals(
                "调节条上端 ${maxSizeSp}sp 必须被原生接受：原生单元格宽 ${max.cellWidth}，" +
                    "按 min→max 比例应为 $expectedWidthAtMax",
                expectedWidthAtMax,
                max.cellWidth,
                kotlin.math.max(0.5f, expectedWidthAtMax * 0.02f),
            )
            assertTrue(
                "字号由 ${minSizeSp}sp 升到 ${maxSizeSp}sp，单元格宽必须变大" +
                    "（前 ${min.cellWidth} 后 ${max.cellWidth}）",
                max.cellWidth > min.cellWidth,
            )
            assertTrue(
                "字号由 ${minSizeSp}sp 升到 ${maxSizeSp}sp，单元格高必须变大" +
                    "（前 ${min.cellHeight} 后 ${max.cellHeight}）",
                max.cellHeight > min.cellHeight,
            )
            assertTrue(
                "字号放大后列数必须收缩（$colsAtMin → $colsAtMax）",
                colsAtMax < colsAtMin,
            )
            android.util.Log.i(
                "FontSizeReflow",
                "scale=$scale min=$minSizeSp cols=$colsAtMin cell=${min.cellWidth}x${min.cellHeight} " +
                    "max=$maxSizeSp cols=$colsAtMax cell=${max.cellWidth}x${max.cellHeight}",
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
            val before = readRuntimeMetrics()
            val originalSizeSp = before.sizeSp
            val colsBefore = before.cols
            val cellHeightBefore = before.cellHeight
            val cellWidthBefore = before.cellWidth
            android.util.Log.i(
                "FontSizeReflow",
                "before sizeSp=$originalSizeSp cols=$colsBefore cellH=$cellHeightBefore",
            )
            assertTrue("应用字号必须为正, 实际: $originalSizeSp", originalSizeSp > 0)
            assertTrue("网格列数必须为正, 实际: $colsBefore", colsBefore > 0)
            assertTrue("单元格高必须为正, 实际: $cellHeightBefore", cellHeightBefore > 0)
            assertTrue("单元格宽必须为正, 实际: $cellWidthBefore", cellWidthBefore > 0)

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
            // 落地判据为原生单元格宽按字号比缩放（±2%，宽度是严格线性的）。
            val expectedWidthTarget = cellWidthBefore * (targetSizeSp / originalSizeSp)
            val landed = applyAndAwait(targetSizeSp) { expectedWidthTarget }
            val landedSizeSp = landed.sizeSp
            val colsAfter = landed.cols
            val cellHeightAfter = landed.cellHeight
            android.util.Log.i("FontSizeReflow", "landed sizeSp=$landedSizeSp expected=$targetSizeSp")
            // 实测比例断言而非假设翻倍：钳制/减半路径同样覆盖。
            val ratio = landedSizeSp / originalSizeSp
            assertEquals(
                "原生单元格宽必须随字号按比例变化（原生回读 ${landed.cellWidth}，" +
                    "期望≈$expectedWidthTarget）",
                expectedWidthTarget,
                landed.cellWidth,
                kotlin.math.max(0.5f, expectedWidthTarget * 0.02f),
            )
            // 单元格高按比例断言，容差 = max(1.5 行高, 期望值的 6%)：`cell_metrics` 对
            // 高度做了 `ceil`，跨量级时偏差累积成百分之几，故给到 6%（宽度只需 2%，
            // 因为它严格线性）。
            val expectedHeightTarget = cellHeightBefore * ratio
            assertEquals(
                "原生单元格高必须随字号按比例变化（原生回读 $cellHeightAfter，" +
                    "期望≈$expectedHeightTarget）",
                expectedHeightTarget,
                cellHeightAfter,
                kotlin.math.max(1.5f, expectedHeightTarget * 0.06f),
            )
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
        } finally {
            composeTestRule.activityRule.scenario.onActivity { activity: MainActivity ->
                activity.terminalViewModel.setFontSize(SettingsRepository.defaultFontSizeFor(widthDp))
            }
        }
    }
}
