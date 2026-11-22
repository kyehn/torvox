package terminal.emulator.benchmark

import androidx.test.uiautomator.UiDevice

/*
 * 宿主负载的记录。
 *
 * 模拟器跑在宿主上，宿主被并发任务抢占时测得值整体变慢——实测冷启动十轮中位数从 2.0s
 * 涨到 3.5s、终端内容层刷新槽位比例从 0.91 降到 0.59。故每轮记录宿主负载：慢轮次
 * 旁边有抢占证据，事后即可把它与「应用慢」分开。
 *
 * 负载用宿主抢占时间（`/proc/stat` 的 steal 计数）占比表达：它正是宿主抢走 CPU 的那
 * 一部分时间，设备自身读不到宿主负载，但读得到「自己没拿到多少 CPU」。
 *
 * 只记录，不据此折算：测试进程内的定长 CPU 循环作速度参照已被实测否掉（启动是多线程
 * 且含 I/O 的延迟敏感过程，参照是单线程吞吐敏感的，二者不成比例，按参照折算会把宿主慢
 * 误判成应用慢）。折算 steal 占比同样不成立：steal 记的是全部 vCPU 的被抢时间，空闲的
 * vCPU 被抢走并不拖慢被测进程，按占比缩放会把被测进程算得比实际更快。
 */

/**
 * 宿主抢占占比的判定阈值。
 *
 * 实测（本机 AVD，4 核宿主，冷启动窗口内）：宿主安静（负载 0.3–0.9）时 steal 占比
 * 1.9%–5.2%；宿主被 3 个忙循环占用（负载 1.7–4.6）时 44%–46%；被 8 个忙循环占用
 * （负载 0.9–5.3）时 38%–43%；模拟器空闲且宿主安静时约 0.1%。
 *
 * 取 10%：安静宿主的实测上限（5.2%）的两倍，又远低于任何抢占态的实测下限（38%）。
 */
internal const val HOST_CONTENTION_STEAL_RATIO = 0.10

/** `/proc/stat` 的 cpu 汇总行：用户、nice、系统、空闲、iowait、irq、softirq、steal。 */
private val CPU_TIME_ROW =
    Regex("""^cpu\s+(\d+)\s+(\d+)\s+(\d+)\s+(\d+)\s+(\d+)\s+(\d+)\s+(\d+)\s+(\d+)""")

/** 一时刻的 CPU 时间分布（单位为 jiffies）。 */
internal data class CpuTimeSnapshot(
    /** 用户态时间。 */
    val user: Long,
    /** 低优先级用户态时间。 */
    val nice: Long,
    /** 内核态时间。 */
    val system: Long,
    /** 空闲时间。 */
    val idle: Long,
    /** iowait 时间。 */
    val iowait: Long,
    /** 中断时间。 */
    val irq: Long,
    /** 软中断时间。 */
    val softirq: Long,
    /** 宿主抢占时间：虚拟化环境下宿主把 CPU 拿走的那部分。 */
    val steal: Long,
) {
    /** 计入统计的总时间。 */
    val total: Long
        get() = user + nice + system + idle + iowait + irq + softirq + steal

    /** 设备自身忙碌的时间（不含空闲与 iowait）。 */
    val busy: Long
        get() = total - idle - iowait
}

/**
 * 解析 `/proc/stat` 的 cpu 汇总行。
 *
 * 取汇总行而非逐核行：逐核行是各 vCPU 的账，汇总行才是全体合计。汇总行末尾还有
 * guest 与 guest_nice 两列（已计入 user），取前八列即可。
 */
internal fun parseCpuTime(output: String): CpuTimeSnapshot {
    val match = output.lineSequence()
        .map { it.trim() }
        .mapNotNull { CPU_TIME_ROW.find(it) }
        .firstOrNull()
        ?: error("读不到 /proc/stat 的 cpu 汇总行")
    return CpuTimeSnapshot(
        user = match.groupValues[1].toLong(),
        nice = match.groupValues[2].toLong(),
        system = match.groupValues[3].toLong(),
        idle = match.groupValues[4].toLong(),
        iowait = match.groupValues[5].toLong(),
        irq = match.groupValues[6].toLong(),
        softirq = match.groupValues[7].toLong(),
        steal = match.groupValues[8].toLong(),
    )
}

/** 一段测量窗口内的负载。 */
internal data class HostLoad(
    /** 宿主抢占占 CPU 总时间的比例（0–1）。 */
    val stealRatio: Double,
    /** 设备自身忙碌占 CPU 总时间的比例（0–1）。 */
    val busyRatio: Double,
    /** 窗口时长（毫秒）。 */
    val windowMillis: Long,
) {
    /** 宿主抢占是否足以影响本轮测量。 */
    val contended: Boolean
        get() = stealRatio > HOST_CONTENTION_STEAL_RATIO

    /** 一行式说明：进实测行与测量行。 */
    fun describe(): String =
        "stealRatio=$stealRatio busyRatio=$busyRatio windowMillis=$windowMillis contended=$contended"
}

/** 两份 CPU 时间快照之间的负载。 */
internal fun hostLoadBetween(before: CpuTimeSnapshot, after: CpuTimeSnapshot, windowMillis: Long): HostLoad {
    val totalDelta = after.total - before.total
    if (totalDelta <= 0) {
        // 窗口太短或计数器回绕：此刻没有可比的负载数据，如实记成零负载而不是猜一个。
        return HostLoad(stealRatio = 0.0, busyRatio = 0.0, windowMillis = windowMillis)
    }
    return HostLoad(
        stealRatio = (after.steal - before.steal).toDouble() / totalDelta,
        busyRatio = (after.busy - before.busy).toDouble() / totalDelta,
        windowMillis = windowMillis,
    )
}

/** 读一时刻的 CPU 时间分布。 */
internal fun readCpuTime(device: UiDevice): CpuTimeSnapshot = parseCpuTime(device.executeShellCommand("cat /proc/stat"))
