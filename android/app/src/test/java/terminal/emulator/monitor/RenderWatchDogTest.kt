package terminal.emulator.monitor

import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * RenderWatchDog fires onHangDetected when the render thread stalls past
 * hangTimeoutNanos. Interval and timeout are injected so a JVM test runs
 * in milliseconds instead of the production 2 s / 10 s values.
 */
class RenderWatchDogTest {

    private val fastTimeout = 1_000_000L // 1 ms
    private val fastInterval = 10L // 10 ms

    @Test
    fun `stalled render thread triggers the hang callback`() {
        var hangs = 0
        // Start timestamp in the past, done never advanced past it:
        // start > done means a frame began and never finished.
        val stalledStart = System.nanoTime() - 10_000_000_000L
        val watchdog =
            RenderWatchDog(
                getStart = { stalledStart },
                getDone = { 0L },
                isRunning = { true },
                onHangDetected = { hangs++ },
                hangTimeoutNanos = fastTimeout,
                checkIntervalMs = fastInterval,
            )
        watchdog.start()
        Thread.sleep(200)
        watchdog.stop()
        assertTrue("stalled renderer must trigger the callback", hangs > 0)
    }

    @Test
    fun `progressing render thread stays silent`() {
        var hangs = 0
        val watchdog =
            RenderWatchDog(
                getStart = { System.nanoTime() },
                getDone = { System.nanoTime() },
                isRunning = { true },
                onHangDetected = { hangs++ },
                hangTimeoutNanos = fastTimeout,
                checkIntervalMs = fastInterval,
            )
        watchdog.start()
        Thread.sleep(200)
        watchdog.stop()
        assertEquals("a healthy renderer must not trigger", 0, hangs)
    }

    @Test
    fun `stopped watchdog never fires`() {
        var hangs = 0
        val watchdog =
            RenderWatchDog(
                getStart = { 0L },
                getDone = { 0L },
                isRunning = { true },
                onHangDetected = { hangs++ },
                hangTimeoutNanos = fastTimeout,
                checkIntervalMs = fastInterval,
            )
        watchdog.start()
        watchdog.stop()
        Thread.sleep(100)
        assertEquals("stop must join and silence the watchdog", 0, hangs)
    }

    @Test
    fun `not-running renderer does not fire even when stalled`() {
        var hangs = 0
        val watchdog =
            RenderWatchDog(
                getStart = { 0L },
                getDone = { 0L },
                isRunning = { false },
                onHangDetected = { hangs++ },
                hangTimeoutNanos = fastTimeout,
                checkIntervalMs = fastInterval,
            )
        watchdog.start()
        Thread.sleep(200)
        watchdog.stop()
        assertEquals("a stopped renderer must not be reported as hung", 0, hangs)
    }
}
