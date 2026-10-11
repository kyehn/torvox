package terminal.emulator.input

import android.view.KeyEvent

/**
 * 修饰键掩码：**上游 `key.Mods` 的原始位**，全仓唯一一种表示。
 *
 * 曾经有两套并存：应用自定义位（SHIFT=1/ALT=2/CTRL=4/META=8）与上游位
 * （SHIFT=1/CTRL=2/ALT=4/SUPER=8）。两套形状相近而位值不同，每个转换点都是一次
 * 可能错位的机会，而鼠标与按键路径还各走一套。现统一为上游位：原生侧
 * `Mods::from_bits_retain` 与 `encode_modifiers` 消费同一套。
 */
object KeyModifiers {
    const val SHIFT = 1
    const val CTRL = 2
    const val ALT = 4
    const val SUPER = 8

    /**
     * 仅取工具栏粘滞状态。布局感知路径中 Shift 已并入生成的字符，故不进掩码。
     */
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
    fun fromKeyEvent(event: KeyEvent, ctrlState: ModifierState, altState: ModifierState): Int {
        var mask = fromStickyStates(ctrlState, altState)
        if (event.isShiftPressed) mask = mask or SHIFT
        if (event.isCtrlPressed) mask = mask or CTRL
        if (event.isAltPressed) mask = mask or ALT
        if (event.isMetaPressed) mask = mask or SUPER
        return mask
    }

    /**
     * Android [KeyEvent.metaState] → 本掩码。触摸与滚轮路径没有 KeyEvent 的修饰
     * 查询可用，只能读 metaState（鼠标编码直接用这套位）。
     */
    fun ghosttyMods(metaState: Int): Int {
        var mask = 0
        if (metaState and KeyEvent.META_SHIFT_ON != 0) mask = mask or SHIFT
        if (metaState and KeyEvent.META_CTRL_ON != 0) mask = mask or CTRL
        if (metaState and KeyEvent.META_ALT_ON != 0) mask = mask or ALT
        if (metaState and KeyEvent.META_META_ON != 0) mask = mask or SUPER
        return mask
    }
}
