package terminal.emulator

import android.app.Activity
import android.graphics.Bitmap
import android.os.Looper
import android.os.SystemClock
import android.view.MotionEvent
import android.view.View
import android.view.ViewGroup
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.junit4.AndroidComposeTestRule
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.performClick
import androidx.test.ext.junit.rules.ActivityScenarioRule
import androidx.test.platform.app.InstrumentationRegistry
import androidx.test.uiautomator.By
import androidx.test.uiautomator.UiDevice
import androidx.test.uiautomator.Until
import org.junit.Assert.fail
import terminal.emulator.bridge.Bridge
import terminal.emulator.installer.TerminalPrefix
import terminal.emulator.util.runCatchingCancellable

private const val DRAWER_BUTTON_DESCRIPTION = "打开会话抽屉"
private const val DRAWER_BUTTON_TIMEOUT_MS = 5_000L
private const val BRIDGE_READY_TIMEOUT_MS = 30_000L
private const val BRIDGE_POLL_INTERVAL_MS = 200L
private const val DRAWER_VISIBLE_TIMEOUT_MS = 10_000L
private const val NOT_RESPONDING_DIALOG_WAIT_ID = "android:id/aerr_wait"
private const val NOT_RESPONDING_DIALOG_CLOSE_ID = "android:id/aerr_close"
private const val DIALOG_DISMISS_SETTLE_MS = 500L

// ── Data model ──────────────────────────────────────

/**
 * Display probe for Compose assertions: returns false when the assertion
 * fails instead of throwing. Compose assertions throw AssertionError (an
 * Error, not an Exception), so the cancellable Result wrapper cannot be
 * used for probes — it would propagate and fail the calling test at the
 * probe site instead of letting fallbacks run.
 */
internal inline fun probeAssertion(crossinline check: () -> Unit): Boolean = try {
    check()
    true
} catch (_: AssertionError) {
    false
}

fun AndroidComposeTestRule<*, *>.waitForSession(timeoutMs: Long = 60_000) {
    // 「会话可用」包含「有 shell 可交互」：依赖真实 shell 的用例要往 PTY 里打标记并
    // 等回显，而 `:app:connectedAndroidTest` 开跑前会全新安装（应用数据清空，prefix
    // 一并消失）。prefix 只由 installer.* 的用例装回，字典序却排在后面——于是失败与否
    // 取决于类名字母序。实测本地全量套件里 PasteButtonInstrumentedTest 等 8 个用例因此
    // 报「标记未落格」，单跑即过。幂等，已装时只是一次文件存在性检查。
    TerminalPrefix.ensureInstalled()
    System.setProperty("test.minSurface", "true")
    // MainActivity.onCreate() requests POST_NOTIFICATIONS on first run
    // (Android 13+); the permission dialog overlays the activity and
    // blocks waitUntil(TerminalScreen). Grant up front — idempotent,
    // and a pending request dialog is dismissed once the permission is
    // granted underneath it.
    grantNotificationPermission()
    // Use the standard assertion approach (same as search steps) instead of
    // allNodes + fetchSemanticsNodes, which may fail in merged-tree scenarios
    waitForTerminalScreen(timeoutMs)
}

/**
 * 等待终端页面节点可断言。
 *
 * 像素/选择类用例的公共就绪门槛：Activity 已起但 Compose 尚未组合出节点时，
 * 任何 `onNodeWithTag(...).assert*` 都会抛断言错误，而那只是「还没好」。
 *
 * 就绪门槛同时包含「系统弹窗不在挡路」：终端节点在设置浮层与无响应对话框之下
 * 仍照常组合（`assertIsDisplayed` 照样通过），而后两者一旦出现，节点查找与像素
 * 采样量到的全是它们——判红原因与被测行为无关。放在这里而非各调用点，是因为
 * 两者都等同一个节点，而只有部分调用点记得关对话框（漏关者即本仓的
 * `diag.CursorPixelAcceptanceTest`）。
 */
fun AndroidComposeTestRule<*, *>.waitForTerminalScreen(timeoutMs: Long = 60_000) {
    dismissNotRespondingDialog()
    waitUntil(timeoutMillis = timeoutMs) {
        probeAssertion { onNodeWithTag("TerminalScreen").assertIsDisplayed() }
    }
}

/**
 * 像素类用例的就绪门槛：在 [waitForTerminalScreen] 之上再等原生 Surface 挂载。
 *
 * 终端节点在 Compose 组合完成时**就已存在**，那早于 SurfaceView 拿到有效
 * Surface（`surfaceCreated` 在 0 尺寸或无效 Surface 时会推迟到 `surfaceChanged`）。
 * 只等节点时，像素类用例会在 `Surface::get_current_texture_view` 报
 * `NotConfigured` 的窗口内采样，量到的全是零——判红原因与被测行为无关。
 * CI 实测：run 的日志里该 Validation Error 与像素用例判红同批出现，
 * 且资源越紧（CI 为 1536M/2 核）窗口越大。
 *
 * 只对**确实采样像素**的用例是必需的：不采样像素的用例（会话、抽屉、安装）
 * 不该被 surface 时序绑住，故不并入 [waitForTerminalScreen]。
 */
fun AndroidComposeTestRule<*, *>.waitForTerminalPixels(timeoutMs: Long = 60_000) {
    waitForTerminalScreen(timeoutMs)
    waitUntil(timeoutMillis = timeoutMs) { isTerminalSurfaceAttached() }
}

/** 终端 Surface 是否已 attach 且仍然有效（主线程同步读）。 */
fun AndroidComposeTestRule<*, *>.isTerminalSurfaceAttached(): Boolean {
    var attached = false
    runOnMainThread { attached = (activity as MainActivity).terminalViewModel.currentSurface?.isValid == true }
    return attached
}

/**
 * 关掉系统「应用无响应」对话框（点「等待」）。
 *
 * 该对话框是系统级模态窗口：它一旦出现，UiAutomator 只能看到它，`By.desc(...)` /
 * `By.text(...)` 全部查不到应用节点，于是与产品无关的用例成片报「抽屉按钮必须存在」。
 * 软件渲染（swiftshader）的模拟器被应用渲染压满时必然触发——本地全量套件实测如此：
 * 同批用例单跑全过、全量跑成片红。没有对话框时本函数为空操作。
 */
fun dismissNotRespondingDialog() {
    val device = UiDevice.getInstance(InstrumentationRegistry.getInstrumentation())
    val dialogButton =
        device.findObject(By.res(NOT_RESPONDING_DIALOG_WAIT_ID))
            ?: device.findObject(By.res(NOT_RESPONDING_DIALOG_CLOSE_ID))
            ?: return
    runCatchingCancellable { dialogButton.click() }
    Thread.sleep(DIALOG_DISMISS_SETTLE_MS)
}

fun grantNotificationPermission() {
    val instrumentation = androidx.test.platform.app.InstrumentationRegistry.getInstrumentation()
    instrumentation.uiAutomation.grantRuntimePermission(
        instrumentation.targetContext.packageName,
        android.Manifest.permission.POST_NOTIFICATIONS,
    )
}

/**
 * 在主线程同步执行 [block]，已身处主线程时直接执行。
 *
 * `Instrumentation.runOnMainSync` 禁止在主线程上调用（抛
 * "This method can not be called from the main application thread"），
 * 而用例体、`@After` 与 `runOnUiThread` 回调都可能已在主线程上——这些调用点
 * 必须直接跑，否则 `@After` 会在清场前抛出，把共享会话的污染整轮留给后继用例。
 * 其异常保持外抛：它只在 activity 缺失这类真实故障上出现，吞掉会把故障
 * 伪装成「桥为 null」或孵化超时。
 */
private fun runOnMainThread(block: () -> Unit) {
    if (Looper.myLooper() == Looper.getMainLooper()) {
        block()
    } else {
        InstrumentationRegistry.getInstrumentation().runOnMainSync(block)
    }
}

fun AndroidComposeTestRule<*, *>.getBridge(): Bridge? {
    var bridge: Bridge? = null
    // v2 createAndroidComposeRule 没有 activityRule 字段（v1 API）：
    // 用 activity 直接进主线程读桥。必须同步等待：runOnUiThread 仅在本线程
    // 即主线程时同步执行，测试线程调用时只投递就立即返回，读到的恒为 null；
    // 桥为 null（会话孵化中）则由调用方重试。
    runOnMainThread { bridge = (activity as MainActivity).runtime.bridge() }
    return bridge
}

/**
 * 等待运行时桥孵化完成并返回。会话创建后桥异步建立，`waitForSession` 只等界面
 * 节点，首次直读必为 null——未等即用会拿到「桥为 null」的假失败。
 */
fun AndroidComposeTestRule<*, *>.awaitBridge(timeoutMs: Long = BRIDGE_READY_TIMEOUT_MS): Bridge {
    UxTestUtils.pollUntilTrue(timeoutMs = timeoutMs, intervalMs = BRIDGE_POLL_INTERVAL_MS) {
        getBridge() != null
    }
    return checkNotNull(getBridge()) { "运行时桥必须就绪（${timeoutMs}ms 未孵化）" }
}

fun AndroidComposeTestRule<*, *>.openDrawer() {
    waitForIdle()
    val device = UiDevice.getInstance(InstrumentationRegistry.getInstrumentation())
    // 系统权限对话框会遮挡抽屉按钮：存在就点掉，不存在不是错误。
    if (device.hasObject(By.text("Allow")) || device.hasObject(By.text("ALLOW"))) {
        (device.findObject(By.text("Allow")) ?: device.findObject(By.text("ALLOW")))?.click()
    }
    if (!device.wait(Until.hasObject(By.desc(DRAWER_BUTTON_DESCRIPTION)), DRAWER_BUTTON_TIMEOUT_MS)) {
        fail("会话抽屉按钮未出现，无法继续")
    }
    device.findObject(By.desc(DRAWER_BUTTON_DESCRIPTION))?.click()
        ?: fail("会话抽屉按钮无法点击")
    waitForIdle()
    waitUntil(timeoutMillis = DRAWER_VISIBLE_TIMEOUT_MS) {
        probeAssertion {
            onNodeWithTag("SessionDrawer", useUnmergedTree = true).assertIsDisplayed()
        } || device.hasObject(By.text("Sessions")) || device.hasObject(By.res("SessionDrawer"))
    }
}

/** 当前活跃会话 id（主线程同步读）：会话增删断言的共享读口。 */
fun AndroidComposeTestRule<*, *>.activeSessionId(): Long {
    var id = -1L
    runOnMainThread { id = (activity as MainActivity).terminalViewModel.state.value.activeSessionId }
    return id
}

/** 会话总数（主线程同步读）：与 [activeSessionId] 同口径。 */
fun AndroidComposeTestRule<*, *>.sessionCount(): Int {
    var count = -1
    runOnMainThread { count = (activity as MainActivity).terminalViewModel.state.value.sessions.size }
    return count
}

/** 会话在抽屉列表中的位置（0 起）：按 id 定位，避免硬编码“会话 N”文本。 */
fun AndroidComposeTestRule<*, *>.sessionIndex(id: Long): Int {
    var index = -1
    runOnMainThread {
        index = (activity as MainActivity).terminalViewModel.state.value.sessions.indexOfFirst { it.id == id }
    }
    return index
}

/**
 * 关掉可能盖住终端的设置浮层，回到终端页面。设置未打开时为空操作。
 *
 * 设置是 `MainActivity` 内的 Compose 浮层：`TerminalScreen(isOverlayVisible=…)`
 * 始终保持组合、其上再盖一层 `SettingsScreen`。于是**只 `openSettings()` 不返回**
 * 的用例会留下两个后果，且都不报错：终端节点仍在语义树里（`assertIsDisplayed`
 * 照样通过），而截图量到的已是设置界面——其后每一个像素用例的断言都在测设置页
 * （实测光标反差与红/绿/蓝像素同时为 0）。这是「本地单跑全绿、整类连跑成片红」
 * 的又一例同类根因。
 */
fun AndroidComposeTestRule<*, *>.closeSettingsOverlay() {
    if (!probeAssertion { onNodeWithTag("SettingsScreen", useUnmergedTree = true).assertIsDisplayed() }) return
    onNodeWithTag("SettingsBackButton").performClick()
    waitUntil(timeoutMillis = 10_000) {
        probeAssertion { onNodeWithTag("TerminalScreen").assertIsDisplayed() }
    }
}

fun AndroidComposeTestRule<*, *>.openSettings() {
    // Grant notification permission before probing: the permission dialog overlays
    // the activity and blocks Settings navigation on first run (CI cold start on PlayStore image).
    runCatchingCancellable { grantNotificationPermission() }
    // Also dismiss the system permission dialog if it is still visible (PlayStore image shows
    // Allow/Deny).
    runCatchingCancellable {
        val d = UiDevice.getInstance(InstrumentationRegistry.getInstrumentation())
        if (d.hasObject(By.text("Allow"))) d.findObject(By.text("Allow"))?.click()
        if (d.hasObject(By.text("ALLOW"))) d.findObject(By.text("ALLOW"))?.click()
        if (d.hasObject(By.text("Deny"))) {
            // Do not click Deny; just grant via uiAutomation and wait
            Thread.sleep(300)
        }
    }
    // Fast-path: Settings may already be visible (shared activity across cucumber scenarios).
    val settingsAlreadyVisible =
        probeAssertion {
            onNodeWithTag("SettingsScreen", useUnmergedTree = true).assertIsDisplayed()
        }
    if (settingsAlreadyVisible) return
    val device = UiDevice.getInstance(InstrumentationRegistry.getInstrumentation())
    if (
        device.hasObject(By.text("Font Family")) ||
        device.hasObject(By.res("SettingsScreen")) ||
        device.hasObject(By.text("Appearance"))
    ) {
        return
    }
    // Try direct Compose click on SettingsButton even when drawer is closed — the drawer content is
    // composed off-screen (ModalNavigationDrawer) and the button is still in the semantics tree.
    val directClickOpened =
        runCatchingCancellable {
            onNodeWithTag("SettingsButton", useUnmergedTree = true).performClick()
            waitForIdle()
            waitForSettingsScreenProbe(8000)
        }
            .getOrDefault(false)
    if (directClickOpened) return
    // Try UiAutomator on the button's testTag resource id (testTagsAsResourceId = true) and
    // desc/text.
    val uiResClickOpened =
        runCatchingCancellable {
            val found =
                device.wait(Until.hasObject(By.res("SettingsButton")), 3000) ||
                    device.wait(Until.hasObject(By.desc("Settings")), 3000) ||
                    device.wait(Until.hasObject(By.text("Settings")), 3000)
            if (found) {
                val obj =
                    device.findObject(By.res("SettingsButton"))
                        ?: device.findObject(By.desc("Settings"))
                        ?: device.findObject(By.text("Settings"))
                obj?.click()
                waitForIdle()
                waitForSettingsScreenProbe(8000)
            } else {
                false
            }
        }
            .getOrDefault(false)
    if (uiResClickOpened) return
    // Fallback: open drawer explicitly then click Settings. Retry up to 3 times for CI flakiness.
    repeat(3) { attempt ->
        openDrawer()
        // After drawer is open, try Compose first (most reliable), then UiDevice res/desc/text.
        val composeAfterDrawer =
            runCatchingCancellable {
                onNodeWithTag("SettingsButton", useUnmergedTree = true).performClick()
                waitForIdle()
                waitForSettingsScreenProbe(8000)
            }
                .getOrDefault(false)
        if (composeAfterDrawer) return
        val selectors =
            listOf(
                By.res("SettingsButton"),
                By.desc("Settings"),
                By.text("Settings"),
                By.res("SearchButton"),
            )
        var selectorClicked = false
        for (selector in selectors) {
            // Skip SearchButton — we only use it to confirm drawer is open, not to click Settings
            if (selector == By.res("SearchButton")) {
                if (!device.hasObject(selector)) continue else break
            }
            selectorClicked =
                runCatchingCancellable {
                    if (device.wait(Until.hasObject(selector), 5000)) {
                        device.findObject(selector)?.click()
                        true
                    } else {
                        false
                    }
                }
                    .getOrDefault(false)
            if (selectorClicked) break
        }
        if (selectorClicked) {
            waitForIdle()
            if (waitForSettingsScreenProbe(8000)) return
            // Also try a second Compose click after UiDevice click (covers scrim race)
            runCatchingCancellable {
                onNodeWithTag("SettingsButton", useUnmergedTree = true).performClick()
                waitForIdle()
                if (waitForSettingsScreenProbe(5000)) return
            }
        }
        if (attempt < 2) Thread.sleep(700)
    }
    waitForSettingsScreen(timeoutMs = 60_000)
}

private fun AndroidComposeTestRule<*, *>.waitForSettingsScreenProbe(timeoutMs: Long): Boolean {
    val device = UiDevice.getInstance(InstrumentationRegistry.getInstrumentation())
    val deadline = System.currentTimeMillis() + timeoutMs
    while (System.currentTimeMillis() < deadline) {
        val visible =
            probeAssertion {
                onNodeWithTag("SettingsScreen", useUnmergedTree = true).assertIsDisplayed()
            }
        if (visible) return true
        // Check both text and resource id (testTagsAsResourceId) for robustness on PlayStore image
        if (
            device.hasObject(By.text("Font Family")) ||
            device.hasObject(By.res("SettingsScreen")) ||
            device.hasObject(By.text("Appearance")) ||
            device.hasObject(By.text("Font Size")) ||
            device.hasObject(By.text("Cursor")) ||
            device.hasObject(
                By.text("Sessions"),
            ) // drawer still open but Settings should be overlay
        ) {
            // If we see Sessions but not Settings, drawer is open but Settings not yet — keep waiting
            if (
                device.hasObject(By.text("Font Family")) ||
                device.hasObject(By.res("SettingsScreen")) ||
                device.hasObject(By.text("Appearance"))
            ) {
                return true
            }
        }
        Thread.sleep(200)
    }
    return false
}

/** Wait until [SettingsScreen] test-tag is displayed. */
fun AndroidComposeTestRule<*, *>.waitForSettingsScreen(timeoutMs: Long = 60_000) {
    val device = UiDevice.getInstance(InstrumentationRegistry.getInstrumentation())
    // CI PlayStore emulator (swiftshader, 2 cores, 1536M) is much slower than local google_apis:
    // 30s still timed out in  (8 failures). Use 60s and poll both Compose tag and
    // UiDevice resource/text so we succeed even when Compose idle is delayed or semantics are merged.
    waitUntil(timeoutMillis = timeoutMs) {
        probeAssertion { onNodeWithTag("SettingsScreen", useUnmergedTree = true).assertIsDisplayed() } ||
            device.hasObject(By.res("SettingsScreen")) ||
            device.hasObject(By.text("Font Family")) ||
            device.hasObject(By.text("Appearance")) ||
            device.hasObject(By.text("Font Size")) ||
            device.hasObject(By.text("Cursor style"))
    }
}

// ── 单元格坐标 ───────────────────────────────────────

/**
 * 单元格物理尺寸（px）：桥给的是逻辑值，必须乘光栅缩放——直接当 px 用会小 2~3 倍。
 *
 * 缩放系数须与运行时同口径（`TerminalRuntime.spToPxScale` = 密度 × 系统字体缩放，
 * 亦即推给原生 `setRasterScale` 的值）：渲染的四边形尺寸是 `cell_metrics × raster_scale`
 * （`native/src/render/pass.rs`），只乘密度会在系统字体缩放 ≠ 1 时与渲染相差
 * 一个 fontScale 倍，触摸数学随之错位。
 */
fun terminalCellSizePx(activity: Activity, bridge: Bridge): Pair<Float, Float> {
    val spToPxScale =
        activity.resources.displayMetrics.density * activity.resources.configuration.fontScale
    return Pair(bridge.getCellWidth() * spToPxScale, bridge.getCellHeight() * spToPxScale)
}

/**
 * 视口内第 [col] 列、第 [row] 行单元格中心的**屏幕**坐标。
 *
 * 屏幕尺寸随设备而变（CI 模拟器 320×640、真机 1080×2400），硬编码坐标在这些尺寸上
 * 直接越界，手势根本没进终端——外部表现是「长按没建选择」「点击没关选择」，与产品无关。
 */
fun terminalCellCenterOnScreen(activity: Activity, bridge: Bridge, col: Int, row: Int): Pair<Int, Int> {
    val (cellWidth, cellHeight) = terminalCellSizePx(activity, bridge)
    val location = IntArray(2)
    findTerminalSurface(activity).getLocationOnScreen(location)
    return (location[0] + (col + 0.5f) * cellWidth).toInt() to
        (location[1] + (row + 0.5f) * cellHeight).toInt()
}

/**
 * 视口列数（表面实际宽度 ÷ 单元格宽度）。网格列数随屏幕宽度变化，依赖固定列数的
 * 用例必须先量出列数：超出即折行，文本查询按整串匹配时恒不成立。
 */
fun terminalGridColumns(activity: Activity, bridge: Bridge): Int {
    val (cellWidth, _) = terminalCellSizePx(activity, bridge)
    return (findTerminalSurface(activity).width / cellWidth).toInt()
}

/** 抽屉边缘手势区宽度（dp），与 `TerminalSurface.onTouchEvent` 的判据同值。 */
private const val DRAWER_EDGE_WIDTH_DP = 32

/**
 * 终端表面**不吃触摸**的最左列号：中心点仍在边缘区内的列一律被丢弃
 * （`TerminalSurface.onTouchEvent` 对 `event.x < 32dp` 直接返回 false，
 * 手势交给抽屉边缘滑动手势）。
 *
 * 必须用本函数而非写死列号：边缘区是 dp 而列是设备相关的
 * （1080×2400@420dpi 时约占前 4 列，320×640@160dpi 时约占前 3 列），
 * 写死的列号在这两种尺寸上会分别落在区内与区外。
 */
fun firstColumnBeyondDrawerEdge(activity: Activity, bridge: Bridge): Int {
    val (cellWidth, _) = terminalCellSizePx(activity, bridge)
    val edgePixels = DRAWER_EDGE_WIDTH_DP * activity.resources.displayMetrics.density
    val columns = terminalGridColumns(activity, bridge)
    var column = 0
    while (column < columns && (column + 0.5f) * cellWidth <= edgePixels) {
        column++
    }
    return column
}

// ── GPU frame helpers ───────────────────────────────

fun getDisplayWidth(): Int {
    val context = InstrumentationRegistry.getInstrumentation().targetContext
    val windowManager = context.getSystemService(android.view.WindowManager::class.java)
    return windowManager.currentWindowMetrics.bounds.width()
}

fun analyzeNonBlackRatio(bitmap: Bitmap): Double {
    val width = bitmap.width
    val height = bitmap.height
    val pixels = IntArray(width * height)
    bitmap.getPixels(pixels, 0, width, 0, 0, width, height)
    var nonBlack = 0L
    for (pixel in pixels) {
        val r = (pixel shr 16) and 0xFF
        val g = (pixel shr 8) and 0xFF
        val b = pixel and 0xFF
        if (r > 15 || g > 15 || b > 15) nonBlack++
    }
    return nonBlack.toDouble() / pixels.size.toDouble()
}

fun injectLongPress(view: View, x: Float, y: Float) {
    val dt = SystemClock.uptimeMillis()
    // Must NOT block the main thread — GestureDetector uses a Handler on the
    // main-thread looper.  If we dispatch DOWN then sleep(800) on the main
    // thread the long-press timer message is queued but never processed before
    // ACTION_UP arrives — GestureDetector then cancels the pending long-press
    // and treats the gesture as a tap (the root cause of long-press
    // selection assertions failing in text-selection tests).
    view.post {
        view.dispatchTouchEvent(MotionEvent.obtain(dt, dt, MotionEvent.ACTION_DOWN, x, y, 0))
    }
    // Sleep on the CALLER test thread — the main thread is free to process
    // the GestureDetector long-press handler (~500 ms) and the selection
    // state update before MOVE / UP arrive.
    try {
        Thread.sleep(1200)
    } catch (_: InterruptedException) {
        Thread.currentThread().interrupt()
    }
    view.post {
        view.dispatchTouchEvent(
            MotionEvent.obtain(dt, dt + 1200, MotionEvent.ACTION_MOVE, x + 1f, y + 1f, 0),
        )
        view.dispatchTouchEvent(
            MotionEvent.obtain(dt, dt + 1250, MotionEvent.ACTION_UP, x + 1f, y + 1f, 0),
        )
    }
    // Give the main thread time to process MOVE/UP before the caller
    // continues to the "Then" assertion step.
    try {
        Thread.sleep(300)
    } catch (_: InterruptedException) {
        Thread.currentThread().interrupt()
    }
}

// 点击时序（实测根因：DOWN→UP 100ms 恰压在框架 TAP_TIMEOUT 边界上，
// GestureDetector 不认 tap，onSingleTapUp 永不触发；
// 真实触摸 DOWN→UP 约 50ms，取 50ms。）
private const val TAP_DOWN_UP_MILLIS = 50L

fun injectTap(view: View, x: Float, y: Float) {
    val dt = SystemClock.uptimeMillis()
    view.post {
        view.dispatchTouchEvent(MotionEvent.obtain(dt, dt, MotionEvent.ACTION_DOWN, x, y, 0))
    }
    try {
        Thread.sleep(TAP_DOWN_UP_MILLIS)
    } catch (_: InterruptedException) {
        Thread.currentThread().interrupt()
    }
    view.post {
        view.dispatchTouchEvent(
            MotionEvent.obtain(dt, dt + TAP_DOWN_UP_MILLIS, MotionEvent.ACTION_UP, x, y, 0),
        )
    }
}

// ── Selection assertion helpers ─────────────────────

/**
 * 用例收尾：把**共用会话**恢复干净——选区与浮动菜单、滚动偏移、屏幕内容、回滚。
 *
 * `TerminalRuntime` 是 `@Singleton`，`:app:connectedDebugAndroidTest` 全部用例
 * 同进程：选区/菜单/滚动偏移都跨用例存活。不清场的用例会把上千行回滚、悬着的
 * 选区菜单留给后继用例，全量文本查询越过原生 `QUERY_TIMEOUT_MS` 即返回空串，
 * 渲染快照也可能超时——外部表现是与被测行为无关的成片红灯。ED3 后回滚必须真的
 * 归零，不归零就大声失败：静默失效会把污染继续往后传。
 */
fun AndroidComposeTestRule<*, *>.cleanUpTerminalState() {
    closeSettingsOverlay()
    // 选区与其浮动菜单同为跨用例存活的状态：不清掉，下一个用例的「全选后必须
    // 出现复制」会对着上一个用例留下的菜单判空——判红原因与全选无关。
    runOnMainThread { (activity as MainActivity).terminalViewModel.clearSelection() }
    val bridge = getBridge() ?: return
    bridge.setScrollOffset(0)
    if (!bridge.feedTerminal("\u001B[2J\u001B[3J\u001B[H".toByteArray(Charsets.UTF_8))) {
        throw AssertionError("清屏送显失败")
    }
    val cleared =
        UxTestUtils.pollUntilTrue(timeoutMs = 5_000, intervalMs = 100) {
            bridge.scrollbackLength() == 0
        }
    if (cleared == null) {
        throw AssertionError("ED3 必须真的清空回滚（残留 ${bridge.scrollbackLength()} 行）")
    }
}

fun findTerminalSurface(activity: Activity): View {
    val content = activity.findViewById<View>(android.R.id.content) as ViewGroup
    return content.findViewWithTag<View>("TerminalSurfaceView")
        ?: run {
            fun traverse(group: ViewGroup): View? {
                for (i in 0 until group.childCount) {
                    val child = group.getChildAt(i)
                    if (child is android.view.TextureView) return child
                    if (child is ViewGroup) {
                        val result = traverse(child)
                        if (result != null) return result
                    }
                }
                return null
            }
            traverse(content) ?: content
        }
}
