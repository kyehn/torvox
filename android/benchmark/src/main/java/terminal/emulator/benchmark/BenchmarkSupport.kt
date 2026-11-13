package terminal.emulator.benchmark

import android.content.Intent
import android.os.SystemClock
import androidx.test.uiautomator.By
import androidx.test.uiautomator.UiDevice
import androidx.test.uiautomator.UiObject2
import androidx.test.uiautomator.Until

/** 被测 Activity 的类名：applicationId 是 `com.termux`，类名不在该包下。 */
internal const val TARGET_ACTIVITY = "terminal.emulator.MainActivity"

/** 与 `MainActivity.EXTRA_LOG_LEVEL` 一致；基准模块不依赖 app 模块，故本地声明。 */
private const val LOG_LEVEL_EXTRA = "terminal.emulator.log_level"

/** force-stop 后等设备空闲的上限，让软件渲染的图形栈喘口气。 */
internal const val EMULATOR_IDLE_TIMEOUT_MILLIS = 2_000L

/**
 * 宏基准启动 Intent：把被测进程的日志等级强制到 DEBUG。
 *
 * 原生呈现汇总是 debug 级，release 缺省 WARN 会把首输出标记与呈现汇总一起吞掉，
 * 基准将永远等不到 shell 就绪、也读不到任何滚动帧。extra 只改本进程等级，
 * 不写回用户设置（见 `MainActivity.applyLogLevelExtra`）。
 */
internal fun benchmarkLaunchIntent(): Intent = Intent(Intent.ACTION_MAIN).apply {
    setClassName(TARGET_PACKAGE, TARGET_ACTIVITY)
    addCategory(Intent.CATEGORY_LAUNCHER)
    addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
    putExtra(LOG_LEVEL_EXTRA, "DEBUG")
}

/** 输入法状态的等待上限：输入法可能在冷启动后过一会儿才弹起，不能立刻下结论。 */
internal const val IME_STATE_TIMEOUT_MILLIS = 5_000L

/** 输入法状态的轮询间隔（每次 shell 调用本身也有耗时）。 */
internal const val IME_STATE_POLL_MILLIS = 200L

/** 输入法已弹出的状态串（`dumpsys input_method`）。 */
internal const val IME_SHOWN_STATE = "mInputShown=true"

/**
 * 元素缺失 MUST 失败：找不到元素说明界面没到位，继续测量只会产出无意义的数字。
 */
internal fun requireElement(device: UiDevice, resourceId: String, label: String): UiObject2 {
    val element = device.wait(Until.findObject(By.res(resourceId)), UI_TIMEOUT_MILLIS)
    checkNotNull(element) { "未找到$label（$resourceId）" }
    return element
}

/**
 * 测量窗口的耗时：取单调时钟，墙上时钟被校准时不会把预算判定带偏。
 */
internal fun elapsedMillis(block: () -> Unit): Long {
    val start = SystemClock.elapsedRealtime()
    block()
    return SystemClock.elapsedRealtime() - start
}

/**
 * 软件渲染的模拟器连续跑长宏基准会耗尽图形栈，任务以输出插件 EOF 崩溃，
 * 因此每个用例前先强制停止被测应用，再等设备空闲让图形栈恢复。
 */
internal fun recoverEmulator(device: UiDevice) {
    device.executeShellCommand("am force-stop $TARGET_PACKAGE")
    device.waitForIdle(EMULATOR_IDLE_TIMEOUT_MILLIS)
}

/**
 * 输入法是否已弹出：弹出即终端拿到了输入焦点，硬件按键事件才会送达终端。
 *
 * 状态成立即刻返回；不成立则轮询到 [timeoutMillis] 为止。
 */
internal fun isImeShown(device: UiDevice, timeoutMillis: Long = IME_STATE_TIMEOUT_MILLIS): Boolean =
    waitForImeState(device, timeoutMillis) { state -> state.contains(IME_SHOWN_STATE) }

/** 输入法是否已收起：状态不成立（没有 `mInputShown=true`）即视为收起。 */
internal fun isImeHidden(device: UiDevice, timeoutMillis: Long = IME_STATE_TIMEOUT_MILLIS): Boolean =
    waitForImeState(device, timeoutMillis) { state -> !state.contains(IME_SHOWN_STATE) }

private fun waitForImeState(device: UiDevice, timeoutMillis: Long, matches: (String) -> Boolean): Boolean {
    val deadline = SystemClock.elapsedRealtime() + timeoutMillis
    while (true) {
        // 命令不经 shell（`Runtime.exec` 按空白切词），管道不生效：取回整份 dump
        // 后在本地筛出状态行。
        val state = device.executeShellCommand("dumpsys input_method")
        if (matches(state)) return true
        if (SystemClock.elapsedRealtime() >= deadline) return false
        SystemClock.sleep(IME_STATE_POLL_MILLIS)
    }
}

/** 一条 logcat 记录。 */
internal data class LogcatRecord(
    /** 记录时刻（epoch 毫秒）。 */
    val epochMillis: Long,
    /** 标签（logcat 的 tag）。 */
    val tag: String,
    /** 正文（标签之后的部分）。 */
    val message: String,
)

/**
 * logcat 记录行的格式（`-v epoch`，API 35 模拟器实测）。
 *
 * 依次是时刻、pid、tid、优先级、标签与正文。标签左对齐补空格到 8 列，故标签与
 * 冒号之间可能有空格（`Runtime : ...`）；时刻的毫秒定长三位（`%03ld`）。
 */
private val LOGCAT_RECORD_PATTERN =
    Regex("""^\s*(\d+)\.(\d{3})\s+\d+\s+\d+\s+[VDIWEFS]\s+(\S+)\s*:\s?(.*)$""")

/**
 * 设备侧过滤词必须是 POSIX ERE，且不得含空白。
 *
 * 两道约束都来自执行环境：
 *
 * 1. `UiDevice.executeShellCommand` 走 `Runtime.exec(String)`（见 AOSP
 *    `UiAutomationConnection`）：**不经 shell**，按空白切词，引号原样传进参数，
 *    管道也不生效。故命令与过滤词里都不能有空白与引号。
 * 2. logcat 的 `-e` 是 POSIX ERE：Java 正则的 `\d`/`\w` 在 ERE 里不是字符类，
 *    `\d+` 会被当成字面量 `d`，设备侧过滤把真正的记录全滤掉（实测：
 *    `session \d+ first output received` 匹配不到 `session 1 first output received`，
 *    换成 `session [0-9]+ ...` 才命中）。
 *
 * 空白用 `.` 代替：ERE 的 `.` 匹配任意字符，本地再用 Java 正则精确复核。
 */
private val LOGCAT_PATTERN_WHITESPACE = Regex("""\s""")

/** UiAutomator 把执行的 shell 命令本身以 `UiDevice` 标签记进 logcat。 */
private const val SHELL_ECHO_TAG = "UiDevice"

/** 解析 logcat 输出；非记录行（分段标记、空行）忽略。 */
internal fun parseLogcatRecords(output: String): List<LogcatRecord> = output.lineSequence().mapNotNull { line ->
    val match = LOGCAT_RECORD_PATTERN.find(line) ?: return@mapNotNull null
    val seconds = match.groupValues[1].toLongOrNull() ?: return@mapNotNull null
    val millis = match.groupValues[2].toLongOrNull() ?: return@mapNotNull null
    LogcatRecord(
        epochMillis = seconds * 1000L + millis,
        tag = match.groupValues[3],
        message = match.groupValues[4],
    )
}.toList()

/**
 * 读出 logcat 自 [sinceEpochMillis] 起、正文匹配 [pattern] 的记录。
 *
 * `-T` 由设备侧按时刻过滤（闭区间），`-e` 按正则过滤出需要的行，两者合起来
 * 只把匹配的行搬回来；`*:D` 显式放开 debug 级日志——原生渲染器的呈现汇总
 * 就是 debug 级。
 *
 * 命令回显（`UiDevice` 标签）里含过滤词本身，按标签剔除：那是 UiAutomator 记
 * 下的命令，不是被测应用的记录。
 */
internal fun readLogcatSince(device: UiDevice, sinceEpochMillis: Long, pattern: Regex): List<LogcatRecord> {
    require(sinceEpochMillis >= 0) { "logcat 起始时刻不能为负：$sinceEpochMillis" }
    require(!LOGCAT_PATTERN_WHITESPACE.containsMatchIn(pattern.pattern)) {
        "logcat 过滤词不得含空白（命令不经 shell，按空白切词）：${pattern.pattern}"
    }
    val output =
        device.executeShellCommand(
            "logcat -d -v epoch *:D -T ${formatEpochMillis(sinceEpochMillis)} -e ${pattern.pattern}",
        )
    return parseLogcatRecords(output)
        .filter { it.tag != SHELL_ECHO_TAG && pattern.containsMatchIn(it.message) }
}

/** epoch 毫秒 → logcat `-T` 认的定点时刻串（秒.毫秒，毫秒补齐三位）。 */
private fun formatEpochMillis(epochMillis: Long): String {
    val seconds = epochMillis / 1000L
    val millis = epochMillis % 1000L
    return "$seconds.${millis.toString().padStart(3, '0')}"
}
