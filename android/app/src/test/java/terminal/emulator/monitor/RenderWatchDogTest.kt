package terminal.emulator.monitor

import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test
import java.util.concurrent.CountDownLatch
import java.util.concurrent.TimeUnit
import kotlin.system.measureTimeMillis

/**
 * RenderWatchDog fires onHangDetected when the render thread stalls past
 * hangTimeoutNanos. Interval and timeout are injected so a JVM test runs
 * in milliseconds instead of the production 2 s / 10 s values.
 */
class RenderWatchDogTest {

    private companion object {
        /** `stop` 的允许耗时上限。旧实现阻塞满 2s，测试留出 20 倍余量仍能判红。 */
        const val STOP_BUDGET_MS = 100L
    }

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
        assertEquals("stop must silence the watchdog", 0, hangs)
    }

    /**
     * `stop` 在 `sessionLock` 内被调用（`pauseRendering` 的执行器线程、
     * `handleSessionExit` 的渲染线程），因此必须是非阻塞的：任何等待都会按会话数
     * 累加到主线程的 `stopForegroundServiceIfIdle` 上。
     *
     * 探针停在不可取消的阻塞上时，旧实现的 `runBlocking { cancelAndJoin() }`
     * 会阻塞满等待上限（生产 2s/会话）。
     */
    @Test
    fun `stop returns without waiting for the watchdog probe`() {
        val probeEntered = CountDownLatch(1)
        val release = CountDownLatch(1)
        val watchdog =
            RenderWatchDog(
                getStart = {
                    probeEntered.countDown()
                    release.await()
                    0L
                },
                getDone = { 0L },
                isRunning = { true },
                onHangDetected = {},
                hangTimeoutNanos = fastTimeout,
                checkIntervalMs = fastInterval,
            )
        watchdog.start()
        assertTrue("watchdog must reach the probe", probeEntered.await(2, TimeUnit.SECONDS))

        val stopMillis = measureTimeMillis { watchdog.stop() }
        release.countDown()

        assertTrue("stop blocked for ${stopMillis}ms", stopMillis < STOP_BUDGET_MS)
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
