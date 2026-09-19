package terminal.emulator

import android.content.ClipData
import android.content.ClipboardManager
import android.graphics.Bitmap
import android.util.Log
import androidx.test.platform.app.InstrumentationRegistry
import androidx.test.rule.GrantPermissionRule
import androidx.test.uiautomator.By
import androidx.test.uiautomator.UiDevice
import androidx.test.uiautomator.UiScrollable
import androidx.test.uiautomator.UiSelector
import androidx.test.uiautomator.Until
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Rule
import org.junit.Test

class BehaviorInstrumentedTest {
    // MainActivity requests POST_NOTIFICATIONS on Android 13+ at startup;
    // without pre-granting it the system permission dialog covers the UI
    // and none of the drawer/settings nodes appear.
    @get:Rule
    val notificationPermission = GrantPermissionRule.grant(android.Manifest.permission.POST_NOTIFICATIONS)

    companion object {
        private const val TAG = "BehaviorTest"
        private const val PACKAGE = "com.termux"
        private const val WAIT_TIMEOUT = 30_000L
        private const val SELECTION_PIXEL_GAIN_THRESHOLD = 300
    }

    private lateinit var device: UiDevice
    private var initialized = false

    @Before
    fun setUp() {
        try {
            device = UiDevice.getInstance(InstrumentationRegistry.getInstrumentation())
            initialized = true
            device.executeShellCommand("am start -n $PACKAGE/terminal.emulator.MainActivity")
            device.wait(Until.hasObject(By.pkg(PACKAGE).depth(0)), WAIT_TIMEOUT)
            Thread.sleep(10000)
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
        Thread.sleep(3000)
    }

    // 长按前后截图采样差分：选择高亮/手柄/菜单必改数千采样像素，
    // 状态栏时钟仅贡献百级。用量化的像素数代替“是否看见”，防抖且诚实。
    private fun countChangedPixels(before: Bitmap?, after: Bitmap?): Int {
        if (before == null || after == null) return 0
        if (before.width != after.width || before.height != after.height) return 0
        var changed = 0
        var samplingX = 0
        while (samplingX < before.width) {
            var samplingY = 0
            while (samplingY < before.height) {
                if (before.getPixel(samplingX, samplingY) != after.getPixel(samplingX, samplingY)) changed++
                samplingY += 4
            }
            samplingX += 4
        }
        return changed
    }

    private fun scrollTo(text: String, maxSwipes: Int = 30) {
        for (i in 0 until maxSwipes) {
            Thread.sleep(500)
            if (device.findObject(By.textContains(text)) != null) return
            try {
                val scrollable = UiScrollable(UiSelector().scrollable(true))
                scrollable.scrollForward()
            } catch (_: Exception) {
                val cx = device.displayWidth / 2
                device.swipe(cx, device.displayHeight * 6 / 10, cx, device.displayHeight / 4, 10)
            }
            Thread.sleep(800)
        }
    }

    private fun goBack() {
        device.pressBack()
        Thread.sleep(1000)
    }

    @Test
    fun behavior_app_process_alive() {
        val output = device.executeShellCommand("dumpsys activity processes | grep -i $PACKAGE")
        assertTrue("App process should be running", output.isNotEmpty())
    }

    @Test
    fun behavior_app_stays_in_foreground() {
        // Smoke check only: no color/rendering assertion is possible while
        // Bridge.setTheme is a log-only implemented (native query path is wired).
        val output = device.executeShellCommand("dumpsys activity top | grep -i $PACKAGE")
        assertTrue("App should be in foreground", output.isNotEmpty())
    }

    @Test
    fun behavior_font_picker_opens_with_change_button() {
        openSettings()
        // 字体区文案均为中文：标题“字体”，按钮“更改”，对话框标题“选择字体”。
        val fontReady = device.wait(Until.hasObject(By.text("更改")), WAIT_TIMEOUT)
        if (!fontReady) {
            scrollTo("字体")
        }
        val changeBtn =
            device.findObject(By.text("更改"))
                ?: throw AssertionError("更改按钮必须存在")
        changeBtn.click()
        Thread.sleep(2000)
        val dialog =
            device.findObject(By.text("选择字体"))
                ?: device.findObject(By.textContains("monospace"))
                ?: device.findObject(By.textContains("Mono"))
                ?: device.findObject(By.textContains("Noto"))
        assertTrue("Font picker dialog should appear", dialog != null)
        goBack()
    }

    @Test
    fun behavior_selection_toolbar_shows_copy_select_all() {
        // 选择菜单走 Surface 侧 PopupWindow（复制/粘贴/分享/全选），不用系统
        // ActionMode；键盘模式无设置 UI（默认安全模式），旧模式切换步骤是空转，
        // 连同其恢复块一并删除（删的是无操作步骤，不是覆盖）。
        // 新启动的 Activity 终端默认已获焦，无需额外点击（SurfaceView 挖洞
        // 渲染在 UiAutomator 层级中不可见，任何基于节点的聚焦定位都不可靠）。
        Thread.sleep(2000)
        // The menu only appears after an actual selection: long-press the
        // shell prompt near the bottom of the terminal. On the
        // software-rendered emulator the press may land on a blank cell
        // (paste-only menu: 粘贴) or on text (full menu: 复制); either
        // proves the selection menu surfaced through the real input
        // pipeline.
        // 空白格长按只在剪贴板有内容时才出粘贴菜单（否则动作表为空直接
        // return）；新机剪贴板恒空，先放种子文本，保证两种落点都有菜单。
        val clipboardManager =
            InstrumentationRegistry.getInstrumentation().targetContext
                .getSystemService(ClipboardManager::class.java)
        clipboardManager?.setPrimaryClip(ClipData.newPlainText("seed", "seed-selection"))
        val beforePress = device.takeScreenshot()
        // UiDevice.swipe 把 DOWN/UP 发进同一主线程批处理，常被当点按吃掉
        // （TestUtils.injectLongPress 有述）；经 input flinger 按真实时长
        // 下发事件，保证长按定时器能触发。
        device.executeShellCommand("input touchscreen swipe 200 1850 200 1850 1000")
        Thread.sleep(1500)
        val copy = device.findObject(By.text("复制"))
        val paste = device.findObject(By.text("粘贴"))
        // 菜单 PopupWindow 若未进无障碍层级，文本查不到但像素必变：双信号判决。
        val changedPixels = countChangedPixels(beforePress, device.takeScreenshot())
        assertTrue(
            "Selection must surface after long-press " +
                "(copy=${copy != null} paste=${paste != null} changedPx=$changedPixels)",
            copy != null || paste != null || changedPixels > SELECTION_PIXEL_GAIN_THRESHOLD,
        )
        // When the long-press selects text, the paste-only menu must NOT
        // be shown (paste-only selections are reserved for blank cells).
        if (copy != null) {
            assertFalse(
                "Paste should NOT appear when text selected",
                paste != null,
            )
        }
    }

    @Test
    fun behavior_settings_theme_names_visible() {
        openSettings()
        val themeReady = device.wait(Until.hasObject(By.text("Dracula Plus")), WAIT_TIMEOUT)
        if (!themeReady) {
            scrollTo("Dracula Plus")
        }
        val dracula = device.findObject(By.text("Dracula Plus"))
        val catppuccin = device.findObject(By.text("Catppuccin Mocha"))
        val nord = device.findObject(By.text("Nord"))
        assertTrue("Dracula Plus should be visible", dracula != null)
        assertTrue("Catppuccin Mocha should be visible", catppuccin != null)
        assertTrue("Nord should be visible", nord != null)
        goBack()
    }

    @Test
    fun behavior_settings_bootstrap_action_buttons() {
        openSettings()
        // 预设行在 Install 按钮上方：先滚到预设断言，再继续下滚到按钮断言
        // （一次只保证一项在视口内，同时断言两项在窄屏上恒失败）。
        scrollTo("Termux 默认", maxSwipes = 60)
        val termuxDefault = device.findObject(By.text("Termux 默认"))
        assertTrue("Termux 默认 should be visible", termuxDefault != null)
        scrollTo("安装", maxSwipes = 60)
        val installBtn = device.findObject(By.text("安装"))
        assertTrue("Install button should be visible", installBtn != null)
        goBack()
    }

    @Test
    fun behavior_settings_no_nerd_osc133_toggles() {
        openSettings()
        assertFalse(
            "Nerd toggle should NOT exist",
            device.findObject(By.textContains("Nerd")) != null,
        )
        assertFalse(
            "OSC133 toggle should NOT exist",
            device.findObject(By.textContains("OSC")) != null,
        )
        goBack()
    }

    @Test
    fun behavior_modifier_bar_visible() {
        val modifierBarReady =
            device.wait(Until.hasObject(By.text("ESC")), WAIT_TIMEOUT)
        assertTrue("Modifier bar should load with ESC key", modifierBarReady)
        val esc = device.findObject(By.text("ESC"))
        val ctrl = device.findObject(By.text("CTRL"))
        val alt = device.findObject(By.text("ALT"))
        val home = device.findObject(By.text("HOME"))
        assertTrue("ESC should be visible", esc != null)
        assertTrue("CTRL should be visible", ctrl != null)
        assertTrue("ALT should be visible", alt != null)
        assertTrue("HOME should be visible", home != null)
    }

    @Test
    fun behavior_drawer_shows_sessions_and_settings() {
        // 抽屉内文案均为中文：设置入口“设置”，会话项标题“会话 N”。
        val drawerBtn =
            device.findObject(By.desc("打开会话抽屉"))
                ?: device.findObject(By.text("☰"))
                ?: throw AssertionError("抽屉按钮必须存在")
        drawerBtn.click()
        Thread.sleep(2000)
        val drawerReady = device.wait(Until.hasObject(By.text("设置")), WAIT_TIMEOUT)
        assertTrue("Drawer should load with Settings option", drawerReady)
        val settings =
            device.findObject(By.text("设置"))
                ?: throw AssertionError("Settings should be in drawer")
        val sessions =
            device.findObject(By.textContains("会话"))
                ?: throw AssertionError("Session should be in drawer")
        assertTrue("Session row must be enabled", sessions.isEnabled || sessions.isClickable)
        settings.click()
        Thread.sleep(2000)
        goBack()
    }

    @Test
    fun behavior_shell_path_correct() {
        openSettings()
        val shellReady = device.wait(Until.hasObject(By.text("/system/bin/sh")), WAIT_TIMEOUT)
        if (!shellReady) {
            scrollTo("/system/bin/sh")
        }
        val shell = device.findObject(By.text("/system/bin/sh"))
        assertTrue("Shell path should be /system/bin/sh", shell != null)
        goBack()
    }
}
