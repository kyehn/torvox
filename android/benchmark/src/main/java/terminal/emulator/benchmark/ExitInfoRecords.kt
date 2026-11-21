package terminal.emulator.benchmark

import android.app.ApplicationExitInfo
import androidx.test.uiautomator.UiDevice
import java.text.ParsePosition
import java.text.SimpleDateFormat

/*
 * 进程退出归因（`dumpsys activity exit-info <包名>`）。
 *
 * 宏基准的场景若进程中途死了，测得的数字全部作废；但「为什么死的」决定了这算应用
 * 故障还是环境抖动。该通道给出系统侧的归因：退出原因、子原因、死亡时刻与说明。
 *
 * 时刻是墙钟本地时间（`yyyy-MM-dd HH:mm:ss.SSS`，设备时区与日历），不是单调时钟；
 * 场景起止也取墙钟，同机同时区故可直接比较。
 */

/** 一条进程退出记录。 */
internal data class ExitInfoRecord(
    /** 进程死亡时刻（epoch 毫秒）。 */
    val timestampMillis: Long,
    /** 进程号。 */
    val pid: Int,
    /** 退出原因码（`ApplicationExitInfo.REASON_*`）。 */
    val reasonCode: Int,
    /** 退出原因名（系统给出的可读名，如 `APP CRASH(EXCEPTION)`）。 */
    val reasonName: String,
    /** 退出子原因码。 */
    val subReasonCode: Int,
    /** 退出子原因名（如 `FORCE STOP`）。 */
    val subReasonName: String,
    /** 系统给出的补充说明。 */
    val description: String,
    /** 死亡前的内存占用显示值（系统侧按单位舍入，如 `0.00`、`50MB`）。 */
    val pss: String,
    /** 死亡前的常驻内存显示值。 */
    val rss: String,
) {
    /** 退出归因分类。 */
    val cause: ProcessExitCause
        get() = exitCauseOf(reasonCode)

    /** 一行式说明：用于失败信息与实测输出。 */
    fun describe(): String = "reason=$reasonName($reasonCode) cause=$cause " +
        "subreason=$subReasonName($subReasonCode) description=$description " +
        "timestamp=$timestampMillis pid=$pid pss=$pss rss=$rss"
}

/** 进程退出的归因分类。 */
internal enum class ProcessExitCause {
    /** 预期退出：用户或基准框架主动停止，或进程自行退出。 */
    EXPECTED,

    /** 应用自身故障：崩溃、原生崩溃、ANR、初始化失败、资源滥用。 */
    APPLICATION_FAULT,

    /** 环境导致：内存压力、信号、其他系统原因、包更新或状态变化。 */
    ENVIRONMENT,

    /** 原因码不在已知集合内：平台新增原因时归此类，判红并交人查。 */
    UNKNOWN,
}

/**
 * 退出原因码 → 归因分类。
 *
 * 分类依据 `ApplicationExitInfo` 的公开原因码（平台新增原因码时归 [ProcessExitCause.UNKNOWN]，
 * 不猜语义）：崩溃、ANR、初始化失败与资源滥用是应用自身的问题；内存压力、信号、包更新
 * 一类由环境决定；用户或基准框架的强停属预期。
 */
internal fun exitCauseOf(reasonCode: Int): ProcessExitCause = when (reasonCode) {
    ApplicationExitInfo.REASON_EXIT_SELF,
    ApplicationExitInfo.REASON_USER_REQUESTED,
    ApplicationExitInfo.REASON_USER_STOPPED,
    -> ProcessExitCause.EXPECTED

    ApplicationExitInfo.REASON_CRASH,
    ApplicationExitInfo.REASON_CRASH_NATIVE,
    ApplicationExitInfo.REASON_ANR,
    ApplicationExitInfo.REASON_INITIALIZATION_FAILURE,
    ApplicationExitInfo.REASON_EXCESSIVE_RESOURCE_USAGE,
    -> ProcessExitCause.APPLICATION_FAULT

    ApplicationExitInfo.REASON_SIGNALED,
    ApplicationExitInfo.REASON_LOW_MEMORY,
    ApplicationExitInfo.REASON_PERMISSION_CHANGE,
    ApplicationExitInfo.REASON_DEPENDENCY_DIED,
    ApplicationExitInfo.REASON_OTHER,
    ApplicationExitInfo.REASON_FREEZER,
    ApplicationExitInfo.REASON_PACKAGE_STATE_CHANGE,
    ApplicationExitInfo.REASON_PACKAGE_UPDATED,
    -> ProcessExitCause.ENVIRONMENT

    else -> ProcessExitCause.UNKNOWN
}

/** dump 输出的首行：确认拿到的是退出历史而不是别的输出。 */
private const val EXIT_INFO_DUMP_HEADER = "ACTIVITY MANAGER PROCESS EXIT INFO"

/** 记录块起始行。 */
private val EXIT_RECORD_HEADER = Regex("""ApplicationExitInfo #\d+:""")

/** 记录块里的时刻行。 */
private val EXIT_TIMESTAMP_ROW =
    Regex("""timestamp=(.+) pid=(\d+) realUid=\d+ packageUid=\d+ definingUid=\d+ user=\d+""")

/** 记录块里的原因行：原因名自带括号（`APP CRASH(EXCEPTION)`），故取到 ` subreason=` 为止。 */
private val EXIT_REASON_ROW =
    Regex("""process=\S+ reason=(\d+) \((.+?)\) subreason=(\d+) \((.+?)\) status=(-?\d+)""")

/** 记录块里的明细行：说明文字里可能含 `state=`，故取最后一个 ` state=` 作分隔。 */
private val EXIT_DETAIL_ROW =
    Regex("""importance=(\d+) pss=(\S+) rss=(\S+) description=(.*) state=(.*) trace=(.*)""")

/** 退出记录的时刻格式：系统侧用默认时区与默认日历打印的本地墙钟。 */
private const val EXIT_TIMESTAMP_FORMAT = "yyyy-MM-dd HH:mm:ss.SSS"

/**
 * 解析 `dumpsys activity exit-info` 的输出。
 *
 * 输出按记录分块：块首是 `ApplicationExitInfo #N`，其后三行分别是时刻、原因与明细。
 * 包没有退出历史时只有 dump 头，返回空列表。
 */
internal fun parseExitInfoRecords(output: String): List<ExitInfoRecord> {
    check(output.contains(EXIT_INFO_DUMP_HEADER)) {
        "退出归因采集失败：输出不是 dumpsys activity exit-info 的结果：" +
            output.lineSequence().firstOrNull().orEmpty()
    }
    val records = mutableListOf<ExitInfoRecord>()
    var recordRows = mutableListOf<String>()
    var insideRecord = false
    for (row in output.lineSequence()) {
        if (EXIT_RECORD_HEADER.containsMatchIn(row)) {
            if (insideRecord) records += parseExitInfoRecord(recordRows)
            insideRecord = true
            recordRows = mutableListOf()
        } else if (insideRecord) {
            recordRows += row.trim()
        }
    }
    if (insideRecord) records += parseExitInfoRecord(recordRows)
    return records
}

/** 解析一条退出记录的三行内容；字段缺失即格式漂移，当场报错而不是当作没有退出。 */
private fun parseExitInfoRecord(rows: List<String>): ExitInfoRecord {
    val timestamp = rows.firstNotNullOfOrNull { EXIT_TIMESTAMP_ROW.matchEntire(it) }
        ?: error("退出归因采集失败：进程退出记录缺少时刻行：$rows")
    val reason = rows.firstNotNullOfOrNull { EXIT_REASON_ROW.matchEntire(it) }
        ?: error("退出归因采集失败：进程退出记录缺少原因行：$rows")
    val detail = rows.firstNotNullOfOrNull { EXIT_DETAIL_ROW.matchEntire(it) }
        ?: error("退出归因采集失败：进程退出记录缺少明细行：$rows")
    return ExitInfoRecord(
        timestampMillis = parseExitTimestampMillis(timestamp.groupValues[1]),
        pid = timestamp.groupValues[2].toInt(),
        reasonCode = reason.groupValues[1].toInt(),
        reasonName = reason.groupValues[2],
        subReasonCode = reason.groupValues[3].toInt(),
        subReasonName = reason.groupValues[4],
        description = detail.groupValues[4],
        pss = detail.groupValues[2],
        rss = detail.groupValues[3],
    )
}

/** 解析退出记录的墙钟时刻（设备本地时区）。 */
private fun parseExitTimestampMillis(text: String): Long {
    val format = SimpleDateFormat(EXIT_TIMESTAMP_FORMAT).apply { isLenient = false }
    val position = ParsePosition(0)
    val instant = format.parse(text, position)
        ?: error("退出归因采集失败：进程退出记录的时刻无法解析：$text")
    check(position.index == text.length) { "退出归因采集失败：进程退出记录的时刻有多余内容：$text" }
    return instant.time
}

/** 读出设备上被测包的进程退出历史。 */
internal fun readExitInfoRecords(device: UiDevice): List<ExitInfoRecord> =
    parseExitInfoRecords(device.executeShellCommand("dumpsys activity exit-info $TARGET_PACKAGE"))

/** 落在场景时间窗内的退出记录。 */
internal fun List<ExitInfoRecord>.withinWindow(startMillis: Long, endMillis: Long): List<ExitInfoRecord> =
    filter { it.timestampMillis in startMillis..endMillis }

/**
 * 给失败补上进程退出归因：进程死了的话，原因就在退出记录里。
 *
 * 读退出记录本身失败时保留原始失败（把读失败挂成 suppressed），不因归因失败而丢掉
 * 真正的失败原因。
 */
internal fun Throwable.withProcessExitAttribution(device: UiDevice, sinceMillis: Long): Throwable {
    val exits = try {
        readExitInfoRecords(device).filter { it.timestampMillis >= sinceMillis }
    } catch (readFailure: Throwable) {
        addSuppressed(readFailure)
        return this
    }
    if (exits.isEmpty()) return this
    return IllegalStateException(
        "$message；进程退出归因：${exits.joinToString("；") { it.describe() }}",
        this,
    )
}
