package terminal.emulator

import android.content.res.Configuration
import android.graphics.Bitmap
import androidx.compose.ui.graphics.toArgb
import org.junit.After
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Test
import terminal.emulator.ui.theme.AppThemeMode
import terminal.emulator.ui.theme.TerminalThemeMode
import terminal.emulator.ui.theme.ThemeRepository

/**
 * 终端主题跟随系统日夜：系统夜览模式翻转后，原生渲染的终端底色必须跟着换。
 *
 * ## 为什么要有这条
 *
 * 规范要求终端主题在「日夜切换」开启后跟随软件主题在日/夜两套之间切换；软件主题
 * 跟随系统时，系统夜览模式一变，终端底色就必须换。否则界面外壳已经变暗、终端内容
 * 仍是浅底（夜里盯着一块刺眼的亮底），或反之。
 *
 * 判据是呈现像素（PixelCopy 读回 Surface）而不是设置项：设置项写对了但原生渲染器
 * 没收到新主题时，界面照旧是旧底色——这类缺陷只有像素判据能抓住。
 *
 * ## 前置条件
 *
 * 终端主题模式与软件主题模式都设为「跟随系统」，日/夜主题取一对底色与前景都拉得开的
 * 主题（日间用默认浅色，夜间用 tokyo_night，见 [NIGHT_THEME_NAME]）。系统夜览模式由
 * shell 切换：仪器化进程没有改系统设置的权限。
 */
class ThemeFollowSystemInstrumentedTest : ComposeTerminalLogcatTest() {

    private companion object {
        /** 底色命中像素的下限：终端内容区整片都是底色，命中上千即说明整屏已换色。 */
        const val BACKGROUND_PIXEL_FLOOR = 2_000

        /** 等待主题生效的上限（毫秒）：配置变更 → 设置重解析 → 原生换主题 → 重绘。 */
        const val THEME_SWITCH_TIMEOUT_MS = 30_000L

        /** 等待设置落盘的上限（毫秒）。 */
        const val SETTINGS_TIMEOUT_MS = 15_000L

        /**
         * 夜间主题名：刻意不用默认夜间主题。
         *
         * 默认夜间主题（dracula_plus）的前景 #F8F8F2 与日间主题（catppuccin_latte）
         * 的底色 #EFF1F5 各通道差 ≤ 9，而 [UxTestUtils.matchColor] 的容差 24 覆盖
         * 得了——夜间画面里的文字笔画会被数成「日间底色残留」，画面内容一变判据就
         * 漂（实测整屏夜间的字迹被数成 27278 个日间底色像素，用例偶发红）。tokyo_night
         * 的前景 #A9B1D6 与日间底色各通道差 ≥ 31，底色判定不再与文字混淆：残留计数
         * 只可能是没换掉的日间底色。
         */
        const val NIGHT_THEME_NAME = "tokyo_night"
    }

    private var originalNightMode = "no"
    private var originalSettings: terminal.emulator.settings.SettingsRepository.SettingsState? = null

    /** 最近一次底色等待里命中的最多像素数；失败时进断言信息。 */
    private var bestPixelCount = 0

    /** 最近一次底色等待里，带新底色的帧中旧底色的最少残留；null 表示没出现过这种帧。 */
    private var bestResidualCount: Int? = null

    /** 最近一次底色等待里读回失败的次数；失败时进断言信息。 */
    private var captureFailures = 0

    @Before
    fun setUp() {
        composeTestRule.waitForSession()
        composeTestRule.activityRule.scenario.onActivity { activity: MainActivity ->
            originalSettings = activity.terminalViewModel.settings.value
        }
        originalNightMode = readNightMode()
        setNightMode("no", Configuration.UI_MODE_NIGHT_NO)
        composeTestRule.activityRule.scenario.onActivity { activity: MainActivity ->
            val viewModel = activity.terminalViewModel
            viewModel.setThemeMode(TerminalThemeMode.DAY_NIGHT)
            viewModel.setAppThemeMode(AppThemeMode.FOLLOW_SYSTEM)
            viewModel.setDayThemeName(ThemeRepository.defaultLightName())
            viewModel.setNightThemeName(NIGHT_THEME_NAME)
        }
        val applied =
            UxTestUtils.pollUntilTrue(timeoutMs = SETTINGS_TIMEOUT_MS, intervalMs = 100) {
                val stored = readSettings()
                stored.themeMode == TerminalThemeMode.DAY_NIGHT &&
                    stored.appThemeMode == AppThemeMode.FOLLOW_SYSTEM &&
                    stored.dayThemeName == ThemeRepository.defaultLightName() &&
                    stored.nightThemeName == NIGHT_THEME_NAME
            }
        assertNotNull("主题设置没有落盘", applied)

        // 清屏：画面里的文字笔画颜色与底色接近时会被颜色计数算进残留（历史实测
        // 27487 个全是文字笔画，同一 head 上多轮结果随内容漂移）。夜间主题已换成
        // 与日间底色拉得开的 tokyo_night，文字不再与底色混淆；这里仍清屏，让画面
        // 少些干扰，判据回归「底色本身」。
        val bridgeReady =
            UxTestUtils.pollUntilTrue(timeoutMs = 30_000, intervalMs = 200) {
                composeTestRule.getBridge() != null
            }
        assertNotNull("运行时桥必须就绪（30s 未孵化）", bridgeReady)
        checkNotNull(composeTestRule.getBridge())
            .feedTerminal("\u001B[2J\u001B[H".toByteArray(Charsets.UTF_8))
        val cleared =
            UxTestUtils.pollUntilTrue(timeoutMs = 10_000, intervalMs = 100) {
                composeTestRule.pumpedTerminalText().isNullOrBlank()
            }
        assertNotNull("清屏后终端必须无文本（前序输出残留会污染残留判定）", cleared)
    }

    @After
    fun tearDown() {
        val before = originalSettings
        if (before != null) {
            composeTestRule.activityRule.scenario.onActivity { activity: MainActivity ->
                val viewModel = activity.terminalViewModel
                viewModel.setThemeMode(before.themeMode)
                viewModel.setAppThemeMode(before.appThemeMode)
                viewModel.setThemeName(before.themeName)
                viewModel.setDayThemeName(before.dayThemeName)
                viewModel.setNightThemeName(before.nightThemeName)
            }
        }
        // 还原系统夜览模式：它影响设备全局状态，不还原即把后续用例全留在夜间。
        val expected =
            when (originalNightMode) {
                "yes" -> Configuration.UI_MODE_NIGHT_YES
                "no" -> Configuration.UI_MODE_NIGHT_NO
                else -> null
            }
        setNightMode(originalNightMode, expected)
    }

    /**
     * 系统切到夜间后，终端底色必须换成夜间主题的底色；切回日间亦然。
     *
     * 判别力：主题解析读的是「应用进程看到的系统夜览状态」。若配置变更到达时
     * 读到的是旧值，主题名解析成日间主题，终端底色就停在浅底不变。
     */
    @Test
    fun terminal_background_switches_with_system_night_mode() {
        val dayBackground = ThemeRepository.byName(ThemeRepository.defaultLightName()).background.toArgb()
        val nightBackground = ThemeRepository.byName(NIGHT_THEME_NAME).background.toArgb()

        val dayShot = awaitBackground(dayBackground, "day", absentColor = nightBackground)
        assertNotNull(
            "日间主题底色没有出现在终端 Surface 上（命中 $bestPixelCount 像素，读回失败 $captureFailures 次）",
            dayShot,
        )

        setNightMode("yes", Configuration.UI_MODE_NIGHT_YES)
        val nightShot = awaitBackground(nightBackground, "night", absentColor = dayBackground)
        assertNotNull(
            "系统切到夜间后，终端底色仍不是夜间主题底色（命中 $bestPixelCount 像素，" +
                "带夜间底色的帧里日间底色最少剩 ${bestResidualCount ?: "—"} 像素，读回失败 $captureFailures 次）",
            nightShot,
        )

        val dayRemainder = UxTestUtils.matchColor(checkNotNull(nightShot), dayBackground).count
        UxTestUtils.metric("theme_day_pixels_after_night", dayRemainder.toLong())
        assertTrue("夜间画面里仍残留 $dayRemainder 个日间底色像素", dayRemainder < BACKGROUND_PIXEL_FLOOR)

        // 再切回日间：单程切换通过不等于往返都通过（原生侧可能只认第一次换主题）。
        setNightMode("no", Configuration.UI_MODE_NIGHT_NO)
        val backShot = awaitBackground(dayBackground, "day_back", absentColor = nightBackground)
        assertNotNull(
            "系统切回日间后，终端底色仍不是日间主题底色（命中 $bestPixelCount 像素，" +
                "带日间底色的帧里夜间底色最少剩 ${bestResidualCount ?: "—"} 像素，读回失败 $captureFailures 次）",
            backShot,
        )
    }

    /**
     * 等待终端 Surface 呈现 [color] 的底色、且 [absentColor] 的底色已经退场，返回那一帧；
     * 超时返回 null。
     *
     * 两条判据必须落在**同一帧**上：只看「新底色已出现」会把换色过程中的中间帧当成
     * 切换完成（那一帧可能还带着半屏旧底色），故要求旧底色同时退场。
     *
     * 读回很慢（软件渲染设备上一次整屏合成几百毫秒），故按固定节奏取，
     * 不用它做高频轮询——否则量具本身就把渲染循环按住了。
     *
     * 诊断量记在 [bestPixelCount]、[bestResidualCount] 与 [captureFailures]：命中 0 像素
     * 且读回不失败即「画面在但不是这个底色」，读回一直失败即「Surface 上根本没有
     * 画面」——两者的根因完全不同，断言信息里必须能分开。
     *
     * [metricLabel] 是计量的名字段（ASCII）：logcat 里的计量名要能直接 grep。
     */
    private fun awaitBackground(color: Int, metricLabel: String, absentColor: Int): Bitmap? {
        var shot: Bitmap? = null
        bestPixelCount = 0
        bestResidualCount = null
        captureFailures = 0
        UxTestUtils.pollUntilTrue(timeoutMs = THEME_SWITCH_TIMEOUT_MS, intervalMs = 400) {
            // 每轮重新解析视图：Activity 重建后旧的 SurfaceView 引用已失效，
            // 对着失效视图做 PixelCopy 只会一直失败，而视图本身并未出错。
            val surface = UxTestUtils.terminalSurface(composeTestRule.activity)
            if (surface == null) {
                captureFailures++
                return@pollUntilTrue false
            }
            val captured = UxTestUtils.captureSurface(surface)
            if (captured == null) {
                captureFailures++
                return@pollUntilTrue false
            }
            val matched = UxTestUtils.matchColor(captured, color).count
            if (matched > bestPixelCount) bestPixelCount = matched
            if (matched < BACKGROUND_PIXEL_FLOOR) return@pollUntilTrue false
            val residual = UxTestUtils.matchColor(captured, absentColor).count
            val previousBest = bestResidualCount
            if (previousBest == null || residual < previousBest) bestResidualCount = residual
            if (residual >= BACKGROUND_PIXEL_FLOOR) return@pollUntilTrue false
            shot = captured
            true
        }
        UxTestUtils.metric(
            "theme_${metricLabel}_pixels",
            (
                shot?.let {
                    UxTestUtils.matchColor(it, color).count
                } ?: bestPixelCount
                ).toLong(),
        )
        UxTestUtils.metric(
            "theme_${metricLabel}_residual_pixels",
            (
                shot?.let {
                    UxTestUtils.matchColor(it, absentColor).count
                } ?: bestResidualCount ?: -1
                ).toLong(),
        )
        UxTestUtils.metric("theme_${metricLabel}_capture_failures", captureFailures.toLong())
        return shot
    }

    /**
     * 切换系统夜览模式并等应用侧收到配置变更。
     *
     * [expectedNightFlag] 为 null 表示无法从模式名推出生效标志（如 `auto`），
     * 此时只执行命令，不等配置到达。
     */
    private fun setNightMode(mode: String, expectedNightFlag: Int?) {
        UxTestUtils.runShellCommand("cmd uimode night $mode")
        if (expectedNightFlag == null) return
        var actual = -1
        val applied =
            UxTestUtils.pollUntilTrue(timeoutMs = THEME_SWITCH_TIMEOUT_MS, intervalMs = 200) {
                composeTestRule.activityRule.scenario.onActivity { activity: MainActivity ->
                    actual =
                        activity.resources.configuration.uiMode and Configuration.UI_MODE_NIGHT_MASK
                }
                actual == expectedNightFlag
            }
        assertNotNull("系统夜览模式没有切到 $mode（应用侧 uiMode=$actual）", applied)
    }

    /** 当前设置快照（主线程同步读）。 */
    private fun readSettings(): terminal.emulator.settings.SettingsRepository.SettingsState {
        var settings: terminal.emulator.settings.SettingsRepository.SettingsState? = null
        composeTestRule.activityRule.scenario.onActivity { activity: MainActivity ->
            settings = activity.terminalViewModel.settings.value
        }
        return checkNotNull(settings)
    }

    /** 当前系统夜览模式（`yes`/`no`/`auto`）。 */
    private fun readNightMode(): String {
        val output = UxTestUtils.runShellCommand("cmd uimode night").trim()
        assertTrue("读不到系统夜览模式（输出：$output）", output.startsWith("Night mode:"))
        return output.substringAfterLast(':').trim()
    }
}
