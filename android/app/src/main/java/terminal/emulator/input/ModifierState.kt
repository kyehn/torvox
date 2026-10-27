package terminal.emulator.input

enum class ModifierState { Off, Once, Locked }

fun ModifierState.next(): ModifierState = when (this) {
    ModifierState.Off -> ModifierState.Once
    ModifierState.Once -> ModifierState.Locked
    ModifierState.Locked -> ModifierState.Off
}

// 与 Termux 一致：轻点仅切换一次性生效状态，锁定只由长按触发。
fun ModifierState.toggled(): ModifierState = if (this == ModifierState.Off) ModifierState.Once else ModifierState.Off

/**
 * 修饰键当前是否生效：`Once`（一次性）与 `Locked`（锁定）都算按住。
 *
 * 判定只此一处：粘滞状态分散在工具条、按键编码与输入法提交三条路径上，
 * 各写一遍时漏改一处就表现为「工具条亮着但按键没带修饰」。
 */
fun ModifierState.isActive(): Boolean = this == ModifierState.Locked || this == ModifierState.Once
