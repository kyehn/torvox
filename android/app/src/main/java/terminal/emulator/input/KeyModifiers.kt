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
}
