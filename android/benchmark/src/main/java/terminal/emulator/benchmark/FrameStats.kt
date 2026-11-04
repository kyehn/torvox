package terminal.emulator.benchmark

import android.os.SystemClock
import android.util.Log
import androidx.test.uiautomator.UiDevice

/** 0 帧时的重试次数与间隔：覆盖软件渲染的一个 vsync 帧时间。 */
private const val FRAME_READ_ATTEMPT_COUNT = 3

private const val FRAME_READ_RETRY_MILLIS = 300L

/**
 * 一次交互窗口内被测应用的帧统计，解析自 `dumpsys gfxinfo <pkg>` 的聚合输出。
 *
 * 比 `FrameTimingMetric` 多一层用途：Metric 的百分位只进 instrumentation 结果
 * bundle（Studio/IDE 可见，CI 控制台看不到），而 [FrameStats] 来自 shell 输出，
 * 可以 `println` 进设备 logcat（连接测试按用例把 logcat 落进构建产物），让 CI
 * 拿到实测帧耗时做基准校准，也可以直接对字段断言拦住卡顿回归。
 */
internal data class FrameStats(
    /** 清零以来渲染的帧总数。 */
    val frames: Int,
    /** 被判定为 jank 的帧数。 */
    val jankyFrames: Int,
    /** jank 帧占比（百分数）。 */
    val jankyRatioPercent: Double,
    /** 帧耗时百分位（毫秒）。 */
    val p50Millis: Long,
    val p90Millis: Long,
    val p95Millis: Long,
    val p99Millis: Long,
) {
    companion object {
        /** 读不到任何统计时的零值：frames=0 交给帧数断言判失败。 */
        val ZERO = FrameStats(0, 0, 0.0, 0, 0, 0, 0)
    }
}

/** `dumpsys gfxinfo` 的行格式（API 35 模拟器实测）。 */
private val FRAME_COUNT_PATTERN = Regex("""Total frames rendered: (\d+)""")

private val JANKY_PATTERN = Regex("""Janky frames: (\d+) \(([\d.]+)%\)""")

/** 50/90/95/99 百分位行；缺失按 0 处理，交给断言与校准流程甄别。 */
private val PERCENTILE_PATTERNS =
    listOf(50, 90, 95, 99).associateWith { percentile ->
        Regex("""${percentile}th percentile: (\d+)ms""")
    }

/**
 * 解析 gfxinfo 输出；没有帧计数行时返回 null——那说明输出根本不是被测应用的
 * 帧统计（进程名打错、应用已被杀死），调用方按零帧处理交给断言判失败。
 */
internal fun parseFrameStats(output: String): FrameStats? {
    val frames =
        FRAME_COUNT_PATTERN.find(output)?.groupValues?.get(1)?.toIntOrNull() ?: return null
    val janky = JANKY_PATTERN.find(output)?.groupValues
    val percentiles =
        PERCENTILE_PATTERNS.mapValues { (_, pattern) ->
            pattern.find(output)?.groupValues?.get(1)?.toLongOrNull() ?: 0L
        }
    return FrameStats(
        frames = frames,
        jankyFrames = janky?.get(1)?.toIntOrNull() ?: 0,
        jankyRatioPercent = janky?.get(2)?.toDoubleOrNull() ?: 0.0,
        p50Millis = percentiles.getValue(50),
        p90Millis = percentiles.getValue(90),
        p95Millis = percentiles.getValue(95),
        p99Millis = percentiles.getValue(99),
    )
}

/**
 * 把实测值打进测试输出：`println` 进 instrumentation 输出流（logcat 的
 * `System.out` 标签），`Log.i("Benchmark")` 进 logcat（与 androidx 基准库同 tag，
 * 本地调试 `adb logcat -s Benchmark` 直达）。连接测试任务按用例把设备 logcat 落进
 * 构建产物，故实测值在 CI 里可复核。
 *
 * 这是「实际测试获得基准值」的采集通道：断言预算必须先有实测，再按实测收紧。
 */
internal fun printMeasured(label: String, elapsedMillis: Long, stats: FrameStats?) {
    val line = buildString {
        append(measuredLinePrefix(label, elapsedMillis))
        if (stats != null) {
            append(" frames=")
            append(stats.frames)
            append(" janky=")
            append(stats.jankyRatioPercent)
            append("% p50=")
            append(stats.p50Millis)
            append("ms p90=")
            append(stats.p90Millis)
            append("ms p95=")
            append(stats.p95Millis)
            append("ms p99=")
            append(stats.p99Millis)
            append("ms")
        }
    }
    emitMeasured(line)
}

/** 实测行的公共前缀：统一标签与耗时字段，CI 按 `[bench-measure]` grep。 */
internal fun measuredLinePrefix(label: String, elapsedMillis: Long): String =
    "[bench-measure] $label elapsed=${elapsedMillis}ms"

/** 实测行的输出通道：`println` 进测试输出流，`Log.i` 进 logcat。 */
internal fun emitMeasured(line: String) {
    println(line)
    Log.i("Benchmark", line)
}

/** 清零被测应用的帧计数，让随后的读数只覆盖测量窗口。 */
internal fun resetFrameStats(device: UiDevice) {
    device.executeShellCommand("dumpsys gfxinfo $TARGET_PACKAGE reset")
}

/** 读取被测应用自上次清零以来的帧统计；读不到时按零值处理，交给断言判失败。 */
internal fun readFrameStats(device: UiDevice): FrameStats =
    parseFrameStats(device.executeShellCommand("dumpsys gfxinfo $TARGET_PACKAGE"))
        ?: FrameStats.ZERO

/**
 * 等动画帧落地后再读帧统计。
 *
 * [UiDevice.waitForIdle] 只等输入/事件队列空闲，而交互反馈帧由 Choreographer 在
 * 其后的 vsync 上产出；`dumpsys` 的 shell 往返快于首帧落地时（软件渲染模拟器一帧
 * 可达数百毫秒），真实跑过的动画会被记成 0 帧——CI 实测同一用例上一轮通过、
 * 下一轮 frames=0，正是这个竞态。0 帧时按 vsync 周期短暂重读；重试耗尽仍为 0
 * 才交给断言判失败（动画真的没跑时依然拦得住）。
 */
internal fun readFrameStatsAfterRender(device: UiDevice): FrameStats {
    repeat(FRAME_READ_ATTEMPT_COUNT) { attempt ->
        val stats = readFrameStats(device)
        if (stats.frames > 0) return stats
        if (attempt < FRAME_READ_ATTEMPT_COUNT - 1) SystemClock.sleep(FRAME_READ_RETRY_MILLIS)
    }
    return readFrameStats(device)
}
