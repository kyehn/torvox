package terminal.emulator.input

/** Toggle state of a modifier key (CTRL/ALT) on the toolbar. */
enum class ModifierState { Off, Once, Locked }

/** Cycle through the toggle states on each press. */
fun ModifierState.next(): ModifierState = when (this) {
    ModifierState.Off -> ModifierState.Once
    ModifierState.Once -> ModifierState.Locked
    ModifierState.Locked -> ModifierState.Off
}

/**
 * Termux-parity tap toggle for CTRL/ALT (termux ExtraKeysView
 * onAnyExtraKeyButtonClick): tap arms one-shot, tap again disarms —
 * never locks. Locking is long-press only.
 */
fun ModifierState.toggled(): ModifierState = if (this == ModifierState.Off) ModifierState.Once else ModifierState.Off
