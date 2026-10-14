package terminal.emulator.input

/**
 * `beginBatchEdit` / `endBatchEdit` 的嵌套计数。
 *
 * 本文件承载 IME 编辑契约的纯状态：批量编辑嵌套与选区折算。两者都刻意
 * 与 `InputConnection` 实例分离——它们是被输入法驱动、却不依赖 Android
 * 生命周期的一层状态机，抽出后可在纯 JVM 单测里覆盖，不必拉起 Surface
 * 走完整触摸与渲染链路。
 *
 * 组字光标锚点回报的准入状态 ([CursorUpdateMonitor]) 也在此：投递面是
 * `InputMethodManager.updateCursorAnchorInfo(View, CursorAnchorInfo)`——公开 API，
 * 其文档职责即「回报文本插入点与组字字符的位置变化」。`InputConnection` 上同名
 * 的 `updateCursorAnchorInfo` 是 `@hide`，但那条路径只是同一个 binder 调用，
 * 经 `InputMethodManager` 走完全等价，故锚点回报可实现。
 *
 * 下溢钳位到 0 而非变负：输入法给出多余的 `endBatchEdit` 时，
 * 负深度会让随后一次 `begin` 仍停留在「非批量」状态，反而错过后续整段编辑。
 */
class BatchEditDepth {
    var depth: Int = 0
        private set

    val isActive: Boolean
        get() = depth > 0

    fun begin() {
        depth++
    }

    fun end() {
        depth = (depth - 1).coerceAtLeast(0)
    }
}

/** 选区折算结果：[magnitude] 为 0 表示无需移动。 */
data class SelectionDelta(val magnitude: Int) {
    /** 正向（右）移动的码点数；0 表示不移动。 */
    fun rightward(): Int = magnitude.coerceAtLeast(0)

    /** 反向（左）移动的码点数；0 表示不移动。 */
    fun leftward(): Int = (-magnitude).coerceAtLeast(0)

    val isEmpty: Boolean
        get() = magnitude == 0
}

/**
 * `setSelection` 的绝对位置折算器。
 *
 * 输入法给出的是**绝对**下标，而终端只有相对移动（左右箭头）这一种可发送的
 * 语义；故必须记住上次回报的位置，按差值折成一次或多次箭头移动。
 *
 * 幅度按码点计并钳位到 [maxMagnitude]：输入法可能给出与终端内容毫无关系的
 * 巨大下标（它数的是自己的整段文本），不钳位会向 PTY 灌入海量退格。
 */
class SelectionMover(private val maxMagnitude: Int) {
    private var lastPosition: Int = 0

    /** 把 [position] 折算为自上次位置以来的位移；不接受时返回空位移。 */
    fun moveTo(position: Int): SelectionDelta {
        val raw = position - lastPosition
        if (raw == 0) return SelectionDelta(0)
        val step = raw.coerceIn(-maxMagnitude, maxMagnitude)
        // 即使被钳位也要推进记录位置：否则下一次折算会重复计入被丢掉的部分。
        lastPosition += step
        return SelectionDelta(step)
    }

    /** 与实际组字基线重新对齐，使下一次折算以该位置为原点。 */
    fun resetTo(position: Int) {
        lastPosition = position
    }
}

/**
 * 输入法请求的光标位置更新监视（`InputConnection.requestCursorUpdates`）。
 *
 * 为何必须是显式状态而不是「每次组字变化都回报」：回报是有成本的 IPC，而输入法
 * 只在需要把候选窗口贴到真实光标时才请求它。未请求时回报是纯浪费；请求被关闭
 * （mode 清掉 MONITOR 位）后继续回报，则把一个已经不需要的通道留着。
 */
class CursorUpdateMonitor {
    /** 输入法最近一次请求的监视模式；0 表示未请求。 */
    var requestedMode: Int = 0
        private set

    /** 是否正处于「光标/锚点变化即回报」的监视中。 */
    val isMonitoring: Boolean
        get() = (requestedMode and MONITOR) != 0

    /** 记录一次请求。重复请求是常态（输入法随焦点与设置变化重发）。 */
    fun update(mode: Int) {
        requestedMode = mode
    }

    companion object {
        /** `CURSOR_UPDATE_MONITOR`：光标或锚点变化即回报。 */
        const val MONITOR = android.view.inputmethod.InputConnection.CURSOR_UPDATE_MONITOR
    }
}
