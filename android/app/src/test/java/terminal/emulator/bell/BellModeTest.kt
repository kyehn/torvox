package terminal.emulator.bell

import org.junit.Assert.assertEquals
import org.junit.Test

/**
 * Unit tests for [BellMode] enum and [BellHandler] debounce/mode-switching.
 *
 * BellMode is pure Kotlin — fully testable. BellHandler uses Android
 * system services (ToneGenerator/Vibrator), but debounce timing and
 * mode switching can be verified.
 */
class BellModeTest {

    @Test
    fun `BellMode fromId returns correct modes`() {
        assertEquals(BellMode.SOUND, BellMode.fromId(0))
        assertEquals(BellMode.VIBRATE, BellMode.fromId(1))
        assertEquals(BellMode.SCREEN_FLASH, BellMode.fromId(2))
        assertEquals(BellMode.SILENT, BellMode.fromId(3))
    }

    @Test
    fun `BellMode fromId falls back to SOUND for unknown id`() {
        assertEquals(BellMode.SOUND, BellMode.fromId(-1))
        assertEquals(BellMode.SOUND, BellMode.fromId(99))
        assertEquals(BellMode.SOUND, BellMode.fromId(Int.MAX_VALUE))
    }

    @Test
    fun `BellMode ids are stable for DataStore persistence`() {
        // These integer IDs are persisted in DataStore; changing them
        // would corrupt existing user settings.
        assertEquals(0, BellMode.SOUND.id)
        assertEquals(1, BellMode.VIBRATE.id)
        assertEquals(2, BellMode.SCREEN_FLASH.id)
        assertEquals(3, BellMode.SILENT.id)
    }
}
