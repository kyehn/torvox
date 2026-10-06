package terminal.emulator.ui

import androidx.test.ext.junit.rules.ActivityScenarioRule
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.filters.LargeTest
import androidx.test.platform.app.InstrumentationRegistry
import androidx.test.uiautomator.By
import androidx.test.uiautomator.UiDevice
import androidx.test.uiautomator.Until
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import terminal.emulator.MainActivity
import terminal.emulator.TerminalLogcatTest

/**
 * UIAutomator instrumentation tests.
 *
 * These exercise the real Android framework via [UiDevice]/[androidx.test.uiautomator.UiObject]
 * interactions (NOT injected `adb input` taps, which do not reach Compose `pointerInput` on the
 * phone emulator — see AGENTS.md pitfall #15). Run them on a tablet emulator or a real device,
 * where the system soft keyboard is available for genuine key input.
 */
@RunWith(AndroidJUnit4::class)
@LargeTest
class UiAutomatorTest : TerminalLogcatTest() {
    @get:Rule val activityRule = ActivityScenarioRule(MainActivity::class.java)

    private lateinit var device: UiDevice

    @Before
    fun setUp() {
        device = UiDevice.getInstance(InstrumentationRegistry.getInstrumentation())
        // MainActivity.onCreate() requests POST_NOTIFICATIONS on Android 13+;
        // the system dialog would cover the UI and break UiAutomator lookups.
        val instrumentation = InstrumentationRegistry.getInstrumentation()
        // Granting the permission dismisses the notification dialog. The
        // ActivityScenarioRule launches a fresh MainActivity per test, so
        // there is no cross-test UI state to reset — and skipping the
        // clear-task relaunch keeps the accessibility tree healthy (a
        // relaunched activity under instrumentation can expose a partial
        // tree with no Compose resource-ids).
        instrumentation.uiAutomation.grantRuntimePermission(
            instrumentation.targetContext.packageName,
            android.Manifest.permission.POST_NOTIFICATIONS,
        )
        assertTrue(
            "Terminal app should reach the foreground",
            device.wait(Until.hasObject(By.pkg("com.termux").depth(0)), 15000),
        )
        // Let the terminal surface render its first frame (software GPU is
        // slow); the render-surface node only appears after that.
        Thread.sleep(5000)
    }

    /** The terminal render surface should be present once the app is launched. */
    @Test
    fun terminalSurfaceAppearsAfterLaunch() {
        // TerminalSurface (a SurfaceView) is hosted inside the TerminalContent
        // AndroidView container, which only exists once the terminal has
        // rendered. Resource id is the bare tag (no package prefix).
        val terminalSurface = device.wait(Until.findObject(By.res("TerminalContent")), 30000)
        assertNotNull("Terminal render surface should be visible", terminalSurface)
    }

    /** The Compose TerminalScreen node should be present in the view hierarchy after launch. */
    @Test
    fun terminalScreenNodeAppearsAfterLaunch() {
        // Compose testTagsAsResourceId exposes the tag as the bare resource id
        // ("TerminalScreen", no package prefix), so By.res must not use the
        // "com.termux:id/..." form.
        val terminalScreen = device.wait(Until.findObject(By.res("TerminalScreen")), 30000)
        assertNotNull("TerminalScreen composable should be present", terminalScreen)
    }

    /**
     * Typing via the real system soft keyboard (driven by UiObject key clicks) should make the app
     * react: focusing the search field and pressing a letter produces a non-empty result count,
     * proving the input reached the application.
     */
    @Test
    fun typingViaSystemKeyboardReacts() {
        val drawerButton = device.findObject(By.desc("打开会话抽屉"))
        assertNotNull("Session drawer button should exist", drawerButton)
        requireNotNull(drawerButton).click()
        assertTrue(
            "Search button should appear after opening the drawer",
            device.wait(Until.hasObject(By.res("SearchButton")), 15000),
        )

        requireNotNull(device.findObject(By.res("SearchButton"))).click()
        val searchField =
            device.wait(
                Until.findObject(By.res("SearchTextField")),
                5000,
            )
        requireNotNull(searchField) { "Search text field should appear" }.click()
        device.waitForIdle(1000)

        // 系统键盘经 IME 异步弹出，单次直查会把「还没来得及」判成「按键缺席」，与计数节点同口径轮询。
        val keyE =
            device.wait(Until.findObject(By.text("e")), 15000)
                ?: device.wait(Until.findObject(By.desc("e")), 5000)
        requireNotNull(keyE) { "System keyboard key 'e' should be visible" }.click()
        device.waitForIdle(1000)

        // 计数节点由「点击按键 → IME 提交 → query 状态更新 → 重组」这条链产生
        // （TextSearchBar 只在 query 非空时挂它），而 waitForIdle 只等设备空闲、
        // 不保证应用侧重组已落地，故这里必须等而不是直读：本文件其余 5 处查找
        // 全用 wait，唯独此处按用户动作之后的出现直读——在 2 核 swiftshader
        // 模拟器上会把「重组还没来得及」判成「计数没出现」。
        val resultCount =
            device.wait(Until.findObject(By.res("SearchResultCount")), 15000)
        assertNotNull("Search result count should become visible after typing", resultCount)
        assertTrue(
            "Search result count text should be non-empty after typing",
            requireNotNull(resultCount).text.isNotEmpty(),
        )
    }
}
