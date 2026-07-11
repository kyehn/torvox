package io.torvox.ui

/**
 * Scans terminal output byte buffers for DECSET/DECRST escape sequences that
 * enable or disable private terminal modes.
 *
 * Tracks:
 * - Mouse modes 1000 (basic), 1002 (button-event), 1003 (any-event)
 * - Bracketed paste mode 2004
 * - Alternate screen buffer 1049 / 1047 / 47 (vim, less, htop)
 *
 * Pattern: ESC [ ? <digits> h  (enable)
 *          ESC [ ? <digits> l  (disable)
 *
 * Handles partial sequences across buffer boundaries via a simple state machine.
 */
class MouseModeTracker {
    private enum class State {
        GROUND,
        ESC,
        BRACKET,
        QUESTION,
        DIGITS,
    }

    private var state = State.GROUND
    private var modeAccumulator = 0
    private val pendingModes = mutableListOf<Int>()

    private val activeModes = mutableSetOf<Int>()

    val mouseMode: Boolean
        get() = activeModes.isNotEmpty()

    /** The highest active mouse mode (1000/1002/1003), or null if none. */
    val activeMouseMode: Int?
        get() = activeModes.maxOrNull()

    var bracketPasteMode: Boolean = false
        private set

    private val activeAlternateScreenModes = mutableSetOf<Int>()

    /** True while the remote is on the alternate screen buffer (vim/less/...). */
    val altScreen: Boolean
        get() = activeAlternateScreenModes.isNotEmpty()

    companion object {
        private val MOUSE_MODES = setOf(1000, 1002, 1003)
        private const val BRACKET_PASTE_MODE = 2004
        private val ALT_SCREEN_MODES = setOf(1049, 1047, 47)
    }

    fun process(
        data: ByteArray,
        offset: Int,
        length: Int,
    ) {
        val end = offset + length
        for (index in offset until end) {
            val byte = data[index].toInt() and 0xFF
            processByte(byte)
        }
    }

    private fun processByte(byte: Int) {
        when (state) {
            State.GROUND -> processGround(byte)
            State.ESC -> processEsc(byte)
            State.BRACKET -> processBracket(byte)
            State.QUESTION -> processQuestion(byte)
            State.DIGITS -> processDigits(byte)
        }
    }

    private fun processGround(byte: Int) {
        if (byte == 0x1B) state = State.ESC
    }

    private fun processEsc(byte: Int) {
        state = if (byte == '['.code) State.BRACKET else State.GROUND
    }

    private fun processBracket(byte: Int) {
        state =
            if (byte == '?'.code) {
                modeAccumulator = 0
                pendingModes.clear()
                State.QUESTION
            } else {
                State.GROUND
            }
    }

    private fun processQuestion(byte: Int) {
        if (byte in '0'.code..'9'.code) {
            modeAccumulator = byte - '0'.code
            pendingModes.clear()
            state = State.DIGITS
        } else {
            state = State.GROUND
        }
    }

    private fun processDigits(byte: Int) {
        when {
            byte in '0'.code..'9'.code -> {
                modeAccumulator = modeAccumulator * 10 + (byte - '0'.code)
            }

            byte == ';'.code -> {
                pendingModes.add(modeAccumulator)
                modeAccumulator = 0
            }

            byte == 'h'.code -> {
                pendingModes.add(modeAccumulator)
                for (mode in pendingModes) applyMode(mode, enable = true)
                pendingModes.clear()
                state = State.GROUND
            }

            byte == 'l'.code -> {
                pendingModes.add(modeAccumulator)
                for (mode in pendingModes) applyMode(mode, enable = false)
                pendingModes.clear()
                state = State.GROUND
            }

            else -> {
                pendingModes.clear()
                state = State.GROUND
            }
        }
    }

    private fun applyMode(
        mode: Int,
        enable: Boolean,
    ) {
        when (mode) {
            in MOUSE_MODES -> {
                if (enable) activeModes.add(mode) else activeModes.remove(mode)
            }

            BRACKET_PASTE_MODE -> {
                bracketPasteMode = enable
            }

            in ALT_SCREEN_MODES -> {
                if (enable) {
                    activeAlternateScreenModes.add(mode)
                } else {
                    activeAlternateScreenModes.remove(mode)
                }
            }
        }
    }
}
