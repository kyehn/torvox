package terminal.emulator.input

import android.view.KeyEvent

/** 编码器与硬件按键路径共用的修饰键位掩码。 */
object KeyModifiers {
    const val SHIFT = 1
    const val ALT = 2
    const val CTRL = 4
    const val META = 8

    /** 仅取工具栏粘滞状态；布局感知路径中 Shift 已并入生成的字符。 */
    fun fromStickyStates(ctrlState: ModifierState, altState: ModifierState): Int {
        var mask = 0
        if (ctrlState == ModifierState.Locked || ctrlState == ModifierState.Once) {
            mask = mask or CTRL
        }
        if (altState == ModifierState.Locked || altState == ModifierState.Once) {
            mask = mask or ALT
        }
        return mask
    }

    /** 硬件按键的完整掩码：物理按键状态与工具栏粘滞状态取并。 */
    fun fromKeyEvent(event: KeyEvent, ctrlState: ModifierState, altState: ModifierState): Byte {
        var mask = 0
        if (event.isShiftPressed) mask = mask or SHIFT
        if (event.isAltPressed || altState == ModifierState.Locked || altState == ModifierState.Once) {
            mask = mask or ALT
        }
        if (event.isCtrlPressed || ctrlState == ModifierState.Locked || ctrlState == ModifierState.Once) {
            mask = mask or CTRL
        }
        if (event.isMetaPressed) mask = mask or META
        return mask.toByte()
    }

    /**
     * 上游 `key.Mods` 原始位（`libghostty-vt` 的 `MODS_*`），供鼠标编码路径使用。
     *
     * 与本对象的 `SHIFT/ALT/CTRL/META` **位值不同**：后者是应用内部约定
     * （Rust `encode_modifiers` 按此解读），此处是上游协议位，Rust 侧
     * `Mods::from_bits_retain` 直接消费，两套不得混用。
     */
    object GhosttyMods {
        const val SHIFT = 1
        const val CTRL = 2
        const val ALT = 4
        const val SUPER = 8
    }

    /** Android [KeyEvent.metaState] → [GhosttyMods] 位值。 */
    fun ghosttyMods(metaState: Int): Int {
        var mods = 0
        if (metaState and KeyEvent.META_SHIFT_ON != 0) mods = mods or GhosttyMods.SHIFT
        if (metaState and KeyEvent.META_CTRL_ON != 0) mods = mods or GhosttyMods.CTRL
        if (metaState and KeyEvent.META_ALT_ON != 0) mods = mods or GhosttyMods.ALT
        if (metaState and KeyEvent.META_META_ON != 0) mods = mods or GhosttyMods.SUPER
        return mods
    }
}
