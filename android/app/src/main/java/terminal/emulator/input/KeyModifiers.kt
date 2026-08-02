package terminal.emulator.input

import android.view.KeyEvent

/**
 * Modifier bit masks shared by the key-encoder and hardware-key paths
 * (R5: round-3 architecture). Previously the mask constants and the
 * sticky-state OR logic were duplicated across TerminalSurface.modifierBitmask
 * and TerminalViewModel.handleLayoutAwareHardwareKey.
 */
object KeyModifiers {
    const val SHIFT = 1
    const val ALT = 2
    const val CTRL = 4
    const val META = 8

    /**
     * Mask contribution from the sticky toolbar modifier states only
     * (used by the layout-aware hardware-key path where Shift is already
     * baked into the produced character).
     */
    fun fromStickyStates(
        ctrlState: ModifierState,
        altState: ModifierState,
    ): Int {
        var mask = 0
        if (ctrlState == ModifierState.Locked || ctrlState == ModifierState.Once) {
            mask = mask or CTRL
        }
        if (altState == ModifierState.Locked || altState == ModifierState.Once) {
            mask = mask or ALT
        }
        return mask
    }

    /**
     * Full mask for a hardware key event: physical key state OR sticky
     * toolbar states (used by the surface's onKeyDown/onKeyUp path).
     */
    fun fromKeyEvent(
        event: KeyEvent,
        ctrlState: ModifierState,
        altState: ModifierState,
    ): Byte {
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
