package io.torvox.ui

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Test

class MouseModeTrackerTest {
    private lateinit var tracker: MouseModeTracker

    @Before
    fun setUp() {
        tracker = MouseModeTracker()
    }

    @Test
    fun initial_state_has_no_modes_active() {
        assertFalse(tracker.mouseMode)
        assertNull(tracker.activeMouseMode)
        assertFalse(tracker.bracketPasteMode)
        assertFalse(tracker.altScreen)
    }

    @Test
    fun enable_mouse_mode_1000() {
        val sequence = "\u001B[?1000h".toByteArray(Charsets.UTF_8)
        tracker.process(sequence, 0, sequence.size)
        assertTrue(tracker.mouseMode)
        assertEquals(1000, tracker.activeMouseMode)
    }

    @Test
    fun enable_mouse_mode_1002() {
        val sequence = "\u001B[?1002h".toByteArray(Charsets.UTF_8)
        tracker.process(sequence, 0, sequence.size)
        assertTrue(tracker.mouseMode)
        assertEquals(1002, tracker.activeMouseMode)
    }

    @Test
    fun enable_mouse_mode_1003() {
        val sequence = "\u001B[?1003h".toByteArray(Charsets.UTF_8)
        tracker.process(sequence, 0, sequence.size)
        assertTrue(tracker.mouseMode)
        assertEquals(1003, tracker.activeMouseMode)
    }

    @Test
    fun disable_mouse_mode_1000() {
        val enable = "\u001B[?1000h".toByteArray(Charsets.UTF_8)
        tracker.process(enable, 0, enable.size)
        assertTrue(tracker.mouseMode)

        val disable = "\u001B[?1000l".toByteArray(Charsets.UTF_8)
        tracker.process(disable, 0, disable.size)
        assertFalse(tracker.mouseMode)
        assertNull(tracker.activeMouseMode)
    }

    @Test
    fun enable_multiple_mouse_modes() {
        val enable = "\u001B[?1000;1006h".toByteArray(Charsets.UTF_8)
        tracker.process(enable, 0, enable.size)
        assertTrue(tracker.mouseMode)
        assertEquals(1000, tracker.activeMouseMode)
    }

    @Test
    fun enable_bracket_paste_mode() {
        val sequence = "\u001B[?2004h".toByteArray(Charsets.UTF_8)
        tracker.process(sequence, 0, sequence.size)
        assertTrue(tracker.bracketPasteMode)
    }

    @Test
    fun disable_bracket_paste_mode() {
        val enable = "\u001B[?2004h".toByteArray(Charsets.UTF_8)
        tracker.process(enable, 0, enable.size)
        assertTrue(tracker.bracketPasteMode)

        val disable = "\u001B[?2004l".toByteArray(Charsets.UTF_8)
        tracker.process(disable, 0, disable.size)
        assertFalse(tracker.bracketPasteMode)
    }

    @Test
    fun enable_alternate_screen_1049() {
        val sequence = "\u001B[?1049h".toByteArray(Charsets.UTF_8)
        tracker.process(sequence, 0, sequence.size)
        assertTrue(tracker.altScreen)
    }

    @Test
    fun enable_alternate_screen_47() {
        val sequence = "\u001B[?47h".toByteArray(Charsets.UTF_8)
        tracker.process(sequence, 0, sequence.size)
        assertTrue(tracker.altScreen)
    }

    @Test
    fun disable_alternate_screen_1049() {
        val enable = "\u001B[?1049h".toByteArray(Charsets.UTF_8)
        tracker.process(enable, 0, enable.size)
        assertTrue(tracker.altScreen)

        val disable = "\u001B[?1049l".toByteArray(Charsets.UTF_8)
        tracker.process(disable, 0, disable.size)
        assertFalse(tracker.altScreen)
    }

    @Test
    fun partial_sequence_across_buffers() {
        val part1 = "\u001B[?100".toByteArray(Charsets.UTF_8)
        tracker.process(part1, 0, part1.size)

        val part2 = "0h".toByteArray(Charsets.UTF_8)
        tracker.process(part2, 0, part2.size)

        assertTrue(tracker.mouseMode)
        assertEquals(1000, tracker.activeMouseMode)
    }

    @Test
    fun non_matching_sequence_does_not_affect_state() {
        val sequence = "\u001B[0m".toByteArray(Charsets.UTF_8)
        tracker.process(sequence, 0, sequence.size)
        assertFalse(tracker.mouseMode)
        assertFalse(tracker.bracketPasteMode)
        assertFalse(tracker.altScreen)
    }

    @Test
    fun interleaved_text_does_not_affect_state() {
        val text = "hello world\u001B[?1003h".toByteArray(Charsets.UTF_8)
        tracker.process(text, 0, text.size)
        assertTrue(tracker.mouseMode)
        assertEquals(1003, tracker.activeMouseMode)
    }

    @Test
    fun highest_mouse_mode_wins() {
        val enable = "\u001B[?1000;1002;1003h".toByteArray(Charsets.UTF_8)
        tracker.process(enable, 0, enable.size)
        assertTrue(tracker.mouseMode)
        assertEquals(1003, tracker.activeMouseMode)
    }

    @Test
    fun disable_one_of_multiple_mouse_modes() {
        val enable = "\u001B[?1000;1002h".toByteArray(Charsets.UTF_8)
        tracker.process(enable, 0, enable.size)
        assertEquals(1002, tracker.activeMouseMode)

        val disable = "\u001B[?1002l".toByteArray(Charsets.UTF_8)
        tracker.process(disable, 0, disable.size)
        assertTrue(tracker.mouseMode)
        assertEquals(1000, tracker.activeMouseMode)
    }

    @Test
    fun partial_sequence_then_invalid_terminator_resets() {
        val partial = "\u001B[?100".toByteArray(Charsets.UTF_8)
        tracker.process(partial, 0, partial.size)

        val invalid = "x".toByteArray(Charsets.UTF_8)
        tracker.process(invalid, 0, invalid.size)

        assertFalse(tracker.mouseMode)
    }
}
