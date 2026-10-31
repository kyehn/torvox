package terminal.emulator.input

enum class ModifierState { Off, Once, Held, Locked }

// 与 Termux 一致：轻点仅切换一次性生效状态，锁定只由长按触发。
fun ModifierState.toggled(): ModifierState = if (this == ModifierState.Off) ModifierState.Once else ModifierState.Off

/**
 * 修饰键当前是否生效：`Once`（一次性）与 `Locked`（锁定）都算按住。
 *
 * 判定只此一处：粘滞状态分散在工具条、按键编码与输入法提交三条路径上，
 * 各写一遍时漏改一处就表现为「工具条亮着但按键没带修饰」。
 */
fun ModifierState.isActive(): Boolean =
    this == ModifierState.Locked || this == ModifierState.Once || this == ModifierState.Held

/**
 * 音量键虚拟 Ctrl（`volume-keys = virtual`）按下/抬起后 Ctrl 应处的状态；不改时返回 null。
 *
 * 按下即 `Held`：Termux 的 `mVirtualControlKeyDown` 在按住期间持续生效，`Held` 不被
 * `consumeOneShotModifiers` 回收，故按住音量减再敲多键每键都带 Ctrl。
 *
 * 已 `Locked` 时按下 MUST NOT 改写：置 `Held` 后抬起会按「是 Held 就关」回收，用户手动
 * 锁定的 Ctrl 被这一次按住静默吃掉。抬起只回收自己置的 `Held`：这期间用户手动切成的
 * `Once`/`Locked` 一律不碰。
 */
internal fun ModifierState.afterVirtualCtrlKey(actionDown: Boolean): ModifierState? = if (actionDown) {
    if (this == ModifierState.Locked) null else ModifierState.Held
} else if (this == ModifierState.Held) {
    ModifierState.Off
} else {
    null
}
