package terminal.emulator

import androidx.compose.ui.test.onAllNodesWithText
import org.junit.Assert.assertTrue
import org.junit.Test
import terminal.emulator.ComposeTerminalLogcatTest

/**
 * 前台桌面通知（OSC 9）：以应用内提示呈现，且不产生平台提示窗口条目。
 *
 * 判别力：这条路径从前走平台提示窗口，而平台提示窗口受同一 UID 窗口独占、窗口生命周期
 * 与按包限额三条约束——每一条到期的提示都会在平台侧留下 W 级条目。故此处断言两件事：
 * 提示看得见（应用内通道真的接上了），且平台侧没有提示窗口条目（旧路径没有残留）。
 *
 * 读 logcat 的时刻在提示到期之后：窗口生命周期的告警是提示到期时才由平台打出的，
 * 读到得太早只能证明「还没到期」，证明不了「没有走那条路」。
 */
class DesktopNotificationInstrumentedTest : ComposeTerminalLogcatTest() {

    @Test
    fun foreground_notification_is_shown_in_app_without_platform_toast() {
        val bridge = composeTestRule.requireBridge()
        composeTestRule.waitForSession()
        awaitShellPrompt(bridge)
        val sinceMs = System.currentTimeMillis()
        bridge.writeToPty("printf '\\033]9;$MARKER\\007'\n".toByteArray(Charsets.US_ASCII))
        val shown =
            UxTestUtils.pollUntilTrue(timeoutMs = 20_000, intervalMs = 200) {
                composeTestRule.onAllNodesWithText(MARKER, substring = true)
                    .fetchSemanticsNodes()
                    .isNotEmpty()
            }
        assertTrue("前台桌面通知必须以应用内提示呈现", shown != null)
        // 提示的时长是平台提示窗口的常量级（数秒）：等它到期，窗口生命周期的告警才有
        // 可能出现。等不到就等于没有那条路——读得太早只会漏判。
        Thread.sleep(TOAST_LIFETIME_SETTLE_MS)
        val platformToastLines =
            UxTestUtils.logcatSince(sinceMs).filter { line ->
                PLATFORM_TOAST_MARKERS.any { marker -> line.contains(marker) }
            }
        assertTrue(
            "平台不得出现提示窗口条目，实际：$platformToastLines",
            platformToastLines.isEmpty(),
        )
    }

    private companion object {
        /** 通知正文标记：终端网格画在 SurfaceView 上，不进语义树，故只有提示条带它。 */
        const val MARKER = "OSC9_MARKER"

        /** 提示到期后仍需静置的时长：覆盖平台提示窗口的时长量级。 */
        const val TOAST_LIFETIME_SETTLE_MS = 8_000L

        /**
         * 平台提示窗口的告警标记：窗口冲突、令牌生命周期、窗口销毁与按包限额四类。
         *
         * 逐条写死而不是只匹配 "toast"：设备上别的进程也可能打提示窗口的条目，
         * 宽匹配会把它们算到本应用头上。其中「窗口销毁」（`Toast (client)`）与
         * 「令牌已销毁」（`Toast already killed`）实测在**单条**提示到期时就会出现，
         * 其余几条要连发或超额才出现。
         */
        val PLATFORM_TOAST_MARKERS =
            listOf(
                "Adding more than one toast window",
                "Toast already killed",
                "Toast already cancelled",
                "ToastPresenter",
                "Toast (client)",
                "above allowed toast quota",
                "BadTokenException",
            )
    }
}
