package terminal.emulator.benchmark

import android.graphics.Rect
import android.os.SystemClock
import androidx.benchmark.macro.FrameTimingMetric
import androidx.benchmark.macro.junit4.MacrobenchmarkRule
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.uiautomator.UiDevice
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith

private const val SCROLL_ITERATION_COUNT = 3

/** 拖动手势的时长：连续拖动才持续出帧，瞬时滑动只有一帧 fling。 */
private const val DRAG_DURATION_MILLIS = 1_500L

/** 呈现静默的判定时长：这么久没有新的呈现汇总即认为渲染已停。 */
private const val PRESENT_SILENCE_MILLIS = 1_500L

/** 静默判定的轮询间隔。 */
private const val PRESENT_POLL_MILLIS = 300L

/** 等渲染停摆的上限：超时说明输出或滚动没有真正发生。 */
private const val PRESENT_SETTLE_TIMEOUT_MILLIS = 30_000L

/**
 * 等 shell 就绪的上限。
 *
 * 模拟器上 shell 首次 PTY 输出实测可达 44s（2026-11-12 logcat：19:28:55 spawn，
 * 19:29:40 首输出），30s 会在 shell 还没开口时就误判失败；90s 留出 CI 慢宿主余量。
 */
private const val SHELL_READY_TIMEOUT_MILLIS = 90_000L

/** 探针的重发间隔：shell 启动时可能冲掉早到的输入，重发即可命中。 */
private const val SHELL_READY_PROBE_INTERVAL_MILLIS = 3_000L

/**
 * shell 就绪探针：shell 回显它，输出到达即证明 shell 已经在读输入。
 *
 * `/system/bin/sh` 在 PTY 里不回显提示符（实测首启没有任何 PTY 输出），故
 * 「shell 开口」只能由探针诱发，不能等提示符。`%s` 是 `input text` 的空格转义。
 */
private const val SHELL_READY_PROBE = "echo%s__terminal_shell_ready__"

/**
 * shell 首个 PTY 输出的日志正文片段：宏基准据此判定「可以往终端敲键」。
 *
 * 空白用 `.` 代替：该正则既作设备侧 `-e` 过滤词（不经 shell，按空白切词），
 * 又作本地复核的判据（详见 [readLogcatSince]）。
 */
private val SHELL_READY_PATTERN = Regex("""first.output.received""")

/** 填满回滚区的命令：500 行足够把视口顶出历史区。 */
private const val SCROLLBACK_COMMAND = "seq%s500"

/**
 * 滚动的帧级宏基准：在回滚区被填满的状态下拖向历史、再拖回底部各一次。
 *
 * TESTING 性能条款把「上滑/下滑正确移动终端，无卡顿」列为覆盖范围，app 侧已有
 * 行为量化测试钉正确性，这里补宏基准侧的流畅度与耗时。内容生成走真实输入链路：
 * `input text` 把 `seq 500` 敲进 shell（应用首启自动创建默认会话），随后拖动
 * 终端内容本身。
 *
 * 帧的来源是原生渲染器的呈现汇总而非 `dumpsys gfxinfo`：终端内容由原生渲染线程
 * 直绘 SurfaceView，不经过主线程 `ViewRootImpl` 遍历，gfxinfo 记不到这些帧
 * （实测滚动全程 0 帧，见 [PresentedFrames]）。`FrameTimingMetric` 仍留在指标列表
 * 里，它记的是 Compose 层的帧。
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
                val startedMillis = System.currentTimeMillis()
                startActivityAndWait(benchmarkLaunchIntent())
                val terminal =
                    requireElement(device, TERMINAL_SCREEN_RESOURCE_ID, "终端界面")
                awaitShellReady(device, startedMillis)
                fillScrollback(device)
                val scrollStartMillis = System.currentTimeMillis()
                val elapsed = elapsedMillis {
                    dragTerminal(device, terminal.visibleBounds, towardHistory = true)
                    dragTerminal(device, terminal.visibleBounds, towardHistory = false)
                }
                awaitPresentedSettle(device, scrollStartMillis)
                val summaries = readPresentedFrameSummaries(device, scrollStartMillis)
                val presentedFrames = summaries.sumOf { it.frames }
                // 实测值先于断言输出：断言失败时构建日志里仍留得下这轮的数字，
                // 否则门禁只报「低于下限」，排障还得重跑一遍。
                emitMeasured(
                    measuredLinePrefix("scroll", elapsed) +
                        " presentedFrames=$presentedFrames presentedWindows=${summaries.size}",
                )
                check(summaries.isNotEmpty()) {
                    "滚动未产生呈现帧：原生渲染器在滚动窗口内没有汇报任何呈现"
                }
                check(summaries.size >= MIN_SCROLL_PRESENTED_WINDOWS) {
                    "滚动只在 ${summaries.size} 个窗口里出帧，低于下限 $MIN_SCROLL_PRESENTED_WINDOWS：" +
                        "滚动中途停摆"
                }
                check(presentedFrames >= MIN_SCROLL_PRESENTED_FRAMES) {
                    "滚动呈现帧数 $presentedFrames 低于下限 $MIN_SCROLL_PRESENTED_FRAMES"
                }
                check(elapsed <= SCROLL_BUDGET_MILLIS) {
                    "上滑下滑耗时 ${elapsed}ms 超过预算 ${SCROLL_BUDGET_MILLIS}ms"
                }
            },
        )
    }

    /**
     * 等 shell 真的能吃键：反复发探针，直到它的回显到达。
     *
     * PTY 建好不等于 shell 在读：shell 初始化 tty 时会冲掉先到的输入，那时敲的
     * 命令会静默消失，回滚区永远填不满。shell 没有提示符可等，故用探针命令诱发
     * 一次输出——输出到达即证明 shell 已经在读输入。探针是幂等的（`echo`），重发
     * 无副作用。
     */
    private fun awaitShellReady(device: UiDevice, sinceMillis: Long) {
        check(isImeShown(device)) { "输入法未弹出，终端没有输入焦点，敲键不会送达终端" }
        val deadline = SystemClock.elapsedRealtime() + SHELL_READY_TIMEOUT_MILLIS
        var nextProbeMillis = 0L
        while (SystemClock.elapsedRealtime() < deadline) {
            if (readLogcatSince(device, sinceMillis, SHELL_READY_PATTERN).isNotEmpty()) return
            val now = SystemClock.elapsedRealtime()
            if (now >= nextProbeMillis) {
                device.executeShellCommand("input text $SHELL_READY_PROBE")
                device.executeShellCommand("input keyevent 66")
                nextProbeMillis = now + SHELL_READY_PROBE_INTERVAL_MILLIS
            }
            SystemClock.sleep(PRESENT_POLL_MILLIS)
        }
        error("shell 未就绪：${SHELL_READY_TIMEOUT_MILLIS}ms 内没有收到 shell 的任何输出")
    }

    /**
     * 敲入 `seq 500` 并等输出渲染完毕。
     *
     * `input text` 走硬件键盘事件通道，不经过输入法，终端像收到物理键盘一样把
     * 字符送进 shell（`%s` 是 `input text` 的空格转义）；回车执行后 500 行输出
     * 把回滚区填满。判定「渲染完毕」看原生侧的呈现汇总：输出期间持续出帧，输出
     * 结束后不再有新的汇总行。
     */
    private fun fillScrollback(device: UiDevice) {
        val commandMillis = System.currentTimeMillis()
        device.executeShellCommand("input text $SCROLLBACK_COMMAND")
        device.executeShellCommand("input keyevent 66")
        awaitPresentedSettle(device, commandMillis)
    }

    /**
     * 等到终端不再呈现新帧。
     *
     * 呈现汇总是原生侧 500ms 窗口的产物，故静默判定取三个窗口：窗口刚交出时
     * 还有一窗未交出，短于此会把「仍在渲染」误判成「渲染完毕」。拖动的惯性
     * 也会在这段时间里继续出帧，等它停摆才能把整个滚动的呈现帧都收进来。
     */
    private fun awaitPresentedSettle(device: UiDevice, sinceMillis: Long) {
        val deadline = SystemClock.elapsedRealtime() + PRESENT_SETTLE_TIMEOUT_MILLIS
        var lastPresentedMillis = sinceMillis
        var presentedAny = false
        while (SystemClock.elapsedRealtime() < deadline) {
            // `-T` 是闭区间：起点取上一条汇总的时刻加一毫秒，否则同一条会被反复
            // 读出，误判成「没有新汇总」。
            val newest =
                readPresentedFrameSummaries(device, lastPresentedMillis + 1)
                    .lastOrNull()
                    ?.epochMillis
            if (newest != null && newest > lastPresentedMillis) {
                lastPresentedMillis = newest
                presentedAny = true
            } else if (
                presentedAny &&
                System.currentTimeMillis() - lastPresentedMillis >= PRESENT_SILENCE_MILLIS
            ) {
                return
            }
            SystemClock.sleep(PRESENT_POLL_MILLIS)
        }
        error(
            "呈现未停摆（${PRESENT_SILENCE_MILLIS}ms 内没有新的呈现）：" +
                "shell 可能未就绪，输入没有送达终端，或滚动没有真正发生",
        )
    }

    /**
     * 一次拖动手势：手指下移把视图拖向历史，上移拖回底部。
     *
     * 用 `input swipe` 的显式时长而非瞬时滑动：瞬时滑动只产生一帧 fling，测不出
     * 连续出帧的节奏；固定时长的拖动在设备之间可比。坐标取终端界面可见范围的
     * 上半部，避开底部的修饰键栏。
     */
    private fun dragTerminal(device: UiDevice, terminalBounds: Rect, towardHistory: Boolean) {
        val centerX = terminalBounds.centerX()
        val upperY = terminalBounds.top + (terminalBounds.height() * 0.2).toInt()
        val lowerY = terminalBounds.top + (terminalBounds.height() * 0.45).toInt()
        val startY = if (towardHistory) upperY else lowerY
        val endY = if (towardHistory) lowerY else upperY
        device.executeShellCommand(
            "input swipe $centerX $startY $centerX $endY $DRAG_DURATION_MILLIS",
        )
    }
}
