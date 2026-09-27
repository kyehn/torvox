package terminal.emulator.input

enum class ModifierState { Off, Once, Locked }

fun ModifierState.next(): ModifierState = when (this) {
    ModifierState.Off -> ModifierState.Once
    ModifierState.Once -> ModifierState.Locked
    ModifierState.Locked -> ModifierState.Off
}

// 与 Termux 一致：轻点仅切换一次性生效状态，锁定只由长按触发。
fun ModifierState.toggled(): ModifierState = if (this == ModifierState.Off) ModifierState.Once else ModifierState.Off
