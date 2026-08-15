package terminal.emulator.ui

/**
 * Expands a Termux-style extra-keys macro into a list of key sequences.
 *
 * Mirrors termux-kotlin `TerminalExtraKeys.onExtraKeyButtonClick` macro
 * semantics:
 *  - The macro is a space-separated list of tokens.
 *  - `CTRL` / `ALT` / `SHIFT` / `FN` are sticky modifier flags: they apply
 *    to the NEXT ordinary key token and are then all reset.
 *  - Ordinary tokens are either special keys (ESC, TAB, arrows, F1-F12…)
 *    translated to their escape sequences (with CSI modifier parameters
 *    when a modifier is active), or text code points (Ctrl folded via the
 *    ANSI control table, Alt prefixed with ESC).
 *  - Alias table (`CONTROL_CHARS_ALIASES`) normalizes alternate spellings
 * PAGEUP → PGUP, BACKSPACE → BKSP, …).
 *
 * Pure function: no Android dependencies, unit-testable on the JVM.
 */
object ToolbarMacroExpander {

    /** termux ExtraKeysConstants.CONTROL_CHARS_ALIASES (canonical → canonical). */
    private val aliases: Map<String, String> =
        mapOf(
            "CTRL" to "CTRL",
            "ALT" to "ALT",
            "SHIFT" to "SHIFT",
            "FN" to "FN",
            "ESC" to "ESC",
            "ESCAPE" to "ESC",
            "CONTROL" to "CTRL",
            "SHFT" to "SHIFT",
            "RETURN" to "ENTER",
            "FUNCTION" to "FN",
            "TAB" to "TAB",
            "ENTER" to "ENTER",
            "SPACE" to "SPACE",
            "HOME" to "HOME",
            "END" to "END",
            "PGUP" to "PGUP",
            "PGDN" to "PGDN",
            "PAGEUP" to "PGUP",
            "PAGE_UP" to "PGUP",
            "PAGE UP" to "PGUP",
            "PAGE-UP" to "PGUP",
            "PAGEDOWN" to "PGDN",
            "PAGE_DOWN" to "PGDN",
            "PAGE-DOWN" to "PGDN",
            "LT" to "LEFT",
            "RT" to "RIGHT",
            "DN" to "DOWN",
            "INS" to "INS",
            "DEL" to "DEL",
            "DELETE" to "DEL",
            "BKSP" to "BKSP",
            "BACKSPACE" to "BKSP",
            "BACKSLASH" to "\\",
            "QUOTE" to "\"",
            "APOSTROPHE" to "'",
        )

    /** Special keys → escape sequence with no modifiers (termux KeyHandler). */
    private val specialKeySequences: Map<String, String> =
        mapOf(
            "ESC" to "\u001b",
            "TAB" to "\t",
            "ENTER" to "\r",
            "SPACE" to " ",
            "HOME" to "\u001b[H",
            "END" to "\u001b[F",
            "PGUP" to "\u001b[5~",
            "PGDN" to "\u001b[6~",
            "INS" to "\u001b[2~",
            "DEL" to "\u001b[3~",
            "BKSP" to "\u007f",
            "UP" to "\u001b[A",
            "DOWN" to "\u001b[B",
            "LEFT" to "\u001b[D",
            "RIGHT" to "\u001b[C",
        ) + FN_KEY_SEQUENCES.toMap()

    /** Special keys that support CSI `;mod` parameters when a modifier is active. */
    private val csiModifiableKeys: Set<String> =
        setOf(
            "HOME",
            "END",
            "PGUP",
            "PGDN",
            "INS",
            "DEL",
            "UP",
            "DOWN",
            "LEFT",
            "RIGHT",
            "F1",
            "F2",
            "F3",
            "F4",
            "F5",
            "F6",
            "F7",
            "F8",
            "F9",
            "F10",
            "F11",
            "F12",
        )

    /** Modifier tokens; `FN` is accepted for termux parity but produces no
     *  output (termux KeyHandler does not act on META_FUNCTION_ON). */
    private val modifierTokens = setOf("CTRL", "ALT", "SHIFT", "FN")

    /**
     * Expand a macro string into one string per ordinary key press, in
     * order. Modifier tokens produce no entry. An unknown token is kept
     * verbatim (termux passes unknown keys through to inputCodePoint).
     */
    fun expand(macro: String): List<String> {
        if (macro.isBlank()) return emptyList()
        // Filter empty tokens so double spaces cannot consume a sticky
        // modifier or emit a phantom empty key audit fix).
        val tokens = macro.split(" ").filter { it.isNotEmpty() }
        var ctrl = false
        var alt = false
        var shift = false
        var fn = false
        val output = mutableListOf<String>()
        for (raw in tokens) {
            val token = aliases[raw] ?: raw
            when (token) {
                "CTRL" -> ctrl = true

                "ALT" -> alt = true

                "SHIFT" -> shift = true

                "FN" -> fn = true

                else -> {
                    output.add(expandKey(token, ctrl, alt, shift, fn))
                    ctrl = false
                    alt = false
                    shift = false
                    fn = false
                }
            }
        }
        return output
    }

    private fun expandKey(
        token: String,
        ctrl: Boolean,
        alt: Boolean,
        shift: Boolean,
        fn: Boolean,
    ): String {
        val special = specialKeySequences[token]
        if (special != null) {
            // Escape sequences follow the KeyHandler modifier transform:
            // CSI 1;modX when a modifier is active (SHIFT=1, ALT=2, CTRL=4
            // bitmask + 1), bare sequence otherwise.
            if (ctrl || alt || shift || fn) {
                if (token in csiModifiableKeys) {
                    val param = 1 + (if (shift) 1 else 0) + (if (alt) 2 else 0) + (if (ctrl) 4 else 0)
                    return csiWithModifier(token, param) ?: special
                }
                // TAB/ENTER/SPACE/ESC/BKSP have no CSI form; Alt prefix
                // applies (termux: Alt+Enter → ESC \r, Alt+TAB → ESC TAB).
                if (alt) {
                    return "\u001b$special"
                }
            }
            return special
        }
        // Text token: Ctrl folds via the ANSI control table; Alt prefixes
        // ESC. Shift/FN are ignored for text (termux inputCodePoint
        // ignores them in the text branch).
        var cp = token
        if (ctrl) {
            cp = ctrlFold(cp)
        }
        return if (alt) "\u001b$cp" else cp
    }

    /** CSI sequences with a modifier parameter (termux transformForModifiers). */
    private val csiWithModifierSequences: Map<String, String> =
        mapOf(
            "UP" to "\u001b[1;%dA",
            "DOWN" to "\u001b[1;%dB",
            "RIGHT" to "\u001b[1;%dC",
            "LEFT" to "\u001b[1;%dD",
            "HOME" to "\u001b[1;%dH",
            "END" to "\u001b[1;%dF",
            "PGUP" to "\u001b[5;%d~",
            "PGDN" to "\u001b[6;%d~",
            "INS" to "\u001b[2;%d~",
            "DEL" to "\u001b[3;%d~",
            "F1" to "\u001b[1;%dP",
            "F2" to "\u001b[1;%dQ",
            "F3" to "\u001b[1;%dR",
            "F4" to "\u001b[1;%dS",
            "F5" to "\u001b[15;%d~",
            "F6" to "\u001b[17;%d~",
            "F7" to "\u001b[18;%d~",
            "F8" to "\u001b[19;%d~",
            "F9" to "\u001b[20;%d~",
            "F10" to "\u001b[21;%d~",
            "F11" to "\u001b[23;%d~",
            "F12" to "\u001b[24;%d~",
        )

    private fun csiWithModifier(
        token: String,
        param: Int,
    ): String? = csiWithModifierSequences[token]?.format(param)

    /**
     * termux TerminalView.inputCodePoint Ctrl conversion: a-z → 0x01-0x1A,
     * space/2 → NUL, [ /3 → ESC, \ /4 → 0x1C, ] /5 → 0x1D, ^ /6 → 0x1E,
     * _ /7 → 0x1F, 8 → DEL.
     */
    private fun ctrlFold(token: String): String {
        if (token.length != 1) return token
        val c = token[0]
        val folded: Char =
            when {
                c in 'a'..'z' -> (c.code - 'a'.code + 1).toChar()
                c in 'A'..'Z' -> (c.code - 'A'.code + 1).toChar()
                c == ' ' || c == '2' -> 0.toChar()
                c == '[' || c == '3' -> 0x1B.toChar()
                c == '\\' || c == '4' -> 0x1C.toChar()
                c == ']' || c == '5' -> 0x1D.toChar()
                c == '^' || c == '6' -> 0x1E.toChar()
                c == '_' || c == '7' -> 0x1F.toChar()
                c == '8' -> 0x7F.toChar()
                else -> return token
            }
        return folded.toString()
    }

    /**
     * True when [macro] uses macro semantics (contains a space-separated
     * sequence with at least one modifier token or more than one token) —
     * used to decide between legacy single-sequence and macro handling.
     */
    fun isMacro(macro: String): Boolean {
        if (macro.isBlank()) return false
        val tokens = macro.split(" ").filter { it.isNotEmpty() }
        return tokens.size > 1 || tokens.any { it in modifierTokens }
    }
}
