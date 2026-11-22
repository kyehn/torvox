package terminal.emulator.benchmark

import android.os.SystemClock
import androidx.test.uiautomator.UiDevice

/*
 * 原生渲染器的呈现帧统计（logcat 通道）。
 *
 * 终端内容由原生渲染线程直绘 SurfaceView 的 Surface，不经过主线程 `ViewRootImpl`
 * 遍历：`dumpsys gfxinfo` 记的是 HWUI 的帧，终端滚动一帧也不计（实测 `seq 500`
 * 输出 500 行，gfxinfo 帧数只从 20 涨到 21，滚动期间更是纹丝不动）。原生侧按
 * 500ms 窗口汇总呈现帧数并写 logcat（`render_frame: presented N frames`），宏基准
 * 据此测量终端真正呈现的帧。
 *
 * 汇总行的计数覆盖「上次汇总到本次汇总」之间的呈现帧，故一行是一个窗口的呈现
 * 数而非累计数；窗口边界由原生侧的 500ms 限频闸门决定（`native/src/slow_log.rs`）。
 */

/**
 * 呈现汇总的窗口长度：原生侧每 500ms 交出一次汇总（`native/src/slow_log.rs` 的
 * 限频闸门）。手势结束时尚未交出的那一窗要等下一次呈现才补交，故按此宽度把
 * 末尾的窗口一并计入。
 */
internal const val PRESENT_SUMMARY_INTERVAL_MILLIS = 500L

/**
 * 呈现汇总行的匹配：正文形如 `render_frame: presented 42 frames (last: ...)`。
 *
 * 空白用 `.` 代替：该正则既作设备侧 `-e` 过滤词（不经 shell，按空白切词），
 * 又作本地复核的判据，两边必须同源。ERE 与 Java 正则对 `.` 的解释一致。
 */
private val PRESENTED_SUMMARY_PATTERN = Regex("""render_frame:.presented.([0-9]+).frames""")

/** 一条呈现汇总：logcat 行的时刻（epoch 毫秒）与该窗口内呈现的帧数。 */
internal data class PresentedFrameSummary(val epochMillis: Long, val frames: Int)

/** 解析一条 logcat 记录里的呈现汇总；不是呈现汇总则返回 null。 */
internal fun parsePresentedFrameSummary(record: LogcatRecord): PresentedFrameSummary? {
    val match = PRESENTED_SUMMARY_PATTERN.find(record.message) ?: return null
    val frames = match.groupValues[1].toIntOrNull() ?: return null
    return PresentedFrameSummary(epochMillis = record.epochMillis, frames = frames)
}

/** 读出 logcat 里自 [sinceEpochMillis] 起的呈现汇总。 */
internal fun readPresentedFrameSummaries(device: UiDevice, sinceEpochMillis: Long): List<PresentedFrameSummary> =
    readLogcatSince(device, sinceEpochMillis, PRESENTED_SUMMARY_PATTERN)
        .mapNotNull { parsePresentedFrameSummary(it) }

/** 呈现静默的判定时长：这么久没有新的呈现汇总即认为渲染已停。 */
private const val PRESENT_SILENCE_MILLIS = 1_500L

/** 静默判定的轮询间隔。 */
private const val PRESENT_POLL_MILLIS = 300L

/** 等渲染停摆的上限：超时说明输出或滚动没有真正发生。 */
private const val PRESENT_SETTLE_TIMEOUT_MILLIS = 30_000L

/**
 * 等到终端不再呈现新帧，返回窗口内的呈现汇总。
 *
 * 呈现汇总是原生侧 500ms 窗口的产物，故静默判定取三个窗口：窗口刚交出时还有一窗
 * 未交出，短于此会把「仍在渲染」误判成「渲染完毕」。拖动的惯性也会在这段时间里
 * 继续出帧，等它停摆才能把整个滚动的呈现帧都收进来。
 */
internal fun awaitPresentedSettle(device: UiDevice, sinceEpochMillis: Long): List<PresentedFrameSummary> {
    val deadline = SystemClock.elapsedRealtime() + PRESENT_SETTLE_TIMEOUT_MILLIS
    var lastPresentedMillis = sinceEpochMillis
    var presentedAny = false
    val summaries = mutableListOf<PresentedFrameSummary>()
    while (SystemClock.elapsedRealtime() < deadline) {
        // `-T` 是闭区间：起点取上一条汇总的时刻加一毫秒，否则同一条会被反复
        // 读出，误判成「没有新汇总」。
        val newest = readPresentedFrameSummaries(device, lastPresentedMillis + 1)
        if (newest.isNotEmpty()) {
            summaries += newest
            lastPresentedMillis = newest.last().epochMillis
            presentedAny = true
        } else if (
            presentedAny &&
            System.currentTimeMillis() - lastPresentedMillis >= PRESENT_SILENCE_MILLIS
        ) {
            return summaries
        }
        SystemClock.sleep(PRESENT_POLL_MILLIS)
    }
    error(
        "呈现未停摆（${PRESENT_SILENCE_MILLIS}ms 内没有新的呈现）：" +
            "shell 可能未就绪，输入没有送达终端，或滚动没有真正发生",
    )
}
