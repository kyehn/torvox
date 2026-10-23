package terminal.emulator

import android.content.res.Configuration
import android.graphics.Bitmap
import androidx.compose.ui.graphics.toArgb
import androidx.compose.ui.test.junit4.v2.createAndroidComposeRule
import androidx.test.rule.GrantPermissionRule
import org.junit.After
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Rule
import org.junit.Test
import terminal.emulator.ui.theme.ThemeMode
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
 * 终端主题模式与软件主题模式都设为「跟随系统」，日/夜主题取默认浅色与默认深色
 * （两者底色相差极大，颜色扫描不会混淆）。系统夜览模式由 shell 切换：仪器化进程
 * 没有改系统设置的权限。
 */
class ThemeFollowSystemInstrumentedTest : TerminalLogcatTest() {
    @get:Rule
    val notificationPermission = GrantPermissionRule.grant(android.Manifest.permission.POST_NOTIFICATIONS)

    @get:Rule
    val composeTestRule = createAndroidComposeRule<MainActivity>()

    private companion object {
        /** 底色命中像素的下限：终端内容区整片都是底色，命中上千即说明整屏已换色。 */
        const val BACKGROUND_PIXEL_FLOOR = 2_000

        /** 等待主题生效的上限（毫秒）：配置变更 → 设置重解析 → 原生换主题 → 重绘。 */
        const val THEME_SWITCH_TIMEOUT_MS = 30_000L

        /** 等待设置落盘的上限（毫秒）。 */
        const val SETTINGS_TIMEOUT_MS = 15_000L
    }

    private var originalNightMode = "no"
    private var originalSettings: terminal.emulator.settings.SettingsRepository.SettingsState? = null

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
            viewModel.setThemeMode(ThemeMode.FOLLOW_SYSTEM)
            viewModel.setAppThemeMode(ThemeMode.FOLLOW_SYSTEM)
            viewModel.setDayThemeName(ThemeRepository.defaultLightName())
            viewModel.setNightThemeName(ThemeRepository.defaultDarkName())
        }
        val applied =
            UxTestUtils.pollUntilTrue(timeoutMs = SETTINGS_TIMEOUT_MS, intervalMs = 100) {
                val stored = readSettings()
                stored.themeMode == ThemeMode.FOLLOW_SYSTEM &&
                    stored.appThemeMode == ThemeMode.FOLLOW_SYSTEM &&
                    stored.dayThemeName == ThemeRepository.defaultLightName() &&
                    stored.nightThemeName == ThemeRepository.defaultDarkName()
            }
        assertNotNull("主题设置没有落盘", applied)
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
        val nightBackground = ThemeRepository.byName(ThemeRepository.defaultDarkName()).background.toArgb()

        val dayShot = awaitBackground(dayBackground, "day")
        assertNotNull("日间主题底色没有出现在终端 Surface 上", dayShot)

        setNightMode("yes", Configuration.UI_MODE_NIGHT_YES)
        val nightShot = awaitBackground(nightBackground, "night")
        assertNotNull("系统切到夜间后，终端底色仍不是夜间主题底色", nightShot)

        val dayRemainder = UxTestUtils.matchColor(checkNotNull(nightShot), dayBackground).count
        UxTestUtils.metric("theme_day_pixels_after_night", dayRemainder.toLong())
        assertTrue("夜间画面里仍残留 $dayRemainder 个日间底色像素", dayRemainder < BACKGROUND_PIXEL_FLOOR)

        // 再切回日间：单程切换通过不等于往返都通过（原生侧可能只认第一次换主题）。
        setNightMode("no", Configuration.UI_MODE_NIGHT_NO)
        val backShot = awaitBackground(dayBackground, "day_back")
        assertNotNull("系统切回日间后，终端底色仍不是日间主题底色", backShot)
    }

    /**
     * 等待终端 Surface 呈现 [color] 的底色，返回那一帧；超时返回 null。
     *
     * 读回很慢（软件渲染设备上一次整屏合成几百毫秒），故按固定节奏取，
     * 不用它做高频轮询——否则量具本身就把渲染循环按住了。
     *
     * [metricLabel] 是计量的名字段（ASCII）：logcat 里的计量名要能直接 grep。
     */
    private fun awaitBackground(color: Int, metricLabel: String): Bitmap? {
        val surface = UxTestUtils.terminalSurface(composeTestRule.activity) ?: return null
        var shot: Bitmap? = null
        UxTestUtils.pollUntilTrue(timeoutMs = THEME_SWITCH_TIMEOUT_MS, intervalMs = 400) {
            val captured = UxTestUtils.captureSurface(surface)
            if (captured != null && UxTestUtils.matchColor(captured, color).count >= BACKGROUND_PIXEL_FLOOR) {
                shot = captured
                true
            } else {
                false
            }
        }
        UxTestUtils.metric(
            "theme_${metricLabel}_pixels",
            (
                shot?.let {
                    UxTestUtils.matchColor(it, color).count
                } ?: 0
                ).toLong(),
        )
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
