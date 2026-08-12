package terminal.emulator.shortcut

import android.view.KeyEvent
import terminal.emulator.TerminalViewModel

/**
 * Dispatches hardware-keyboard shortcuts to terminal actions.
 *
 * Mirrors reterminal KeyShortcutHandler.kt:9-99 (research-mid-repos-b.md
 * 4.2): a fixed set of [ShortcutAction]s, each with a configurable
 * [ShortcutBinding] persisted via SettingsRepository. The handler is
 * asked first during key dispatch (see TerminalSurface.onKeyDown) and
 * returns true only when the event exactly matches a bound chord — so
 * unbound combinations (Ctrl+C SIGINT, Ctrl+D EOF, …) always flow
 * through to the terminal encoder untouched.
 *
 * Threading: [dispatch] is called on the main thread (view key events);
 * [setBindings] is called from the UI/settings layer with the same
 * bindings the settings screen edits. It is a simple snapshot swap, safe
 * for main-thread use.
 */
class KeyShortcutHandler(
    private val viewModel: TerminalViewModel,
) {
    /** Terminal actions reachable from hardware shortcuts. */
    enum class Action {
        Paste,
        NewSession,
        CloseSession,
        Copy,
        ToggleScroll,
    }

    private data class Entry(
        val action: Action,
        val binding: ShortcutBinding,
    )

    private val idToAction: Map<String, Action> = mapOf(
        Defaults.ACTION_ID_PASTE to Action.Paste,
        Defaults.ACTION_ID_NEW_SESSION to Action.NewSession,
        Defaults.ACTION_ID_CLOSE_SESSION to Action.CloseSession,
        Defaults.ACTION_ID_COPY to Action.Copy,
        Defaults.ACTION_ID_TOGGLE_SCROLL to Action.ToggleScroll,
    )

    private var entries: List<Entry> = emptyList()

    /** Replaces the current bindings (from the settings snapshot). */
    fun setBindings(bindings: Map<String, ShortcutBinding>) {
        entries =
            bindings.mapNotNull { (id, binding) ->
                val action = idToAction[id] ?: return@mapNotNull null
                if (binding.isEmpty()) null else Entry(action, binding)
            }
    }

    /**
     * Returns the binding registered for [action], or [ShortcutBinding.EMPTY].
     */
    fun bindingFor(action: Action): ShortcutBinding = entries.firstOrNull { it.action == action }?.binding ?: ShortcutBinding.EMPTY

    /**
     * Default shortcuts and their stable action ids (settings keys).
     * Defaults follow the reterminal convention — Ctrl+Shift chords —
     * and avoid single-modifier Ctrl+letter so Ctrl+C / Ctrl+D / Ctrl+V
     * keep reaching the terminal (SIGINT / EOF / termux paste).
     */
    object Defaults {
        /** Stable identifier used in SettingsRepository keys. */
        const val ACTION_ID_PASTE = "paste"
        const val ACTION_ID_NEW_SESSION = "new_session"
        const val ACTION_ID_CLOSE_SESSION = "close_session"
        const val ACTION_ID_COPY = "copy"
        const val ACTION_ID_TOGGLE_SCROLL = "toggle_scroll"

        fun all(): Map<String, ShortcutBinding> = mapOf(
            ACTION_ID_PASTE to ShortcutBinding(key = KeyEvent.KEYCODE_V, ctrl = true, shift = true),
            ACTION_ID_NEW_SESSION to ShortcutBinding(key = KeyEvent.KEYCODE_N, ctrl = true, shift = true),
            ACTION_ID_CLOSE_SESSION to ShortcutBinding(key = KeyEvent.KEYCODE_W, ctrl = true, shift = true),
            ACTION_ID_COPY to ShortcutBinding(key = KeyEvent.KEYCODE_C, ctrl = true, shift = true),
            ACTION_ID_TOGGLE_SCROLL to ShortcutBinding(key = KeyEvent.KEYCODE_S, ctrl = true, shift = true),
        )

        fun bindingFor(actionId: String): ShortcutBinding = all()[actionId] ?: ShortcutBinding.EMPTY
    }

    /**
     * Consumes [event] when it exactly matches a bound shortcut.
     *
     * @return true if the shortcut was dispatched (caller must not forward
     *   the key to the terminal), false when the key must fall through to
     *   normal terminal encoding.
     */
    fun dispatch(event: KeyEvent): Boolean {
        val matched =
            entries.firstOrNull { it.binding.matches(event) } ?: return false
        perform(matched.action)
        return true
    }

    private fun perform(action: Action) {
        when (action) {
            Action.Paste -> viewModel.pasteFromClipboard()
            Action.NewSession -> viewModel.createSession()
            Action.CloseSession -> viewModel.closeSession()
            Action.Copy -> viewModel.copySelectionToClipboard()
            Action.ToggleScroll -> viewModel.toggleScrollMode()
        }
    }
}
