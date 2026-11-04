package terminal.emulator.benchmark

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
