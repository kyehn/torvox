package terminal.emulator.benchmark

import androidx.test.uiautomator.UiDevice

/*
 * 被测进程的 CPU 时间：跨应用启动对比的宿主无关工作量口径。
 *
 * 墙钟耗时是宿主吞吐的函数：宿主被并发任务抢占时，冷启动十轮中位数从 2.0s 涨到 3.5s。
 * 进程自身的 CPU 时间不受此影响——工作照样做完，只是被拉长。故启动对比 MUST 同时记
 * 两者：墙钟耗时是用户感受，CPU 时间是应用自身的工作量，宿主抢占时前者涨、后者平。
 *
 * 取值来自 `/proc/<pid>/stat` 的 utime/stime（进程自创建起的累计 CPU 时间，单位是时钟
 * 滴答）与 starttime（进程创建时刻，同样以滴答计），配合 `/proc/uptime` 得进程年龄。
 * 冷启动时进程刚被创建，累计 CPU 时间即启动这段工作的工作量。
 *
 * 滴答频率取 100 Hz：Linux 的 `/proc` 计时用 USER_HZ，Android 上恒为 100。
 */

/** USER_HZ：Linux 的 `/proc` 计时单位，Android 上恒为 100。 */
private const val MILLIS_PER_TICK = 10L

/** `/proc/<pid>/stat` 里 comm 之后的字段序号（0 起）：utime、stime、starttime。 */
private const val PROCESS_UTIME_INDEX = 11

private const val PROCESS_STIME_INDEX = 12

private const val PROCESS_STARTTIME_INDEX = 19

/** 进程的 CPU 时间与年龄。 */
internal data class ProcessCpuTime(
    /** 进程自创建起的用户态 CPU 时间（毫秒）。 */
    val userMillis: Long,
    /** 进程自创建起的内核态 CPU 时间（毫秒）。 */
    val systemMillis: Long,
    /** 进程自创建起经过的墙钟时间（毫秒）。 */
    val ageMillis: Long,
) {
    /** 进程自创建起的 CPU 时间合计（毫秒）。 */
    val totalMillis: Long
        get() = userMillis + systemMillis
}

/**
 * 解析 `/proc/<pid>/stat`。
 *
 * 第二列是进程名（含空格与括号），故从最后一个右括号之后取字段：字段序号以右括号
 * 之后的第一个字段（状态）为 0。
 */
internal fun parseProcessCpuTime(output: String, uptimeMillis: Long): ProcessCpuTime {
    val closingParenthesis = output.lastIndexOf(')')
    if (closingParenthesis < 0) error("进程 stat 行格式不对：$output")
    val fields = output.substring(closingParenthesis + 1).trim().split(WHITESPACE)
    if (fields.size <= PROCESS_STARTTIME_INDEX) error("进程 stat 行字段不足：$output")
    val startTimeMillis = fields[PROCESS_STARTTIME_INDEX].toLong() * MILLIS_PER_TICK
    return ProcessCpuTime(
        userMillis = fields[PROCESS_UTIME_INDEX].toLong() * MILLIS_PER_TICK,
        systemMillis = fields[PROCESS_STIME_INDEX].toLong() * MILLIS_PER_TICK,
        ageMillis = uptimeMillis - startTimeMillis,
    )
}

/** 读设备的开机时长（毫秒）。 */
internal fun readUptimeMillis(device: UiDevice): Long {
    val output = device.executeShellCommand("cat /proc/uptime").trim()
    val seconds = output.substringBefore(' ').toDoubleOrNull() ?: error("读不到 /proc/uptime：$output")
    return (seconds * 1000).toLong()
}

/**
 * 读被测包的进程 CPU 时间。
 *
 * 取该包的主进程：冷启动前该包没有进程，取到进程即本次启动拉起的那个。
 */
internal fun readProcessCpuTime(device: UiDevice, packageName: String): ProcessCpuTime {
    val pid = device.executeShellCommand("pidof $packageName").trim().split(WHITESPACE).firstOrNull()
    if (pid.isNullOrEmpty()) error("被测包没有进程：$packageName")
    return parseProcessCpuTime(
        device.executeShellCommand("cat /proc/$pid/stat"),
        readUptimeMillis(device),
    )
}
