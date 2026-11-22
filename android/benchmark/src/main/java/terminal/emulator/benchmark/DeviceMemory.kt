package terminal.emulator.benchmark

import androidx.test.uiautomator.UiDevice

/*
 * 被测进程的内存占用：`dumpsys meminfo <包名>` 的 App Summary 汇总行。
 *
 * 内存是「资源有界」类场景（稳态空载、持续输出、突发输出）的指标面：它与帧面互补——
 * 帧面看有没有在画，内存看画完（或不画）之后占住多少。
 */

/** 被测进程的内存占用（千字节）。 */
internal data class DeviceMemory(
    /** 总 PSS（千字节）：进程实际占用的物理内存，共享页按份分摊。 */
    val pssKilobytes: Int,
    /** 总 RSS（千字节）：常驻集，含共享页。 */
    val rssKilobytes: Int,
)

/**
 * 汇总行的格式：`TOTAL PSS: 59520            TOTAL RSS:   163660       TOTAL SWAP PSS:      338`。
 *
 * 只认 App Summary 段的这两项：表体里的 `TOTAL` 行是分类汇总，数字相同但格式不同，
 * 两处都取免得口径打架。
 */
private val MEMORY_TOTAL_ROW = Regex("""TOTAL PSS:\s*(\d+)\s+TOTAL RSS:\s*(\d+)""")

/**
 * 解析 `dumpsys meminfo` 的汇总行。
 *
 * 汇总行缺失即判失败：读不到内存数字时继续测下去，落盘的只是一份缺项的记录。
 */
internal fun parseDeviceMemory(output: String): DeviceMemory {
    val row = output.lineSequence()
        .map { it.trim() }
        .firstOrNull { MEMORY_TOTAL_ROW.containsMatchIn(it) }
        ?: error("内存 dump 里没有汇总行：$output")
    val match = MEMORY_TOTAL_ROW.find(row) ?: error("内存汇总行格式不对：$row")
    return DeviceMemory(
        pssKilobytes = match.groupValues[1].toInt(),
        rssKilobytes = match.groupValues[2].toInt(),
    )
}

/** 读被测包的内存占用。 */
internal fun readDeviceMemory(device: UiDevice, packageName: String): DeviceMemory =
    parseDeviceMemory(device.executeShellCommand("dumpsys meminfo $packageName"))
