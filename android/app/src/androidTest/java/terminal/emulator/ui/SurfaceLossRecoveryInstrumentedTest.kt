package terminal.emulator.ui

import androidx.compose.ui.graphics.toArgb
import androidx.compose.ui.test.junit4.v2.createAndroidComposeRule
import androidx.test.platform.app.InstrumentationRegistry
import androidx.test.rule.GrantPermissionRule
import androidx.test.uiautomator.By
import androidx.test.uiautomator.UiDevice
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.junit.runners.JUnit4
import terminal.emulator.MainActivity
import terminal.emulator.TerminalLogcatTest
import terminal.emulator.UxTestUtils
import terminal.emulator.awaitShellPrompt
import terminal.emulator.findTerminalSurface
import terminal.emulator.getBridge
import terminal.emulator.ui.theme.ThemeRepository
import terminal.emulator.ui.theme.resolveAppDarkMode
import terminal.emulator.ui.theme.resolveTerminalThemeName
import terminal.emulator.util.runCatchingCancellable
import terminal.emulator.waitForSession
import terminal.emulator.waitForTerminalPixels

/**
 * 原生 surface 失效后的自愈：持续注入 surface 级失败 → 原生判死并上报 →
 * 宿主换掉 `SurfaceView` 取到新窗口 → 画面重新有墨迹且新命令仍能落格。
 *
 * 真机上「BufferQueue 被遗弃」由 SurfaceFlinger 回收制造，应用侧无法复现；
 * 不注入就只能等系统回收，本路径永远没有回归护栏（模拟器全量套件实测：
 * 废弃后 22 分钟 3074 帧全败，27 个用例退化为「零像素」断言）。
 */
@RunWith(JUnit4::class)
class SurfaceLossRecoveryInstrumentedTest : TerminalLogcatTest() {
    companion object {
        /** 注入后等待自愈的轮询上限：换视图 + 新窗口交付 + 首帧，模拟器上给足余量。 */
        private const val RECOVERY_TIMEOUT_MS = 30_000L

        /** 轮询间隔。 */
        private const val RECOVERY_POLL_MILLIS = 300L

        /** 等待画面出现墨迹的轮询上限。 */
        private const val INK_TIMEOUT_MS = 30_000L

        /** 判定「画面有墨迹」的最低墨量（相对背景的像素差之和）。 */
        private const val MIN_INK_DELTA = 4_000L

        /** 采样步长（x 与 y 同用）。 */
        private const val INK_SAMPLE_STEP_PX = 3

        /**
         * 区域内相对中位色的最大偏差上限：纯色覆盖时只有边缘抗锯齿会跳一两点；
         * 透出桌面时壁纸/图标的像素会立刻撑开这个值。
         */
        private const val COVER_SPREAD_TOLERANCE = 8

        /**
         * 中位色相对终端底色的上限：屏幕合成与色彩管理会让纯色整体偏移几级，
         * 但仍远小于「透出别的画面」的差异（后者是几十以上）。
         */
        private const val COVER_COLOR_TOLERANCE = 24
    }

    /**
     * 终端底色：与 [terminal.emulator.ui.TerminalScreen] 同一条解析链
     * （应用深浅 → 终端主题名 → 目录），而不是写死某个十六进制值。
     * 主题换色时测试跟着走，不会因为基准过期而误判「透出桌面」。
     */
    private val terminalBackgroundArgb: Int by lazy {
        val state = composeTestRule.activity.terminalViewModel.settings.value
        val configuration = composeTestRule.activity.resources.configuration
        val systemDark =
            (configuration.uiMode and android.content.res.Configuration.UI_MODE_NIGHT_MASK) ==
                android.content.res.Configuration.UI_MODE_NIGHT_YES
        val isDark = resolveAppDarkMode(state.appThemeMode, systemDark)
        val name =
            resolveTerminalThemeName(
                mode = state.themeMode,
                fixedName = state.themeName,
                dayName = state.dayThemeName,
                nightName = state.nightThemeName,
                isDark = isDark,
            )
        val theme = ThemeRepository.byNameOrNull(name) ?: ThemeRepository.byName(ThemeRepository.defaultDarkName())
        theme.background.toArgb()
    }

    @get:Rule
    val notificationPermission =
        GrantPermissionRule.grant(android.Manifest.permission.POST_NOTIFICATIONS)

    @get:Rule val composeTestRule = createAndroidComposeRule<MainActivity>()

    private lateinit var device: UiDevice

    @Before
    fun setUp() {
        device = UiDevice.getInstance(InstrumentationRegistry.getInstrumentation())
        composeTestRule.waitForSession()
        // 本类量像素墨迹（`inkPixels` 采样），须再等原生 Surface 挂上，否则量到全零——
        // 判红原因与被测的 Surface 丢失恢复行为无关。见 waitForTerminalPixels。
        composeTestRule.waitForTerminalPixels()
        assertNotNull(
            "运行时桥必须就绪",
            UxTestUtils.pollUntilTrue(timeoutMs = 30_000, intervalMs = 200) {
                composeTestRule.getBridge() != null
            },
        )
    }

    private fun bridge() = composeTestRule.getBridge() ?: throw AssertionError("bridge null")

    private fun pumpAndText(): String? {
        runCatchingCancellable {
            terminal.emulator.bridge.NativeBridge
                .pollEvent()
        }
        return bridge().getTerminalText()
    }

    /** 经 shell 打印一行，回显重发至多 3 次（冷 stdin 竞态，同其他用例口径）。 */
    private fun printAndAwait(command: String, needle: String) {
        awaitShellPrompt(bridge())
        var seen: Long? = null
        repeat(3) {
            bridge().writeToPty("$command\n".toByteArray(Charsets.UTF_8))
            seen =
                UxTestUtils.pollUntilTrue(timeoutMs = 30_000, intervalMs = 100) {
                    pumpAndText()?.contains(needle) == true
                }
            if (seen != null) return
        }
        assertNotNull("标记必须落格: $needle", seen)
    }

    /**
     * 终端 Surface 区域内相对背景的墨量（色差之和）：判定「画面真的有内容」。
     *
     * 返回 `null` 表示截图不可用：换视图期间窗口正在拆装，UiAutomator 的截图服务会
     * 偶发产出无法解码的文件（实测 `takeScreenshot produced an undecodable file`）。
     * 那是观测手段失效，不是被测行为，故由调用方按「继续等」处理。
     */
    private fun terminalInk(): Long? = runCatchingCancellable { terminalInkOrThrow() }.getOrNull()

    private fun terminalInkOrThrow(): Long {
        val bitmap = UxTestUtils.screenshot(device)
        val surface = findTerminalSurface(composeTestRule.activity)
        val location = IntArray(2)
        surface.getLocationOnScreen(location)
        val left = location[0].coerceIn(0, bitmap.width - 1)
        val top = location[1].coerceIn(0, bitmap.height - 1)
        val right = (left + surface.width).coerceIn(left + 1, bitmap.width)
        val bottom = (top + surface.height / 2).coerceIn(top + 1, bitmap.height)
        var inkPixels = 0L
        var pixelY = top
        while (pixelY < bottom) {
            var pixelX = left
            while (pixelX < right) {
                inkPixels +=
                    kotlin.math.abs(bitmap.getPixel(pixelX, pixelY) - terminalBackgroundArgb).toLong() and 0xFFFFFF
                pixelX += INK_SAMPLE_STEP_PX
            }
            pixelY += INK_SAMPLE_STEP_PX
        }
        return inkPixels
    }

    private fun awaitInk(phase: String): Long {
        val seen =
            UxTestUtils.pollUntilTrue(timeoutMs = INK_TIMEOUT_MS, intervalMs = RECOVERY_POLL_MILLIS) {
                (terminalInk() ?: 0L) >= MIN_INK_DELTA
            }
        assertNotNull("$phase 终端画面必须已有墨迹（截图或渲染均无内容）", seen)
        return terminalInk() ?: 0L
    }

    @Test
    fun persistentSurfaceLossTriggersHostRebuildAndKeepsRendering() {
        // 前置：确认画面本来就有内容（否则「恢复后有墨迹」可能是从未有过墨迹）。
        printAndAwait("echo PRE_INK_MARKER", "PRE_INK_MARKER")
        val inkBefore = awaitInk("注入前：")

        val surfaceBefore = findTerminalSurface(composeTestRule.activity)
        assertTrue("失效注入必须被原生接受", bridge().setSurfaceLossInjectedForTest(true))

        // 自愈的第一半必须可观测：宿主换掉 SurfaceView（实例变化）才拿到新的原生
        // 窗口。只断言「最后仍有墨迹」是不够的——画面可能从未掉过墨迹而全程未自愈。
        // 每轮必须制造新输出：空闲帧根本不渲染（渲染循环无脏不取纹理），不喂输出就
        // 永远等不到注入生效的那一帧。
        val rebuilt =
            UxTestUtils.pollUntilTrue(timeoutMs = RECOVERY_TIMEOUT_MS, intervalMs = RECOVERY_POLL_MILLIS) {
                bridge().writeToPty("echo TICK\n".toByteArray(Charsets.UTF_8))
                pumpAndText()
                // 必须推进测试时钟：组合的重算与 LaunchedEffect 都由 Compose 测试的帧
                // 时钟驱动，只在 waitForIdle/advanceTimeBy 期间前进。不调用它，界面
                // 在整个轮询里冻结——实测信号已送达、状态已写，视图却始终不换。
                composeTestRule.waitForIdle()
                val current =
                    runCatchingCancellable { findTerminalSurface(composeTestRule.activity) }.getOrNull()
                current != null && current !== surfaceBefore
            }
        assertNotNull(
            "注入 surface 失效后 $RECOVERY_TIMEOUT_MS 内宿主未换新的 SurfaceView（未读到失效位或未请求重建）",
            rebuilt,
        )

        // 关闭注入：此后取纹理恢复真实行为（真实场景由新窗口的重建自然完成）。
        assertTrue("关闭失效注入必须被原生接受", bridge().setSurfaceLossInjectedForTest(false))

        // 自愈的第二半：新窗口必须真的开始出帧。
        awaitInk("换 SurfaceView 后：")

        // 恢复必须是「新窗口 + 真正在渲染」，而非停在最后一帧：再打印一条必须落格。
        printAndAwait("echo POST_RECOVERY_MARKER", "POST_RECOVERY_MARKER")
        awaitInk("恢复后再输出：")
    }

    @Test
    fun selectionMenuSurvivesSurfaceRebuild() {
        // 选区菜单是**独立系统窗口**，由当前那一个 Surface 定位。宿主换 SurfaceView 后
        // 旧 Surface 已脱离窗口，若菜单不随新 Surface 重显，用户看到的是
        // 「已选中文字却没有任何菜单」——选区状态仍在 ViewModel 里，不会自愈。
        printAndAwait("echo SEL_MENU_MARKER", "SEL_MENU_MARKER")
        composeTestRule.activityRule.scenario.onActivity { activity ->
            // 选区取**全屏**（0,0 → 末行末列），既不取固定的「0 行 2..5 列」，也不取
            // 「最后一行」：全新会话的内容写在**顶部**行（提示符就在第 0 行），任何写死的
            // 行都可能在整套连跑时落在空白上——而空文本选区按产品语义不挂「复制/分享」
            // （menuActionsForSelection），菜单其实出现了，断言却被读成菜单缺失，且该分支
            // 不打任何日志。全屏选择与回滚位置、网格尺寸、列宽都无关。
            val packed = bridge().getGridRowsColsPacked()
            val rows = (packed ushr 32).toInt()
            val cols = (packed and 0xFFFFFFFFL).toInt()
            activity.terminalViewModel.startSelection(0, 0)
            activity.terminalViewModel.updateSelection(rows - 1, cols - 1)
            activity.terminalViewModel.endSelection()
            assertTrue(
                "全屏选区必须含文本，否则菜单不挂「复制」",
                activity.terminalViewModel.selectionTextForAction().isNotEmpty(),
            )
        }
        composeTestRule.waitForIdle()
        assertNotNull("选区菜单必须先出现", awaitCopyAction())

        val surfaceBefore = findTerminalSurface(composeTestRule.activity)
        assertTrue("失效注入必须被原生接受", bridge().setSurfaceLossInjectedForTest(true))
        val rebuilt =
            UxTestUtils.pollUntilTrue(timeoutMs = RECOVERY_TIMEOUT_MS, intervalMs = RECOVERY_POLL_MILLIS) {
                bridge().writeToPty("echo TICK\n".toByteArray(Charsets.UTF_8))
                pumpAndText()
                composeTestRule.waitForIdle()
                val current =
                    runCatchingCancellable { findTerminalSurface(composeTestRule.activity) }.getOrNull()
                current != null && current !== surfaceBefore
            }
        assertNotNull("注入 surface 失效后 $RECOVERY_TIMEOUT_MS 内宿主未换新的 SurfaceView", rebuilt)
        assertTrue("关闭失效注入必须被原生接受", bridge().setSurfaceLossInjectedForTest(false))

        assertNotNull("换 SurfaceView 后选区菜单必须重现", awaitCopyAction())
    }

    /**
     * 无缓冲区期间窗口层必须盖住开孔底色，而不是透出后方桌面。
     *
     * 症状是「切应用返回闪一下」：SurfaceView 的原生 surface 位于窗口**之下**，窗口
     * 在这块是透明开孔。缓冲区缺失时若组合层不着色，开孔就透出桌面/上一个应用。
     * 用持续注入把「无缓冲区」拉长成稳定状态，才能既断言运行期标志，又量到像素：
     * 开孔区域必须是**均匀的终端底色**（透出桌面时不会是纯底色）。
     */
    @Test
    fun noBufferWindowIsCoveredByThemeBackground() {
        val runtime = composeTestRule.activity.terminalViewModel.runtime
        composeTestRule.waitForIdle()
        assertNotNull(
            "正常出帧后必须报告「已有缓冲区」，否则覆盖层永远撤不掉",
            UxTestUtils.pollUntilTrue(timeoutMs = 30_000, intervalMs = 100) {
                runtime.surfaceBufferPresent.value
            },
        )

        assertTrue("失效注入必须被原生接受", bridge().setSurfaceLossInjectedForTest(true))
        val surfaceBefore = findTerminalSurface(composeTestRule.activity)
        // 持续注入让新 Surface 也拿不到缓冲：宿主换视图后「无缓冲区」成为稳定态。
        val rebuilt =
            UxTestUtils.pollUntilTrue(timeoutMs = RECOVERY_TIMEOUT_MS, intervalMs = RECOVERY_POLL_MILLIS) {
                bridge().writeToPty("echo COVER_TICK\n".toByteArray(Charsets.UTF_8))
                pumpAndText()
                composeTestRule.waitForIdle()
                val current =
                    runCatchingCancellable { findTerminalSurface(composeTestRule.activity) }.getOrNull()
                current != null && current !== surfaceBefore
            }
        assertNotNull("注入 surface 失效后 $RECOVERY_TIMEOUT_MS 内宿主未换新的 SurfaceView", rebuilt)
        assertNotNull(
            "换视图后、首帧未到时必须报告「无缓冲区」，否则不会着色遮住开孔",
            UxTestUtils.pollUntilTrue(timeoutMs = RECOVERY_TIMEOUT_MS, intervalMs = 50) {
                !runtime.surfaceBufferPresent.value
            },
        )
        // 像素证据要在「注入仍生效」的这一瞬抓：先把画面拿到手，随后立刻撤注入。
        // 反过来先轮询像素会把注入拖上几秒，重建预算被耗光，之后的恢复就无从发生。
        val shot = runCatchingCancellable { UxTestUtils.screenshot(device) }.getOrNull()
        val released = bridge().setSurfaceLossInjectedForTest(false)
        assertTrue("关闭失效注入必须被原生接受（$released）", released)
        val bitmap = checkNotNull(shot) { "无缓冲区期间必须能截到开孔区域" }
        val surface = findTerminalSurface(composeTestRule.activity)
        val location = IntArray(2)
        surface.getLocationOnScreen(location)
        val cover =
            checkNotNull(
                coverPixels(
                    bitmap = bitmap,
                    left = location[0].coerceIn(0, bitmap.width - 1),
                    top = location[1].coerceIn(0, bitmap.height - 1),
                    width = surface.width,
                    height = surface.height,
                ),
            ) { "开孔区域必须量到像素" }
        UxTestUtils.metric("cover_spread", cover.spread.toLong())
        UxTestUtils.metric("cover_median_deviation", cover.medianDeviation.toLong())
        assertTrue(
            "无缓冲区期间开孔区域必须是均匀的终端底色（实测 spread=${cover.spread} " +
                "medianDeviation=${cover.medianDeviation}），否则就是透出了后方画面",
            cover.spread <= COVER_SPREAD_TOLERANCE && cover.medianDeviation <= COVER_COLOR_TOLERANCE,
        )
        // 恢复与「换视图」同源：失效位持续为 1 时宿主会换新 SurfaceView，新窗口出帧后
        // 覆盖层才该撤。每轮都要制造输出——空闲帧不渲染，不喂输出就既驱动不出重试，
        // 也看不到新窗口的首帧。
        assertNotNull(
            "恢复出帧后必须撤销覆盖层",
            UxTestUtils.pollUntilTrue(timeoutMs = RECOVERY_TIMEOUT_MS, intervalMs = RECOVERY_POLL_MILLIS) {
                bridge().writeToPty("echo COVER_RESUME\n".toByteArray(Charsets.UTF_8))
                pumpAndText()
                composeTestRule.waitForIdle()
                runtime.surfaceBufferPresent.value
            },
        )
        awaitInk("恢复后：")
    }

    /** 开孔区域的底色量测：中位色相对终端底色的偏差，以及区域内相对中位色的最大偏差。 */
    private data class CoverPixels(val medianDeviation: Int, val spread: Int)

    /**
     * 采样开孔区域的像素。
     *
     * 判据分两层：`spread` 小表示这块被**均匀**涂了一块纯色（透出桌面时会混进壁纸、
     * 图标、上一应用，spread 立刻变大）；`medianDeviation` 小表示这块的纯色就是终端
     * 底色本身。分开量是因为屏幕合成与色彩管理会让纯色整体偏移一两级，但仍远小于
     * 「透出别的画面」的差异。
     */
    private fun coverPixels(
        bitmap: android.graphics.Bitmap,
        left: Int,
        top: Int,
        width: Int,
        height: Int,
    ): CoverPixels? {
        val right = (left + width).coerceIn(left + 1, bitmap.width)
        val bottom = (top + height / 2).coerceIn(top + 1, bitmap.height)
        val channels = IntArray(3) { index -> index }
        val samples = ArrayList<IntArray>()
        var pixelY = top
        while (pixelY < bottom) {
            var pixelX = left
            while (pixelX < right) {
                val pixel = bitmap.getPixel(pixelX, pixelY)
                samples += intArrayOf((pixel shr 16) and 0xFF, (pixel shr 8) and 0xFF, pixel and 0xFF)
                pixelX += INK_SAMPLE_STEP_PX
            }
            pixelY += INK_SAMPLE_STEP_PX
        }
        if (samples.isEmpty()) return null
        val median =
            channels.map { channel ->
                val sorted = samples.map { it[channel] }.sorted()
                sorted[sorted.size / 2]
            }
        val expected = channels.map { channel -> (terminalBackgroundArgb shr (16 - channel * 8)) and 0xFF }
        val medianDeviation = channels.maxOf { kotlin.math.abs(median[it] - expected[it]) }
        val spread =
            samples.maxOf { sample ->
                channels.maxOf { kotlin.math.abs(sample[it] - median[it]) }
            }
        return CoverPixels(medianDeviation = medianDeviation, spread = spread)
    }

    /** 轮询至选择菜单的「复制」项出现在无障碍树中（返回耗时，null 表示超时）。 */
    private fun awaitCopyAction(): Long? =
        UxTestUtils.pollUntilTrue(timeoutMs = RECOVERY_TIMEOUT_MS, intervalMs = RECOVERY_POLL_MILLIS) {
            device.hasObject(By.text("复制"))
        }
}
