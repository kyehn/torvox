package terminal.emulator.benchmark

import android.os.SystemClock
import androidx.test.uiautomator.UiDevice

/** 输出类场景负载完成判定的结果。 */
internal data class OutputLoadOutcome(
    /** 完成标记（每批/每次一条）。 */
    val markers: List<LogcatRecord>,
    /** 负载进行中采到的 PSS 峰值（千字节）。 */
    val peakPssKilobytes: Long,
    /** 负载进行中采到的 RSS 峰值（千字节）。 */
    val peakRssKilobytes: Long,
    /** 最后一个完成标记的时刻（epoch 毫秒）；没有标记时为 [loadStartMillis]。 */
    val completedAtMillis: Long,
) {
    val completed: Boolean get() = markers.isNotEmpty()
}

/**
 * 等负载跑完：按 [markerPattern] 收集完成标记，标记数达到 [expectedMarkerCount] 即负载结束。
 *
 * 顺带按 [memorySampleIntervalMillis] 采内存：峰值只能在这段里取——负载结束后内存可能回落。
 * 等待上限取 min([timeoutMillis], 预算剩余)；超时即返回当前已收集的标记，由调用方判定。
 */
internal fun awaitOutputLoadCompletion(
    device: UiDevice,
    packageName: String,
    loadStartMillis: Long,
    markerPattern: Regex,
    expectedMarkerCount: Int,
    timeoutMillis: Long,
    pollMillis: Long,
    memorySampleIntervalMillis: Long,
    budget: ScenarioBudget? = null,
): OutputLoadOutcome {
    val effectiveTimeout = if (budget != null) minOf(timeoutMillis, budget.remainingMillis()) else timeoutMillis
    val deadline = SystemClock.elapsedRealtime() + effectiveTimeout.coerceAtLeast(0L)
    var markers = emptyList<LogcatRecord>()
    var peakPssKilobytes = 0L
    var peakRssKilobytes = 0L
    var nextMemorySampleElapsedMillis = 0L
    while (SystemClock.elapsedRealtime() < deadline) {
        markers = readLogcatSince(device, loadStartMillis, markerPattern)
        if (markers.size >= expectedMarkerCount) break
        val now = SystemClock.elapsedRealtime()
        if (now >= nextMemorySampleElapsedMillis) {
            val memory = readDeviceMemory(device, packageName)
            peakPssKilobytes = maxOf(peakPssKilobytes, memory.pssKilobytes.toLong())
            peakRssKilobytes = maxOf(peakRssKilobytes, memory.rssKilobytes.toLong())
            nextMemorySampleElapsedMillis = now + memorySampleIntervalMillis
        }
        SystemClock.sleep(pollMillis)
    }
    val completedAtMillis = markers.lastOrNull()?.epochMillis ?: loadStartMillis
    budget?.checkDeadline()
    return OutputLoadOutcome(markers, peakPssKilobytes, peakRssKilobytes, completedAtMillis)
}

/** S6 回滚行数：从 logcat 读回滚行数标记。 */
internal fun readScrollbackRows(device: UiDevice, packageName: String, sinceMillis: Long, pattern: Regex): Int {
    val records = readLogcatSince(device, sinceMillis, pattern)
    return records.lastOrNull()?.message?.filter { it.isDigit() }?.toIntOrNull() ?: 0
}
