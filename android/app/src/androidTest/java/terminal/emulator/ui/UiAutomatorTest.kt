package terminal.emulator.ui

import android.graphics.Rect
import android.os.SystemClock
import androidx.test.ext.junit.rules.ActivityScenarioRule
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.filters.LargeTest
import androidx.test.platform.app.InstrumentationRegistry
import androidx.test.uiautomator.By
import androidx.test.uiautomator.BySelector
import androidx.test.uiautomator.UiDevice
import androidx.test.uiautomator.UiObject2
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
        openSearchField().click()
        device.waitForIdle(1000)

        // 系统键盘经 IME 异步弹出，单次直查会把「还没来得及」判成「按键缺席」，与计数节点同口径轮询。
        val letterEKey =
            device.wait(Until.findObject(By.text("e")), 15000)
                ?: device.wait(Until.findObject(By.desc("e")), 5000)
        requireNotNull(letterEKey) { "System keyboard key 'e' should be visible" }.click()
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

    /**
     * 打开抽屉里的文本搜索，返回搜索输入框节点。
     *
     * 抽屉滑入有动画，而无障碍树里的节点边界落后于实际绘制位置：动画期间按
     * 它给出的坐标点击会落到遮罩上（抽屉被点关，搜索框不出现）。实测全量套件
     * 里偶发——抽屉按钮点击后 408ms 就查到了 SearchButton（动画未完），随后的
     * 点击落空，单跑该用例则必绿。
     *
     * 故：等边界停稳再点，点完校验结果，没出现就重开抽屉再点一次。真实用户按
     * 的是眼睛看到的当前位置，不会落到遮罩上——这是量具的时序问题，不是应用缺陷。
     */
    private fun openSearchField(): UiObject2 {
        var summary = ""
        repeat(SEARCH_OPEN_ATTEMPTS) { attempt ->
            // 抽屉已开（滑入动画中节点也在树里）就别再点抽屉按钮：那次点击会落到
            // 遮罩上，把抽屉关掉。
            if (!device.hasObject(By.res(SEARCH_BUTTON))) {
                val drawerButton = device.findObject(By.desc(CD_OPEN_DRAWER))
                assertNotNull("Session drawer button should exist", drawerButton)
                requireNotNull(drawerButton).click()
            }
            val searchButton = awaitSettledButton(By.res(SEARCH_BUTTON))
            assertNotNull("Search button should appear after opening the drawer", searchButton)
            requireNotNull(searchButton).click()
            val field = device.wait(Until.findObject(By.res(SEARCH_FIELD)), SEARCH_FIELD_TIMEOUT_MS)
            if (field != null) return field
            summary = hierarchySummary()
            android.util.Log.i(TAG, "search open attempt ${attempt + 1} missed: $summary")
        }
        throw AssertionError("Search text field should appear（重试 $SEARCH_OPEN_ATTEMPTS 次仍无搜索框）\n$summary")
    }

    /**
     * 等节点滑入到位（可见边界停稳）再返回它；超时返回最后一次找到的节点。
     *
     * 连续两次读到的可见边界相同即视为停稳；边界不在屏幕内的节点（过期节点或
     * 尚未滑入）不算数——那种节点点下去必然落空。
     */
    private fun awaitSettledButton(selector: BySelector): UiObject2? {
        val screen = Rect(0, 0, device.displayWidth, device.displayHeight)
        var lastBounds: Rect? = null
        var stableSamples = 0
        val deadline = SystemClock.uptimeMillis() + BUTTON_SETTLE_TIMEOUT_MS
        while (SystemClock.uptimeMillis() < deadline) {
            val button = device.findObject(selector)
            val bounds = button?.visibleBounds
            if (button != null && bounds != null && screen.contains(bounds)) {
                if (bounds == lastBounds) {
                    stableSamples++
                    if (stableSamples >= 2) return button
                } else {
                    stableSamples = 0
                    lastBounds = bounds
                }
            } else {
                stableSamples = 0
                lastBounds = null
            }
            SystemClock.sleep(BUTTON_SETTLE_INTERVAL_MS)
        }
        return device.findObject(selector)
    }

    /** 无障碍树摘要：带 resource-id / content-desc 的节点及其边界。 */
    private fun hierarchySummary(): String {
        // UiAutomator 2.4 只提供「导出到流/文件」的重载，没有返回字符串的那一个。
        val stream = java.io.ByteArrayOutputStream()
        try {
            device.dumpWindowHierarchy(stream)
        } catch (error: java.io.IOException) {
            android.util.Log.w(TAG, "无障碍树导出失败", error)
            return "（无障碍树导出失败）"
        }
        return stream.toString(Charsets.UTF_8)
            .lineSequence()
            .filter { it.contains("resource-id=") || it.contains("content-desc=") }
            .take(40)
            .joinToString("\n")
            .take(4000)
    }

    private companion object {
        const val TAG = "UiAutomatorTest"

        /** 抽屉里的搜索按钮（resource-id）。 */
        const val SEARCH_BUTTON = "SearchButton"

        /** 搜索输入框（resource-id）。 */
        const val SEARCH_FIELD = "SearchTextField"

        /** 打开会话抽屉按钮的 content description。 */
        const val CD_OPEN_DRAWER = "打开会话抽屉"

        /** 打开搜索框的最大尝试次数。 */
        const val SEARCH_OPEN_ATTEMPTS = 3

        /** 每次尝试后等待搜索框出现的上限（毫秒）。 */
        const val SEARCH_FIELD_TIMEOUT_MS = 5_000L

        /** 等节点边界停稳的上限（毫秒）。 */
        const val BUTTON_SETTLE_TIMEOUT_MS = 10_000L

        /** 边界取样的间隔（毫秒）：要大于一帧，才能看出边界在动。 */
        const val BUTTON_SETTLE_INTERVAL_MS = 150L
    }
}
