package terminal.emulator.ui

import androidx.compose.ui.test.junit4.v2.createAndroidComposeRule
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Rule
import org.junit.Test
import terminal.emulator.MainActivity
import terminal.emulator.TerminalLogcatTest
import terminal.emulator.UxTestUtils
import terminal.emulator.awaitBridge
import terminal.emulator.awaitShellPrompt
import terminal.emulator.bridge.Bridge
import terminal.emulator.restoreSharedShellState
import terminal.emulator.scrollViewportToBottom
import terminal.emulator.waitForSession
import java.io.File

/**
 * STATUS ①「shell 启动/响应缓慢」里与渲染唤醒有关的那一半：**没有输入**的后台输出，
 * 在终端静置超过 5s（渲染循环已落到空闲闭锁）之后，多久才能上屏。
 *
 * 与 [terminal.emulator.BackspaceLatencyTest] 互补——那里每次退格都先经
 * `notifyRender()` 刷新空闲时钟，量的是「有输入」的路径；这里的命令自己在 shell 里
 * 睡 6 秒，醒来后打印，回显到达时**没有任何 Kotlin 侧信号**。
 *
 * ## 为什么不用文本查询来量
 *
 * 早先的版本用 `getTerminalText()` 轮询「文本里出现时间戳」，这是错的量具：
 * 该查询要把**整条回滚逐格读一遍**，单次 30～45ms（回滚越长越慢），于是轮询
 * 周期本身就有百毫秒级抖动——旧实现与修复后读数都是 10/102 的双峰，实际量的是
 * 查询开销而不是送显延迟（该版本已按此结论重写，别再退回）。
 *
 * ## 现在的量具
 *
 * 1. shell 在**打印之前**把 `date +%s%3N` 写进文件（`>` 落页缓存，另一进程立即可见），
 *    测试直接读文件得到「打印时刻」，一次 stat+read 十几微秒，不进 JNI；
 * 2. 打印一行带换行的数字，于是**光标视口行**在那一帧改变；
 * 3. 测试以 1ms 轮询 `renderedCursorPacked()`——渲染线程逐帧发布的原子量，
 *    一次注册表读锁 + 原子读，不碰 VT，也不排队；
 * 4. 延迟 = 见到新光标行 − 文件里的打印时刻。误差上限是 FIFO 的一帧 17ms，
 *    远小于旧量具的上百毫秒噪声。
 *
 * 光标闪烁会让可见性翻转为「不可见」（打包值 -1），既可能掩盖真正的呈现帧、也会
 * 制造假变化，故先用 DECRST 12 + 稳块形状关掉闪烁——闪烁与本题无关。
 */
class IdleOutputWakeTest : TerminalLogcatTest() {
    @get:Rule val composeTestRule = createAndroidComposeRule<MainActivity>()

    private companion object {
        /** shell 先睡这么久，确保渲染循环已越过 5s 空闲阈值。 */
        const val IDLE_SLEEP = "6"

        /** 暖样本的睡眠：命令刚写入，空闲时钟新鲜，闭锁仍是 17ms 节奏。 */
        const val WARM_SLEEP = "0.3"

        /** 冷样本个数：闭锁下延迟是 0..500ms 的均匀分布，取均值判别，单个样本的运气不足以混过预算。 */
        const val COLD_SAMPLES = 7

        /** 冷样本均值预算（毫秒）。 */
        const val COLD_MEAN_BUDGET_MS = 120.0

        /** 冷样本中位数预算（毫秒）：防止「一半样本极快、一半等满闭锁」混过均值。 */
        const val COLD_MEDIAN_BUDGET_MS = 100L

        /** 轮询上限：输出再慢也不该超过这个窗口，超时即判失败并附现场。 */
        const val WAIT_BUDGET_MS = 10_000L
    }

    @Before
    fun setUp() {
        composeTestRule.waitForSession()
        // 本用例的量具依赖「命令回显」这一共享 shell 状态：回显关着时命令照样
        // 执行，但「命令已被 shell 接收」这一信号永远不出现，暖样本会误判超时。
        // 故先复位共享 shell（它同时断言 tty 可交互），把隐含前提变成显式前提。
        val bridge = composeTestRule.awaitBridge()
        bridge.restoreSharedShellState()
        // 视口必须归位到底部：本用例量的是「输出**出现在屏幕上**的延迟」，
        // 而回显判据查的是视口文本（不是全量回滚）。视口若被前一个用例留在
        // 上方（终端不回卷），回显会落在回滚里、视口里永远看不见——
        // 那是量具的前提缺失，不是被测行为。
        scrollViewportToBottom(bridge, composeTestRule.activity.terminalViewModel.runtime)
    }

    @Test
    fun background_output_after_idle_does_not_wait_for_idle_latch() {
        val bridge = composeTestRule.awaitBridge()
        awaitShellPrompt(bridge)

        // 先热身一次：首个命令还包含 shell 的一次性成本（历史加载、命令缓存）。
        measureOnce(bridge, WARM_SLEEP, label = "warmup")

        val cold = (1..COLD_SAMPLES).map { measureOnce(bridge, IDLE_SLEEP, label = "cold$it") }
        val mean = cold.average()
        val median = cold.sorted()[cold.size / 2]
        UxTestUtils.metric("idle_output_cold_mean_ms", mean.toLong())
        UxTestUtils.metric("idle_output_cold_median_ms", median)
        UxTestUtils.metric("idle_output_cold_max_ms", cold.max())

        assertTrue(
            "静置后的后台输出均值 ${mean}ms / 中位数 ${median}ms（样本=$cold）：" +
                "渲染循环仍在等空闲闭锁，而不是被 PTY 输出唤醒",
            mean <= COLD_MEAN_BUDGET_MS && median <= COLD_MEDIAN_BUDGET_MS,
        )
    }

    /**
     * 跑一次「睡 [sleepSeconds] 秒后打印毫秒时间戳」，返回从打印到**该行被渲染**的毫秒数。
     *
     * 时间戳由 shell 自己写文件（打印之前），测试直接读文件，故不受 JNI 查询开销影响；
     * 呈现判据是渲染线程发布的光标行变化（见类文档）。
     */
    private fun measureOnce(bridge: Bridge, sleepSeconds: String, label: String): Long {
        // 稳块 + 关闪烁：闪烁的可见性翻转会把帧信号变成 -1（不可见）。
        assertTrue("光标形状设置失败", bridge.feedTerminal("\u001B[2 q\u001B[?12l".toByteArray()))

        val stampFile = File(File(composeTestRule.activity.filesDir, "home"), ".wake-stamp-$label")
        stampFile.delete()

        val sleepMillis = (sleepSeconds.toDouble() * 1_000).toLong()
        // 一次 date：写文件在打印之前（相隔不到 1ms），故文件里的时刻就是打印时刻。
        // 行尾的一次性注记让「本样本的回显已落地」可判——否则上一样本的同类行会命中。
        val nonce = "wake-$label-${System.nanoTime()}"
        val command =
            "sleep $sleepSeconds; S=\$(date +%s%3N); " +
                "printf '%s' \"\$S\" > ${stampFile.absolutePath}; printf '%s\\n' \"\$S\" # $nonce\n"
        assertTrue("[$label] 命令未写入 PTY", bridge.writeToPty(command.toByteArray()))

        // 回显先落地再计时：慢查询（数十毫秒）留在静置窗口里，不进关键路径。
        val echoed =
            UxTestUtils.pollUntilTrue(timeoutMs = 30_000, intervalMs = 20) {
                bridge.getVisibleText().contains(nonce)
            }
        assertNotNull("[$label] 命令回显未出现", echoed)

        // 留出 200ms 余量再开始轮询，避免把「等待」算进延迟；闭锁相位因此仍然随机。
        Thread.sleep((sleepMillis - 200).coerceAtLeast(0))

        // 基线必须取在回显**之后**：命令行本身就会移动光标，取早了轮询会立刻命中回显的变化。
        val baseline = bridge.renderedCursorPacked()
        val deadline = System.nanoTime() + WAIT_BUDGET_MS * 1_000_000
        var seen = 0L
        var latency = -1L
        while (System.nanoTime() < deadline) {
            val packed = bridge.renderedCursorPacked()
            val stamp = readStamp(stampFile)
            if (stamp != null && packed >= 0 && packed != baseline) {
                seen = packed
                latency = (System.currentTimeMillis() - stamp).coerceAtLeast(0)
                break
            }
        }
        UxTestUtils.metric("idle_output_${label}_cursor_packed", seen)
        assertTrue(
            "[$label] 输出在 ${WAIT_BUDGET_MS}ms 内未反映到渲染光标（基线=$baseline 现值=$seen，" +
                "尾部=${bridge.getVisibleText().takeLast(160)}）",
            latency >= 0,
        )

        // 落地断言留在计时之外：文本里最终必须出现那个时间戳。
        val stamp = requireNotNull(readStamp(stampFile)) { "[$label] 时间戳文件为空" }
        val landed =
            UxTestUtils.pollUntilTrue(timeoutMs = 5_000, intervalMs = 50) {
                bridge.getTerminalText().orEmpty().contains(stamp.toString())
            }
        assertNotNull("[$label] 时间戳未出现在终端文本中", landed)

        UxTestUtils.metric("idle_output_${label}_ms", latency)
        return latency
    }

    /** 读 shell 写入的时间戳；文件尚未创建/内容不全时返回 null。 */
    private fun readStamp(file: File): Long? = try {
        file.readText().trim().toLongOrNull()
    } catch (expected: java.io.IOException) {
        null
    }
}
