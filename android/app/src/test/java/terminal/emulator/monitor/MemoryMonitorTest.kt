package terminal.emulator.monitor

import org.junit.Assert.assertEquals
import org.junit.Test

class MemoryMonitorTest {

    @Test
    fun memoryPressure_lowMemoryFlag_is_critical_regardless_of_avail() {
        assertEquals(MemoryPressure.Critical, memoryPressure(availMb = 0, thresholdMb = 400, lowMemory = true))
        assertEquals(MemoryPressure.Critical, memoryPressure(availMb = 5000, thresholdMb = 400, lowMemory = true))
    }

    @Test
    fun memoryPressure_below_double_threshold_is_warning() {
        assertEquals(MemoryPressure.Warning, memoryPressure(availMb = 799, thresholdMb = 400, lowMemory = false))
        assertEquals(MemoryPressure.Warning, memoryPressure(availMb = 1, thresholdMb = 400, lowMemory = false))
    }

    @Test
    fun memoryPressure_at_exactly_double_threshold_is_ok() {
        // Strict less-than: avail == threshold * 2 must NOT warn.
        assertEquals(MemoryPressure.Ok, memoryPressure(availMb = 800, thresholdMb = 400, lowMemory = false))
    }

    @Test
    fun memoryPressure_above_double_threshold_is_ok() {
        assertEquals(MemoryPressure.Ok, memoryPressure(availMb = 1000, thresholdMb = 400, lowMemory = false))
    }

    @Test
    fun memoryPressure_zero_threshold_never_warns() {
        // Strict less-than against 0.0f — the original logic never fires warning.
        assertEquals(MemoryPressure.Ok, memoryPressure(availMb = 0, thresholdMb = 0, lowMemory = false))
        assertEquals(MemoryPressure.Ok, memoryPressure(availMb = 1, thresholdMb = 0, lowMemory = false))
    }
}
