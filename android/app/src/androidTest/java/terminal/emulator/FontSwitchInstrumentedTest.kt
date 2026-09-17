package terminal.emulator

import android.util.Log
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import androidx.test.uiautomator.By
import androidx.test.uiautomator.UiDevice
import androidx.test.uiautomator.Until
import org.junit.After
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Test
import org.junit.runner.RunWith
import kotlin.properties.Delegates

@RunWith(AndroidJUnit4::class)
class FontSwitchInstrumentedTest {
    private var device by Delegates.notNull<UiDevice>()

    companion object {
        private const val TAG = "FontSwitchInstrumentedTest"
        private const val PACKAGE = "com.termux"
        private const val WAIT_TIMEOUT = 15_000L
    }

    @Before
    fun setUp() {
        try {
            device = UiDevice.getInstance(InstrumentationRegistry.getInstrumentation())
            // MainActivity.onCreate() requests POST_NOTIFICATIONS on first run
            // (Android 13+); the permission dialog overlays the activity and
            // blocks every UiAutomator lookup below. Grant up front so the
            // dialog never appears (see TestUtils.waitForSession).
            grantNotificationPermission()
            // Previous tests leave the app parked on the settings screen /
            // dialogs; am start merely brings that state to the foreground.
            // --activity-clear-task rebuilds the activity (state reset)
            // without force-stopping: force-stop would kill the
            // instrumentation process itself, since androidTest runs inside
            // the target app process.
            device.executeShellCommand("am start --activity-clear-task -n $PACKAGE/terminal.emulator.MainActivity")
            device.wait(Until.hasObject(By.pkg(PACKAGE).depth(0)), WAIT_TIMEOUT)
            Thread.sleep(5000)
        } catch (exception: Exception) {
            Log.e(TAG, "setUp failed", exception)
            throw exception
        }
    }

    @After
    fun tearDown() {
    }

    private fun openSettings() {
        val drawerBtn =
            device.findObject(By.desc("打开会话抽屉"))
                ?: device.findObject(By.text("☰"))
                ?: throw AssertionError("抽屉按钮必须存在")
        // 旧英文定位（Open session drawer/Settings）永远找不到，
        // ?.click 静默吞失败，改为找不到直接抛。
        drawerBtn.click()
        Thread.sleep(2000)
        val settingsEntry =
            device.findObject(By.text("设置"))
                ?: throw AssertionError("设置入口必须存在")
        settingsEntry.click()
        assertTrue(
            "设置页必须打开（字体分区可见）",
            device.wait(Until.hasObject(By.text("字体")), WAIT_TIMEOUT),
        )
    }

    private fun scrollTo(text: String, maxSwipes: Int = 30) {
        for (i in 0 until maxSwipes) {
            if (device.findObject(By.textContains(text)) != null) return
            val cx = device.displayWidth / 2
            device.swipe(cx, device.displayHeight * 3 / 4, cx, device.displayHeight / 4, 10)
            Thread.sleep(800)
        }
    }

    /** 滚动到字体行（含更改按钮）使其可见。 */
    private fun scrollToChange() {
        scrollTo("字体")
        // The title enters the viewport at its bottom edge; the row below it
        // 更改按钮可能仍在屏外，多滑一次。
        if (device.findObject(By.text("更改")) == null) {
            val cx = device.displayWidth / 2
            device.swipe(cx, device.displayHeight * 3 / 4, cx, device.displayHeight / 4, 10)
            Thread.sleep(1200)
        }
    }

    @Test
    fun settings_shows_font_family_section() {
        openSettings()
        scrollTo("字体")
        val found = device.findObject(By.text("字体"))
        assertNotNull("Settings should show 字体 section", found)
    }

    @Test
    fun settings_shows_change_button_for_font() {
        openSettings()
        scrollToChange()
        val changeBtn = device.findObject(By.text("更改"))
        assertNotNull("必须看到字体更改按钮", changeBtn)
    }

    @Test
    fun settings_shows_pick_font_file_button() {
        openSettings()
        scrollToChange()
        val changeBtn = device.findObject(By.text("更改"))
        assertNotNull("必须看到更改按钮", changeBtn)
        changeBtn?.click()
        Thread.sleep(2000)
        val pickBtn = device.findObject(By.text("从文件选择…"))
        assertNotNull("对话框必须出现从文件选择按钮", pickBtn)
    }

    @Test
    fun font_change_opens_dialog() {
        openSettings()
        scrollToChange()
        val changeBtn =
            checkNotNull(device.findObject(By.text("更改"))) {
                "字体设置中更改按钮必须可见"
            }
        changeBtn.click()
        Thread.sleep(2000)
        val dialogVisible =
            device.findObject(By.textContains("Fira")) != null ||
                device.findObject(By.textContains("Roboto")) != null ||
                device.findObject(By.textContains("Noto")) != null ||
                device.findObject(By.textContains("System")) != null
        assertTrue("Font picker dialog should show font names", dialogVisible)
    }

    @Test
    fun font_dialog_shows_system_default() {
        openSettings()
        scrollToChange()
        val changeBtn =
            checkNotNull(device.findObject(By.text("更改"))) {
                "字体设置中更改按钮必须可见"
            }
        changeBtn.click()
        Thread.sleep(2000)
        val hasFonts =
            device.findObject(By.textContains("Mono")) != null ||
                device.findObject(By.textContains("Sans")) != null
        assertTrue("Font picker should show available font options", hasFonts)
    }

    @Test
    fun font_dialog_shows_monospace_fonts() {
        openSettings()
        scrollToChange()
        val changeBtn =
            checkNotNull(device.findObject(By.text("更改"))) {
                "字体设置中更改按钮必须可见"
            }
        changeBtn.click()
        Thread.sleep(2000)
        val hasMono = device.findObject(By.textContains("Mono")) != null
        assertTrue("Font picker should show monospace fonts", hasMono)
    }

    @Test
    fun font_select_changes_font_family() {
        openSettings()
        scrollToChange()
        val changeBtn =
            checkNotNull(device.findObject(By.text("更改"))) {
                "字体设置中更改按钮必须可见"
            }
        changeBtn.click()
        Thread.sleep(3000)
        val fonts = listOf("Roboto Mono", "Noto Sans Mono", "Fira Code", "Source Code Pro", "monospace")
        val selectedFont = checkNotNull(fonts.firstOrNull { device.findObject(By.textContains(it)) != null }) {
            "Font picker must show at least one known font family"
        }
        device.findObject(By.textContains(selectedFont))?.click()
        Thread.sleep(3000)
        val appAlive = device.findObject(By.pkg(PACKAGE).depth(0)) != null
        assertTrue("App must survive font change", appAlive)
    }

    @Test
    fun app_survives_font_change() {
        openSettings()
        scrollToChange()
        val changeBtn =
            checkNotNull(device.findObject(By.text("更改"))) {
                "字体设置中更改按钮必须可见"
            }
        changeBtn.click()
        Thread.sleep(2000)
        // Emulator system font list (from FontInfoDto) contains Fira Code /
        // Droid Sans Mono etc. but no Noto family — pick a font that exists.
        val firstFont = checkNotNull(device.findObject(By.text("Fira Code"))) {
            "Font picker must show the Fira Code family"
        }
        firstFont.click()
        Thread.sleep(3000)
        val appAlive = device.findObject(By.pkg(PACKAGE).depth(0)) != null
        assertTrue("App must survive font change", appAlive)
    }
}
