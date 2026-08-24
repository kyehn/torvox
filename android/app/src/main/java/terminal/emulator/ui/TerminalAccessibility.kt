package terminal.emulator.ui

/**
 * Supplies one grid row's text for accessibility reading; implemented over
 * [terminal.emulator.bridge.NativeBridge.scrollbackLine] on the device and
 * over a fake in JVM tests.
 */
fun interface AccessibilityLineSource {
    /** Row in grid coordinates; null when the row is blank/outside the buffer. */
    fun line(row: Int): String?
}

/** One readable grid line with its absolute row number. */
data class AccessibilityLine(
    val row: Int,
    val text: String,
)

/**
 * Builds the visible-screen line list used for TalkBack line-by-line
 * navigation, termlib AccessibilityOverlay pattern). Pure
 * Kotlin: unit-tested on the JVM with a fake [AccessibilityLineSource].
 */
class AccessibilityLineProvider(
    private val lineSource: AccessibilityLineSource,
) {
    /**
     * The lines currently visible in the viewport, top to bottom. Grid
     * rows run from `scrollbackLength - scrollOffset` up to `+ rows - 1`;
     * trailing whitespace is trimmed and blank rows are dropped so
     * TalkBack does not read empty lines.
     */
    fun visibleLines(
        rows: Int,
        scrollbackLength: Int,
        scrollOffset: Int,
    ): List<AccessibilityLine> {
        if (rows <= 0) return emptyList()
        val firstRow = (scrollbackLength - scrollOffset).coerceAtLeast(0)
        val lastRowExclusive = firstRow + rows
        val lines = ArrayList<AccessibilityLine>(rows)
        for (row in firstRow until lastRowExclusive) {
            val text = lineSource.line(row)?.trimEnd() ?: continue
            if (text.isEmpty()) continue
            lines.add(AccessibilityLine(row, text))
        }
        return lines
    }

    /**
     * A single description of the visible screen: lines joined with "\n".
     * Capped at [MAX_DESCRIPTION_CHARS] so TalkBack is not flooded by a
     * full 40x80 screen.
     */
    fun contentDescription(lines: List<AccessibilityLine>): String {
        val joined = lines.joinToString("\n") { it.text }
        if (joined.length <= MAX_DESCRIPTION_CHARS) return joined
        return joined.take(MAX_DESCRIPTION_CHARS) + "…"
    }

    companion object {
        /** Cap for the screen-wide content description (avoid TalkBack flooding). */
        const val MAX_DESCRIPTION_CHARS = 2000
    }
}

/**
 * Tracks the "currently read" grid row for the Next line / Previous line
 * accessibility actions. Navigation wraps around the visible screen and
 * keeps its position when the viewport scrolls (the row stays current as
 * long as it remains visible). Pure Kotlin: unit-tested on the JVM.
 */
class AccessibilityLineNavigator(
    private val lineProvider: AccessibilityLineProvider,
) {
    private var currentRow: Int? = null

    /** The line to read when the user asks for the current screen. */
    fun current(
        rows: Int,
        scrollbackLength: Int,
        scrollOffset: Int,
    ): AccessibilityLine? {
        val lines = lineProvider.visibleLines(rows, scrollbackLength, scrollOffset)
        // The remembered row wins while it stays visible; otherwise fall
        // back to the top visible line. Either way the chosen line becomes
        // the new current row so a following next()/previous() continues
        // from where the user actually is (fix: current() must
        // also position the cursor).
        val line =
            if (currentRow != null) {
                lines.firstOrNull { it.row == currentRow } ?: lines.firstOrNull()
            } else {
                lines.firstOrNull()
            }
        if (line != null) {
            currentRow = line.row
        }
        return line
    }

    /** The line after the current one, wrapping to the first line at the end. */
    fun next(
        rows: Int,
        scrollbackLength: Int,
        scrollOffset: Int,
    ): AccessibilityLine? {
        val lines = visibleLines(rows, scrollbackLength, scrollOffset) ?: return null
        val index = currentIndexIn(lines)
        val target = if (index >= 0 && index < lines.size - 1) lines[index + 1] else lines[0]
        currentRow = target.row
        return target
    }

    /** The line before the current one, wrapping to the last line at the top. */
    fun previous(
        rows: Int,
        scrollbackLength: Int,
        scrollOffset: Int,
    ): AccessibilityLine? {
        val lines = visibleLines(rows, scrollbackLength, scrollOffset) ?: return null
        val index = currentIndexIn(lines)
        val target = if (index > 0) lines[index - 1] else lines.last()
        currentRow = target.row
        return target
    }

    /** The visible lines for the current viewport, or null when empty. */
    private fun visibleLines(
        rows: Int,
        scrollbackLength: Int,
        scrollOffset: Int,
    ): List<AccessibilityLine>? {
        val lines = lineProvider.visibleLines(rows, scrollbackLength, scrollOffset)
        if (lines.isEmpty()) return null
        return lines
    }

    private fun currentIndexIn(lines: List<AccessibilityLine>): Int {
        val row = currentRow ?: return -1
        return lines.indexOfFirst { it.row == row }
    }
}

/**
 * Debounces updates to a text value: only the last [update] within
 * [debounceMillis] is emitted, and identical text is not re-scheduled.
 * Used to throttle the terminal's accessibility contentDescription
 * refresh (500ms) so TalkBack is not flooded on every scroll frame.
 * Pure Kotlin: unit-tested on the JVM with a fake [DebounceScheduler].
 */
class DebouncedTextUpdater(
    private val debounceMillis: Long,
    private val scheduler: DebounceScheduler,
) {
    // Written from the render thread (accessibilityRenderTick) and cleared
    // from the main thread (navigateAccessibilityLine cancel/update), so
    // visibility is guaranteed across threads.
    @Volatile private var pendingText: String? = null

    /** Schedule [text] for emission; identical pending text is a no-op. */
    fun update(text: String, onEmit: (String) -> Unit) {
        if (pendingText == text) return
        pendingText = text
        scheduler.cancelPending()
        scheduler.postDelayed(debounceMillis) {
            val toEmit = pendingText
            pendingText = null
            if (toEmit != null) onEmit(toEmit)
        }
    }

    /** Drop the pending text without emitting it. */
    fun cancel() {
        pendingText = null
        scheduler.cancelPending()
    }
}
