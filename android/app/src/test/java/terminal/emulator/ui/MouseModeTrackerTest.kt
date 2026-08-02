package terminal.emulator.ui

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

class MouseModeTrackerTest {
    private fun esc(suffix: String): ByteArray = ("\u001B[?" + suffix).toByteArray(Charsets.US_ASCII)

    @Test
    fun `starts with no modes`() {
        val tracker = MouseModeTracker()
        assertFalse(tracker.mouseMode)
        assertNull(tracker.activeMouseMode)
        assertFalse(tracker.bracketPasteMode)
        assertFalse(tracker.altScreen)
    }

    @Test
    fun `enable mouse 1000`() {
        val tracker = MouseModeTracker()
        tracker.process(esc("1000h"), 0, esc("1000h").size)
        assertTrue(tracker.mouseMode)
        assertEquals(1000, tracker.activeMouseMode)
    }

    @Test
    fun `disable mouse 1000`() {
        val tracker = MouseModeTracker()
        tracker.process(esc("1000h"), 0, esc("1000h").size)
        tracker.process(esc("1000l"), 0, esc("1000l").size)
        assertFalse(tracker.mouseMode)
        assertNull(tracker.activeMouseMode)
    }

    @Test
    fun `highest mouse mode wins`() {
        val tracker = MouseModeTracker()
        tracker.process(esc("1000h"), 0, esc("1000h").size)
        tracker.process(esc("1003h"), 0, esc("1003h").size)
        assertEquals(1003, tracker.activeMouseMode)
        tracker.process(esc("1003l"), 0, esc("1003l").size)
        assertEquals(1000, tracker.activeMouseMode)
    }

    @Test
    fun `bracketed paste mode`() {
        val tracker = MouseModeTracker()
        tracker.process(esc("2004h"), 0, esc("2004h").size)
        assertTrue(tracker.bracketPasteMode)
        tracker.process(esc("2004l"), 0, esc("2004l").size)
        assertFalse(tracker.bracketPasteMode)
    }

    @Test
    fun `alternate screen 1049`() {
        val tracker = MouseModeTracker()
        tracker.process(esc("1049h"), 0, esc("1049h").size)
        assertTrue(tracker.altScreen)
        tracker.process(esc("1049l"), 0, esc("1049l").size)
        assertFalse(tracker.altScreen)
    }

    @Test
    fun `sequence split across chunks`() {
        val tracker = MouseModeTracker()
        val full = esc("1000h")
        tracker.process(full.copyOfRange(0, 4), 0, 4)
        tracker.process(full.copyOfRange(4, full.size), 0, full.size - 4)
        assertTrue(tracker.mouseMode)
        assertEquals(1000, tracker.activeMouseMode)
    }

    @Test
    fun `multiple modes in one sequence`() {
        val tracker = MouseModeTracker()
        tracker.process(esc("1000;1003h"), 0, esc("1000;1003h").size)
        assertEquals(1003, tracker.activeMouseMode)
    }
}
