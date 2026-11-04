package terminal.emulator.benchmark

import android.os.SystemClock
import androidx.benchmark.macro.FrameTimingMetric
import androidx.benchmark.macro.junit4.MacrobenchmarkRule
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.uiautomator.By
import androidx.test.uiautomator.UiDevice
import androidx.test.uiautomator.Until
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith

private const val SCROLL_ITERATION_COUNT = 3

/** 一次手势的移动步数：决定滑动速度，太快会被识别成 fling，太慢拉长测量窗口。 */
private const val SWIPE_STEPS = 10

/** `seq 500` 输出渲染稳定性探测：两次读数的间隔。 */
private const val OUTPUT_STABLE_POLL_MILLIS = 300L

/** 上述探测的总超时：超时说明 shell 未就绪或输入没有送达，必须失败。 */
private const val OUTPUT_STABLE_TIMEOUT_MILLIS = 20_000L

/** 帧统计读取在 0 帧时的重试间隔：覆盖软件渲染的一个 vsync 帧时间。 */
private const val SCROLL_FRAME_READ_RETRY_MILLIS = 300L

private const val SCROLL_FRAME_READ_ATTEMPT_COUNT = 3

/** force-stop 后等设备空闲的上限，让软件渲染的图形栈喘口气。 */
private const val EMULATOR_IDLE_TIMEOUT_MILLIS = 2_000L

/**
 * 滚动的帧级宏基准：在回滚区被填满的状态下上滑、下滑各一次。
 *
 * TESTING 性能条款把「上滑/下滑正确移动终端，无卡顿」列为覆盖范围，app 侧已有
 * 行为量化测试钉正确性，这里补宏基准侧的流畅度与耗时。内容生成走真实输入链路：
 * `input text` 把 `seq 500` 敲进 shell（应用首启自动创建默认会话，光标不闪烁，
 * 静止终端不出帧，帧计数稳定即可判定输出渲染完毕），随后清零帧统计，只测量
 * 两次滑动手势本身。
 *
 * 断言与交互基准同构：至少出帧（什么都不画说明滚动没生效）、总耗时在预算内；
 * 帧耗时百分位随 [printMeasured] 落进 CI，作为下一步收紧断言的实测基准。
 */
@RunWith(AndroidJUnit4::class)
class ScrollPerformanceBenchmark {
    @get:Rule
    val benchmarkRule = MacrobenchmarkRule()

    @Test
    fun scrollWithFullScrollback() {
        benchmarkRule.measureRepeated(
            packageName = TARGET_PACKAGE,
            metrics = listOf(FrameTimingMetric()),
            iterations = SCROLL_ITERATION_COUNT,
            setupBlock = {
                grantNotificationPermission(device)
                recoverEmulator(device)
            },
            measureBlock = {
                startActivityAndWait()
                requireElement(device, TERMINAL_SCREEN_RESOURCE_ID, "终端界面")
                typeScrollbackCommand(device)
                resetFrameStats(device)
                val elapsed = elapsedMillis {
                    scrollTerminal(device, towardHistory = true)
                    scrollTerminal(device, towardHistory = false)
                    device.waitForIdle()
                }
                val stats = readFrameStatsAfterRender(device)
                check(stats.frames >= MIN_ANIMATION_FRAME_COUNT) {
                    "滚动未产生帧：frames=${stats.frames}"
                }
                check(elapsed <= SCROLL_BUDGET_MILLIS) {
                    "上滑下滑耗时 ${elapsed}ms 超过预算 ${SCROLL_BUDGET_MILLIS}ms"
                }
                printMeasured("scroll", elapsed, stats)
            },
        )
    }

    /** 元素缺失 MUST 失败：找不到元素说明界面没到位，继续测量只会产出无意义的数字。 */
    private fun requireElement(device: UiDevice, resourceId: String, label: String) {
        val element = device.wait(Until.findObject(By.res(resourceId)), UI_TIMEOUT_MILLIS)
        checkNotNull(element) { "未找到$label（$resourceId）" }
    }

    /**
     * 软件渲染的模拟器连续跑长宏基准会耗尽图形栈，任务以输出插件 EOF 崩溃，
     * 因此每个用例前先强制停止被测应用，再等设备空闲让图形栈恢复。
     */
    private fun recoverEmulator(device: UiDevice) {
        device.executeShellCommand("am force-stop $TARGET_PACKAGE")
        device.waitForIdle(EMULATOR_IDLE_TIMEOUT_MILLIS)
    }

    /**
     * 向 shell 敲入 `seq 500` 并等输出渲染完毕。
     *
     * `input text` 走硬件键盘事件通道，不经过输入法，终端像收到物理键盘一样把
     * 字符送进 shell（`%s` 是 `input text` 的空格转义）；回车执行后 500 行输出
     * 把回滚区填满。判定「渲染完毕」用帧计数稳定：应用没有闪烁光标，渲染循环
     * 只在内容变化时出帧，静止即计数不再增长——这比等待固定时长快且确定。
     */
    private fun typeScrollbackCommand(device: UiDevice) {
        device.executeShellCommand("input text 'seq%s500'")
        device.executeShellCommand("input keyevent 66")
        val deadline = SystemClock.elapsedRealtime() + OUTPUT_STABLE_TIMEOUT_MILLIS
        var previous = readFrameStats(device).frames
        while (SystemClock.elapsedRealtime() < deadline) {
            SystemClock.sleep(OUTPUT_STABLE_POLL_MILLIS)
            val current = readFrameStats(device).frames
            if (current == previous && current > 0) return
            previous = current
        }
        error("seq 500 的输出未渲染完成（帧数未增长）：shell 可能未就绪，或输入没有送达终端")
    }

    /** 一次完整的滑动手势：towardHistory 时手指下滑把视图滚回历史，否则上滑回到底部。 */
    private fun scrollTerminal(device: UiDevice, towardHistory: Boolean) {
        val centerX = device.displayWidth / 2
        val lowerY = (device.displayHeight * 0.7).toInt()
        val upperY = (device.displayHeight * 0.3).toInt()
        if (towardHistory) {
            device.swipe(centerX, upperY, centerX, lowerY, SWIPE_STEPS)
        } else {
            device.swipe(centerX, lowerY, centerX, upperY, SWIPE_STEPS)
        }
    }

    /**
     * 等滑动帧落地后再读帧统计：waitForIdle 只等事件队列，渲染帧在其后的
     * vsync 上产出；连续滑动是大渲染负载，多等两轮重试窗口。
     */
    private fun readFrameStatsAfterRender(device: UiDevice): FrameStats {
        repeat(SCROLL_FRAME_READ_ATTEMPT_COUNT) { attempt ->
            val stats = readFrameStats(device)
            if (stats.frames > 0) return stats
            if (attempt < SCROLL_FRAME_READ_ATTEMPT_COUNT - 1) {
                SystemClock.sleep(SCROLL_FRAME_READ_RETRY_MILLIS)
            }
        }
        return readFrameStats(device)
    }

    private fun elapsedMillis(block: () -> Unit): Long {
        val start = SystemClock.elapsedRealtime()
        block()
        return SystemClock.elapsedRealtime() - start
    }
}
