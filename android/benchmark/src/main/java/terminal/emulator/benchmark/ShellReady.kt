package terminal.emulator.benchmark

import android.os.SystemClock
import androidx.test.uiautomator.UiDevice

/*
 * shell 就绪探针：等终端真的能吃键。
 *
 * PTY 建好不等于 shell 在读：shell 初始化 tty 时会冲掉先到的输入，那时敲的命令会
 * 静默消失，回滚区永远填不满、输出负载也永远跑不起来。shell 没有提示符可等，故用
 * 探针命令诱发一次输出作就绪信号——输出到达即证明 shell 已经在读输入。探针是幂等的
 * （`echo`），重发无副作用。
 */

/**
 * 等 shell 就绪的上限。
 *
 * 模拟器上 shell 首次 PTY 输出实测可达 44s（2026-11-12 logcat：19:28:55 spawn，
 * 19:29:40 首输出），30s 会在 shell 还没开口时就误判失败；90s 留出 CI 慢宿主余量。
 */
internal const val SHELL_READY_TIMEOUT_MILLIS = 90_000L

/** 探针的重发间隔：shell 启动时可能冲掉早到的输入，重发即可命中。 */
internal const val SHELL_READY_PROBE_INTERVAL_MILLIS = 3_000L

/** 探针的重发间隔内的轮询步长。 */
private const val SHELL_READY_POLL_MILLIS = 300L

/**
 * shell 就绪探针：shell 回显它，输出到达即证明 shell 已经在读输入。
 *
 * `/system/bin/sh` 在 PTY 里不回显提示符（实测首启没有任何 PTY 输出），故
 * 「shell 开口」只能由探针诱发，不能等提示符。`%s` 是 `input text` 的空格转义。
 */
internal const val SHELL_READY_PROBE = "echo%s__terminal_shell_ready__"

/**
 * shell 首个 PTY 输出的日志正文片段：据此判定「可以往终端敲键」。
 *
 * 空白用 `.` 代替：该正则既作设备侧 `-e` 过滤词（不经 shell，按空白切词），
 * 又作本地复核的判据（详见 [readLogcatSince]）。
 */
private val SHELL_READY_PATTERN = Regex("""first.output.received""")

/**
 * 等 shell 真的能吃键：反复发探针，直到它的回显到达。
 *
 * [sinceMillis] MUST 取应用启动之前的时刻：就绪信号是会话的首个输出，那条记录写在
 * 会话启动之后，取晚了会把它滤掉，探针再怎么重发也不会再出现第二条首输出记录。
 */
internal fun awaitShellReady(device: UiDevice, sinceMillis: Long) {
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
        SystemClock.sleep(SHELL_READY_POLL_MILLIS)
    }
    error("shell 未就绪：${SHELL_READY_TIMEOUT_MILLIS}ms 内没有收到 shell 的任何输出")
}
