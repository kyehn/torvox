package terminal.emulator.util

import kotlinx.coroutines.CancellationException
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Assert.fail
import org.junit.Test

class RunCatchingTest {
    @Test
    fun success_returns_value() {
        assertEquals(42, runCatchingCancellable { 42 }.getOrNull())
    }

    @Test
    fun exception_captured_as_failure() {
        val result = runCatchingCancellable<String> { throw IllegalStateException("boom") }
        assertTrue(result.isFailure)
        assertEquals("boom", result.exceptionOrNull()?.message)
    }

    @Test
    fun cancellation_is_rethrown() {
        try {
            runCatchingCancellable<String> { throw CancellationException("cancel") }
            fail("CancellationException must not be swallowed")
        } catch (expected: CancellationException) {
            assertEquals("cancel", expected.message)
        }
    }
}
