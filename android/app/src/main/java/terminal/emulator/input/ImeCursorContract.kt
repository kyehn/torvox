package terminal.emulator.input

/**
 * `beginBatchEdit` / `endBatchEdit` 的嵌套计数。
 *
 * 本文件承载 IME 编辑契约的纯状态：批量编辑嵌套与选区折算。两者都刻意
 * 与 `InputConnection` 实例分离——它们是被输入法驱动、却不依赖 Android
 * 生命周期的一层状态机，抽出后可在纯 JVM 单测里覆盖，不必拉起 Surface
 * 走完整触摸与渲染链路。
 *
 * 组字光标锚点回报（`requestCursorUpdates`）刻意不在此：其投递面
 * `InputConnection.updateCursorAnchorInfo` 是框架 `@hide` 方法，
 * compileSdk 37 的公共 stub 里不存在（仅见于 `InputMethodManager`
 * 等 IME 侧类），故不做无法编译的假实现。
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
