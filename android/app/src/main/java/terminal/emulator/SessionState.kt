package terminal.emulator

import terminal.emulator.input.ModifierState

/** 会话抽屉条目：序号按列表位置渲染，[title] 为该会话终端标题（OSC 0/2）。 */
data class SessionInfo(
    val sessionId: Long,
    val title: String = "",
    /**
     * 该会话当前的工作目录（OSC 7 已解析为路径），未知时为空串。
     *
     * 与 [title] 分开存：很多 shell 并不设置标题，那时目录才是唯一能区分会话的信息。
     */
    val workingDirectory: String = "",
)

/** 非强制会话元数据刷新节流窗口；抽屉打开、关闭会话的强制刷新不受此限。 */
internal const val SESSION_META_REFRESH_THROTTLE_MS = 2000L

data class TerminalState(
    val isRunning: Boolean = false,
    val title: String = "Terminal",
    val selection: SelectionState = SelectionState(),
    val ctrlState: ModifierState = ModifierState.Off,
    val altState: ModifierState = ModifierState.Off,
    val scrollActive: Boolean = false,
    val sessions: List<SessionInfo> = emptyList(),
    val activeSessionId: Long = 0L,

    val selectionAccent: Int = 0,
    // 每次程序化滚动复位（由输入驱动的贴底）时递增；TerminalScreen 观察它
    // 并重同步 Surface 的本地选区计算偏移。
    val scrollEpoch: Long = 0L,
)

internal fun shouldCreateDefaultSession(
    surfaceValid: Boolean,
    surfaceWidth: Int,
    surfaceHeight: Int,
    uiSessions: List<SessionInfo>,
    runtimeSessionIds: List<Long>,
): Boolean = surfaceValid &&
    surfaceWidth > 0 &&
    surfaceHeight > 0 &&
    uiSessions.isEmpty() &&
    runtimeSessionIds.isEmpty()

/**
 * 外部 Failsafe Intent 的处置。
 *
 * 三种情形：
 *
 * - 应用未启动（无会话）：只置标记。首个会话由 `runtime.start()` 建出，它读到标记
 *   后即以 `/system/bin/sh` 启动——不必另建一个会话。
 * - 应用已启动且 Intent 是新投递的：置标记**并**建一个新会话。`runtime.start()` 会因
 *   「已有会话」早退，只置标记等于点了快捷方式毫无反应。
 * - Activity 重建时重复投递的同一个启动 Intent（已有会话）：完全忽略。既不建会话
 *   （否则每次配置变更都多出一个应急会话），也不置标记（标记会滞留到下一次任意
 *   建会话，把无关的普通会话变成应急会话）。
 */
internal enum class FailsafeIntentAction {
    /** 忽略：重复投递的旧 Intent。 */
    IGNORE,

    /** 只置标记：首个会话由 `runtime.start()` 以 `/system/bin/sh` 建出。 */
    MARK_ONLY,

    /** 置标记并建一个新会话：应用已在运行。 */
    MARK_AND_CREATE,
}

/**
 * Failsafe Intent 的处置决策，纯函数以便表驱动测试。
 *
 * @param hasSession 运行期是否已有会话
 * @param freshIntent 本次 Intent 是否为新投递（Activity 重建会重复投递旧 Intent）
 */
internal fun decideFailsafeIntent(hasSession: Boolean, freshIntent: Boolean): FailsafeIntentAction = when {
    !freshIntent && hasSession -> FailsafeIntentAction.IGNORE
    hasSession -> FailsafeIntentAction.MARK_AND_CREATE
    else -> FailsafeIntentAction.MARK_ONLY
}
