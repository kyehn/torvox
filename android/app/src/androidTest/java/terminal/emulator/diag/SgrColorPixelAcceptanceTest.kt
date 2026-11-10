package terminal.emulator.diag

import androidx.test.platform.app.InstrumentationRegistry
import androidx.test.uiautomator.UiDevice
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Test
import terminal.emulator.ComposeTerminalLogcatTest
import terminal.emulator.UxTestUtils
import terminal.emulator.bridge.Bridge
import terminal.emulator.countBluishPixels
import terminal.emulator.countGreenishPixels
import terminal.emulator.countReddishPixels
import terminal.emulator.getBridge
import terminal.emulator.placeTextAtRow
import terminal.emulator.waitForSession
import terminal.emulator.waitForTerminalPixels

/**
 * 颜色像素验收：SGR 彩色文本必须在屏幕上产生对应色相的主导像素，
 * 而非只显示背景（回归“颜色文本只显示背景”类缺失）。
 *
 * 标记写进**运行时正在呈现的会话**：渲染循环与截图同源，无需任何跨会话
 * 呈现技巧。隔离会话 + 原生直绘不可行——`render_inner` 的空闲分支按
 * `last_frame` 所属会话决定是否重绘，运行时会话一旦产出新输出即覆盖隔离帧，
 * 且此后隔离会话在空闲分支恒被判为“会话不一致”而永不重绘，标记永久丢失
 * （实测红/蓝像素恒为 0）。字节只过 Ghostty 解析器（直写 VT，不经 shell 行编辑）。
 */
class SgrColorPixelAcceptanceTest : ComposeTerminalLogcatTest() {

    private lateinit var device: UiDevice

    @Before
    fun setUp() {
        device = UiDevice.getInstance(InstrumentationRegistry.getInstrumentation())
        composeTestRule.waitForSession()
        composeTestRule.waitForTerminalPixels()
    }

    private fun awaitBridge(): Bridge {
        // 会话孵化中桥为 null：确定性轮询就绪，而非单次读取碰运气。
        var ready: Bridge? = null
        val seen =
            UxTestUtils.pollUntilTrue(timeoutMs = 30_000, intervalMs = 200) {
                ready = composeTestRule.getBridge()
                ready != null
            }
        assertNotNull("运行时桥必须就绪", seen)
        return requireNotNull(ready) { "运行时桥必须就绪" }
    }

    /**
     * 写标记并等渲染帧真正带上它。
     *
     * 直写 VT 不置脏：[placeTextAtRow] 的判据是 **VT 光标**（同步查询），
     * 光标到位不等于已经上屏，截图可能仍是旧帧。故这里再等**最近一帧的光标**
     * 落到目标格——该值只由渲染线程在真正出帧时发布，等到了就说明这一帧已
     * 带上标记。光标闪烁时该值为 -1，轮询会继续，直到撞上可见相位。
     */
    private fun placeAndAwaitFrame(bridge: Bridge, row: Int, text: String, visibleLength: Int) {
        placeTextAtRow(bridge, row, text)
        val target = (row.toLong() shl 32) or visibleLength.toLong()
        val rendered =
            UxTestUtils.pollUntilTrue(timeoutMs = 15_000, intervalMs = 50) {
                bridge.renderedCursorPacked() == target
            }
        assertNotNull("标记必须已随帧上屏 (行=$row)", rendered)
    }

    @Test
    fun sgrRedTextProducesRedPixels() {
        val bridge = awaitBridge()
        // 三色各占一行：同行的三段在小网格（CI 模拟器仅 25 列）上必折行，
        // 折行后行内位置随网格变化。行号取视口上部，任何后续输出滚动视口都
        // 不会把它们推出可见区。
        val markers = listOf(
            Triple(2, "EEE_RED", "\u001B[31m"),
            Triple(4, "EEE_GREEN", "\u001B[32m"),
            Triple(6, "EEE_BLUE", "\u001B[34m"),
        )

        // 判据取「彩色帧 − 无色帧」的差，而不是「有标记帧 − 标记前帧」。
        // 后者的基线是上一个用例留下的屏幕：写标记会顺手覆盖那里的彩色内容，
        // 该色的增量于是可以为负（实测蓝像素 182 → 58），判据随用例顺序漂移。
        // 同一段文字先按默认色写一遍，残留内容在两帧里同时出现、直接抵消；
        // 只有字形本身的颜色留下差值。
        // 先复位 SGR：默认色那一遍必须落在默认属性上，否则上一个用例留下的
        // 颜色/反显会跟着基线帧一起被量进去。
        assertTrue("SGR 复位送显失败", bridge.feedTerminal("\u001B[0m".toByteArray(Charsets.UTF_8)))
        for ((row, text) in markers.map { it.first to it.second }) {
            placeAndAwaitFrame(bridge, row, text, text.length)
        }
        val plain = device.takeScreenshot() ?: throw AssertionError("截图失败")
        val plainRed = countReddishPixels(plain)
        val plainGreen = countGreenishPixels(plain)
        val plainBlue = countBluishPixels(plain)

        // 彩色帧按 6→4→2 写：与基线帧的写入顺序相反，末帧光标落在 (2,7)，
        // 与基线帧的末帧光标 (6,7) 不同，「已上屏」的判据不会误命中旧帧。
        for ((row, text, sgr) in markers.reversed()) {
            placeAndAwaitFrame(bridge, row, "$sgr$text\u001B[0m", text.length)
        }
        // 呈现是异步的（运行时循环节拍）：字形首次入图集还要再等一帧；盲等
        // 固定时长在慢机上不可靠，改为轮询截图直到三色差值出现，超时大声失败。
        // 运行时循环持续呈现同一网格，每轮截图都是有效采样，不存在覆盖竞态。
        var red = 0
        var green = 0
        var blue = 0
        val gained =
            UxTestUtils.pollUntilTrue(timeoutMs = 15_000, intervalMs = 500) {
                val shot = device.takeScreenshot() ?: return@pollUntilTrue false
                red = countReddishPixels(shot) - plainRed
                green = countGreenishPixels(shot) - plainGreen
                blue = countBluishPixels(shot) - plainBlue
                red > PIXEL_GAIN_THRESHOLD &&
                    green > PIXEL_GAIN_THRESHOLD &&
                    blue > PIXEL_GAIN_THRESHOLD
            }
        assertNotNull(
            "SGR 三色必须呈现 (差 红=$red 绿=$green 蓝=$blue 无色基线=$plainRed/$plainGreen/$plainBlue)",
            gained,
        )
        assertTrue(
            "SGR 红色文本必须产生红色像素 (差=$red 无色基线=$plainRed)",
            red > PIXEL_GAIN_THRESHOLD,
        )
        assertTrue(
            "SGR 绿色文本必须产生绿色像素 (差=$green 无色基线=$plainGreen)",
            green > PIXEL_GAIN_THRESHOLD,
        )
        assertTrue(
            "SGR 蓝色文本必须产生蓝色像素 (差=$blue 无色基线=$plainBlue)",
            blue > PIXEL_GAIN_THRESHOLD,
        )
    }

    companion object {
        private const val PIXEL_GAIN_THRESHOLD = 20
    }
}
