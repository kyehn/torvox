package terminal.emulator.ui

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
import terminal.emulator.TerminalLogcatRule
import terminal.emulator.UxTestUtils
import terminal.emulator.findTerminalSurface
import terminal.emulator.getBridge
import terminal.emulator.util.runCatchingCancellable
import terminal.emulator.waitForSession

/**
 * 原生 surface 失效后的自愈：持续注入 surface 级失败 → 原生判死并上报 →
 * 宿主换掉 `SurfaceView` 取到新窗口 → 画面重新有墨迹且新命令仍能落格。
 *
 * 真机上「BufferQueue 被遗弃」由 SurfaceFlinger 回收制造，应用侧无法复现；
 * 不注入就只能等系统回收，本路径永远没有回归护栏（模拟器全量套件实测：
 * 废弃后 22 分钟 3074 帧全败，27 个用例退化为「零像素」断言）。
 */
@RunWith(JUnit4::class)
class SurfaceLossRecoveryInstrumentedTest {
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

        /** Catppuccin Mocha 背景（0x1E1E2E）：与应用默认主题一致，用作「无墨迹」基准。 */
        private const val BACKGROUND_COLOR = 0x1E1E2E

        /** 选区所在视口行与起始列：与网格列数无关的固定量。 */
        private const val SELECTION_ROW = 0
        private const val SELECTION_START_COL = 2
    }

    @get:Rule val terminalLogcatRule = TerminalLogcatRule()

    @get:Rule
    val notificationPermission =
        GrantPermissionRule.grant(android.Manifest.permission.POST_NOTIFICATIONS)

    @get:Rule val composeTestRule = createAndroidComposeRule<MainActivity>()

    private lateinit var device: UiDevice

    @Before
    fun setUp() {
        device = UiDevice.getInstance(InstrumentationRegistry.getInstrumentation())
        composeTestRule.waitForSession()
        assertNotNull(
            "运行时桥必须就绪",
            UxTestUtils.pollUntilTrue(timeoutMs = 30_000, intervalMs = 200) {
                composeTestRule.getBridge() != null
            },
        )
    }

    private fun bridge() = composeTestRule.getBridge() ?: throw AssertionError("bridge null")

    private fun pumpAndText(): String? {
        runCatchingCancellable { terminal.emulator.bridge.NativeBridge.pollEvent() }
        return bridge().getTerminalText()
    }

    /** 经 shell 打印一行，回显重发至多 3 次（冷 stdin 竞态，同其他用例口径）。 */
    private fun printAndAwait(command: String, needle: String) {
        assertNotNull(
            "shell prompt 未出现",
            UxTestUtils.pollUntilTrue(timeoutMs = 30_000, intervalMs = 100) {
                val text = pumpAndText().orEmpty()
                text.contains("$") || text.contains("#")
            },
        )
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
        var ink = 0L
        var y = top
        while (y < bottom) {
            var x = left
            while (x < right) {
                ink += kotlin.math.abs(bitmap.getPixel(x, y) - BACKGROUND_COLOR).toLong() and 0xFFFFFF
                x += INK_SAMPLE_STEP_PX
            }
            y += INK_SAMPLE_STEP_PX
        }
        return ink
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
            activity.terminalViewModel.startSelection(SELECTION_ROW, SELECTION_START_COL)
            activity.terminalViewModel.updateSelection(SELECTION_ROW, SELECTION_START_COL + 3)
            activity.terminalViewModel.endSelection()
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

    /** 轮询至选择菜单的「复制」项出现在无障碍树中（返回耗时，null 表示超时）。 */
    private fun awaitCopyAction(): Long? =
        UxTestUtils.pollUntilTrue(timeoutMs = RECOVERY_TIMEOUT_MS, intervalMs = RECOVERY_POLL_MILLIS) {
            device.hasObject(By.text("复制"))
        }
}
