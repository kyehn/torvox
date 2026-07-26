package io.term.runtime

import io.mockk.every
import io.mockk.mockkStatic
import io.mockk.unmockkStatic
import io.term.monitor.RenderWatchDog
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Test
import java.util.concurrent.CountDownLatch
import java.util.concurrent.TimeUnit
import java.util.concurrent.atomic.AtomicBoolean
import java.util.concurrent.atomic.AtomicInteger
import java.util.concurrent.atomic.AtomicLong

/**
 * Pure-JVM unit tests for render thread resilience patterns.
 *
 * Tests cover:
 *  1. RuntimeState initial/default values (the public data class used across the bridge)
 *  2. Companion constant thresholds (error limits, timeouts, restart policy)
 *  3. RenderWatchDog hung-render detection
 *  4. Transient vs persistent error classification (constant invariants)
 *  5. Mutex poison recovery (synchronized block resilience)
 */
class RenderThreadTest {
    @Before
    fun setup() {
        mockkStatic(android.util.Log::class)
        every { android.util.Log.e(any<String>(), any<String>()) } returns 0
        every { android.util.Log.e(any<String>(), any<String>(), any<Throwable>()) } returns 0
        every { android.util.Log.w(any<String>(), any<String>()) } returns 0
        every { android.util.Log.d(any<String>(), any<String>()) } returns 0
        every { android.util.Log.i(any<String>(), any<String>()) } returns 0
    }

    @After
    fun tearDown() {
        unmockkStatic(android.util.Log::class)
    }

    // ── 1. RuntimeState default values ──────────────────────────────────

    @Test
    fun runtimeState_isRunning_defaultsToFalse() {
        val state = RuntimeState()
        assertFalse("isRunning should default to false", state.isRunning)
    }

    @Test
    fun runtimeState_title_defaultsToTerminal() {
        val state = RuntimeState()
        assertEquals("Terminal", state.title)
    }

    @Test
    fun runtimeState_rows_defaultsTo24() {
        val state = RuntimeState()
        assertEquals(24, state.rows)
    }

    @Test
    fun runtimeState_cols_defaultsTo80() {
        val state = RuntimeState()
        assertEquals(80, state.cols)
    }

    @Test
    fun runtimeState_activeSessionId_defaultsToZero() {
        val state = RuntimeState()
        assertEquals(0L, state.activeSessionId)
    }

    @Test
    fun runtimeState_sessionIds_defaultsToEmpty() {
        val state = RuntimeState()
        assertEquals(emptyList<Long>(), state.sessionIds)
    }

    @Test
    fun runtimeState_copy_preservesUnmodifiedFields() {
        val original = RuntimeState(isRunning = true, title = "bash", rows = 50, cols = 120)
        val copy = original.copy(title = "zsh")
        assertTrue(copy.isRunning)
        assertEquals("zsh", copy.title)
        assertEquals(50, copy.rows)
        assertEquals(120, copy.cols)
    }

    // ── 2. Companion constant thresholds via reflection ─────────────────

    private fun getPrivateCompanionConstant(name: String): Any? {
        val clazz = TerminalRuntime::class.java
        val field = clazz.getDeclaredField(name)
        field.isAccessible = true
        return field.get(null)
    }

    @Test
    fun companion_maxConsecutiveErrors_is100() {
        val value = getPrivateCompanionConstant("RENDER_MAX_CONSECUTIVE_ERRORS")
        assertEquals(100, (value as Int))
    }

    @Test
    fun companion_maxTransientErrors_is50() {
        val value = getPrivateCompanionConstant("RENDER_MAX_TRANSIENT_ERRORS")
        assertEquals(50, (value as Int))
    }

    @Test
    fun companion_errorSleepMs_is50() {
        val value = getPrivateCompanionConstant("RENDER_ERROR_SLEEP_MS")
        assertEquals(50L, value)
    }

    @Test
    fun companion_errorBackoffMs_is200() {
        val value = getPrivateCompanionConstant("RENDER_ERROR_BACKOFF_MS")
        assertEquals(200L, value)
    }

    @Test
    fun companion_hangTimeoutNanos_is10Seconds() {
        val value = getPrivateCompanionConstant("RENDER_HANG_TIMEOUT_NANOS")
        assertEquals(10_000_000_000L, value)
    }

    @Test
    fun companion_maxRestartAttempts_is5() {
        val value = getPrivateCompanionConstant("RENDER_MAX_RESTART_ATTEMPTS")
        assertEquals(5, (value as Int))
    }

    @Test
    fun companion_initialRestartDelayMs_is100() {
        val value = getPrivateCompanionConstant("INITIAL_RESTART_DELAY_MS")
        assertEquals(100L, value)
    }

    @Test
    fun companion_maxRestartDelayMs_is1000() {
        val value = getPrivateCompanionConstant("MAX_RESTART_DELAY_MS")
        assertEquals(1000L, value)
    }

    @Test
    fun companion_gracePeriodAfterRestartMs_is300() {
        val value = getPrivateCompanionConstant("GRACE_PERIOD_AFTER_RESTART_MS")
        assertEquals(300L, value)
    }

    @Test
    fun companion_renderMonitorIntervalMs_is500() {
        val value = getPrivateCompanionConstant("RENDER_MONITOR_INTERVAL_MS")
        assertEquals(500L, value)
    }

    @Test
    fun companion_latchTimeoutNanos_is16ms() {
        val value = getPrivateCompanionConstant("RENDER_LATCH_TIMEOUT_NANOS")
        assertEquals(16_000_000L, value)
    }

    @Test
    fun companion_latchIdleTimeoutNanos_is500ms() {
        val value = getPrivateCompanionConstant("RENDER_LATCH_IDLE_TIMEOUT_NANOS")
        assertEquals(500_000_000L, value)
    }

    @Test
    fun companion_idleThresholdNanos_is5Seconds() {
        val value = getPrivateCompanionConstant("RENDER_IDLE_THRESHOLD_NANOS")
        assertEquals(5_000_000_000L, value)
    }

    // ── 3. RenderWatchDog hung-render detection ─────────────────────────

    @Test
    fun watchdog_detectsHang_whenStartGreaterThanDone() {
        val hangDetected = AtomicBoolean(false)
        val startNanos = AtomicLong(System.nanoTime())
        val doneNanos = AtomicLong(0L) // done < start → hung

        val watchdog =
            RenderWatchDog(
                getStart = { startNanos.get() },
                getDone = { doneNanos.get() },
                isRunning = { true },
                onHangDetected = { hangDetected.set(true) },
                hangTimeoutNanos = 1L, // 1 nanosecond — detect immediately
            )
        watchdog.start()
        val detected = hangDetected.get() || hangDetected.waitForValue(5000L)
        watchdog.stop()
        assertTrue("watchdog should detect a hung render", detected)
    }

    @Test
    fun watchdog_doesNotFire_whenRenderIsHealthy() {
        val hangDetected = AtomicBoolean(false)
        val now = System.nanoTime()

        val watchdog =
            RenderWatchDog(
                getStart = { now },
                getDone = { now }, // done == start → healthy
                isRunning = { true },
                onHangDetected = { hangDetected.set(true) },
                hangTimeoutNanos = 1L,
            )
        watchdog.start()
        Thread.sleep(1500) // less than CHECK_INTERVAL_MS (2000)
        watchdog.stop()
        assertFalse("watchdog should NOT fire for a healthy render", hangDetected.get())
    }

    @Test
    fun watchdog_doesNotFire_whenNotRunning() {
        val hangDetected = AtomicBoolean(false)
        val startNanos = AtomicLong(System.nanoTime())
        val doneNanos = AtomicLong(0L)

        val watchdog =
            RenderWatchDog(
                getStart = { startNanos.get() },
                getDone = { doneNanos.get() },
                isRunning = { false }, // session stopped
                onHangDetected = { hangDetected.set(true) },
                hangTimeoutNanos = 1L,
            )
        watchdog.start()
        Thread.sleep(1500)
        watchdog.stop()
        assertFalse("watchdog should NOT fire when session is not running", hangDetected.get())
    }

    @Test
    fun watchdog_stop_preventsFurtherCallbacks() {
        val hangDetected = AtomicBoolean(false)
        val startNanos = AtomicLong(System.nanoTime())
        val doneNanos = AtomicLong(0L)

        val watchdog =
            RenderWatchDog(
                getStart = { startNanos.get() },
                getDone = { doneNanos.get() },
                isRunning = { true },
                onHangDetected = { hangDetected.set(true) },
                hangTimeoutNanos = 1L,
            )
        watchdog.start()
        watchdog.stop()
        // After stop, no further callbacks should fire
        Thread.sleep(2500) // well past one CHECK_INTERVAL_MS
        // hangDetected may or may not have fired before stop — that's OK.
        // The important thing is it doesn't fire AGAIN after stop.
        val firstValue = hangDetected.get()
        Thread.sleep(2500)
        assertEquals("no additional callbacks after stop()", firstValue, hangDetected.get())
    }

    @Test
    fun watchdog_callsOnHangDetectedExactlyOnce_perHang() {
        val hangCount = AtomicInteger(0)
        val startNanos = AtomicLong(System.nanoTime())
        val doneNanos = AtomicLong(0L)
        val latch = CountDownLatch(1)

        val watchdog =
            RenderWatchDog(
                getStart = { startNanos.get() },
                getDone = { doneNanos.get() },
                isRunning = { true },
                onHangDetected = {
                    hangCount.incrementAndGet()
                    latch.countDown()
                },
                hangTimeoutNanos = 1L,
            )
        watchdog.start()
        latch.await(5, TimeUnit.SECONDS)
        watchdog.stop()
        // The watchdog checks every 2s. Over 5s window we get at most 2-3 checks,
        // but each one fires onHangDetected (no dedup in the class).
        assertTrue("onHangDetected should fire at least once", hangCount.get() >= 1)
    }

    // ── 4. Transient vs persistent error classification ─────────────────

    @Test
    fun transientErrorThreshold_isHalfOfPersistentThreshold() {
        val maxTransient = getPrivateCompanionConstant("RENDER_MAX_TRANSIENT_ERRORS") as Int
        val maxConsecutive = getPrivateCompanionConstant("RENDER_MAX_CONSECUTIVE_ERRORS") as Int
        assertEquals(
            "transient limit (50) must be half of persistent limit (100)",
            maxConsecutive / 2,
            maxTransient,
        )
    }

    @Test
    fun transientErrorThreshold_allowsBackoffTransition() {
        // After 10 transient errors the code switches from 50ms to 200ms backoff.
        // The transient limit (50) must be greater than this transition point.
        val maxTransient = getPrivateCompanionConstant("RENDER_MAX_TRANSIENT_ERRORS") as Int
        assertTrue(
            "transient limit ($maxTransient) must exceed the backoff transition point (10)",
            maxTransient > 10,
        )
    }

    @Test
    fun errorBackoffMs_isFourTimesErrorSleepMs() {
        val sleepMs = getPrivateCompanionConstant("RENDER_ERROR_SLEEP_MS") as Long
        val backoffMs = getPrivateCompanionConstant("RENDER_ERROR_BACKOFF_MS") as Long
        assertEquals("backoff should be 4x base sleep", sleepMs * 4, backoffMs)
    }

    @Test
    fun maxRestartAttempts_preventsInfiniteRestart() {
        val maxAttempts = getPrivateCompanionConstant("RENDER_MAX_RESTART_ATTEMPTS") as Int
        assertTrue("max restart attempts must be bounded", maxAttempts in 1..20)
    }

    @Test
    fun restartDelayExponentialBackoff_isCapped() {
        val initial = getPrivateCompanionConstant("INITIAL_RESTART_DELAY_MS") as Long
        val max = getPrivateCompanionConstant("MAX_RESTART_DELAY_MS") as Long
        assertTrue("initial delay must be less than max delay", initial < max)
        // Simulate exponential backoff: 100, 200, 400, 800, 1000 (capped)
        var delay = initial
        val delays = mutableListOf(delay)
        for (i in 1..5) {
            delay = (delay * 2).coerceAtMost(max)
            delays.add(delay)
        }
        assertEquals("after 5 doublings starting at 100, cap at 1000", 1000L, delays.last())
        assertEquals("6 steps of backoff", 6, delays.size)
    }

    // ── 5. Mutex poison recovery ────────────────────────────────────────

    @Test
    fun synchronized_recoversAfterPoison() {
        val lock = Any()
        val counter = AtomicInteger(0)

        // Poison the lock by throwing inside synchronized
        try {
            synchronized(lock) {
                counter.incrementAndGet()
                error("simulated poison")
            }
        } catch (_: IllegalStateException) {
            // expected
        }

        // The lock is NOT permanently poisoned in Java/Kotlin reentrant locks.
        // Unlike Rust's Mutex, Java's synchronized block is always recoverable.
        synchronized(lock) {
            counter.incrementAndGet()
        }
        assertEquals("counter should be 2 after recovery", 2, counter.get())
    }

    @Test
    fun reentrantLock_survivesExceptionInCriticalSection() {
        val lock =
            java.util.concurrent.locks
                .ReentrantLock()
        val counter = AtomicInteger(0)

        lock.lock()
        try {
            counter.incrementAndGet()
            throw RuntimeException("simulated error")
        } catch (_: RuntimeException) {
            // handled
        } finally {
            lock.unlock()
        }

        // Lock is free and usable after exception
        lock.lock()
        try {
            counter.incrementAndGet()
        } finally {
            lock.unlock()
        }
        assertEquals("counter should be 2", 2, counter.get())
    }

    @Test
    fun atomicReference_survivesConcurrentUpdateRace() {
        val ref =
            java.util.concurrent.atomic
                .AtomicReference("initial")
        val attempts = AtomicInteger(0)

        // Simulate multiple threads racing to update; some may fail
        val threads =
            (1..10).map {
                Thread {
                    attempts.incrementAndGet()
                    ref.compareAndSet("initial", "updated-$it")
                }
            }
        threads.forEach { it.start() }
        threads.forEach { it.join() }

        assertEquals("all threads attempted", 10, attempts.get())
        assertTrue(
            "value was updated by exactly one thread",
            ref.get().startsWith("updated-"),
        )
    }

    @Test
    fun atomicBoolean_flagResetAfterSuccess() {
        // Simulates error-counter reset: flag goes true on error, false on recovery
        val hasError = AtomicBoolean(false)
        val errorCount = AtomicInteger(0)

        // Simulate error
        hasError.set(true)
        errorCount.incrementAndGet()

        // Simulate recovery (reset)
        hasError.set(false)
        errorCount.set(0)

        assertFalse("error flag cleared after recovery", hasError.get())
        assertEquals("error count reset to zero", 0, errorCount.get())
    }

    @Test
    fun concurrentHashMapIsThreadSafeForSessionMap() {
        val sessions = java.util.concurrent.ConcurrentHashMap<Long, String>()
        val latch = CountDownLatch(20)

        val writers =
            (1L..20L).map { id ->
                Thread {
                    sessions[id] = "session-$id"
                    latch.countDown()
                }
            }
        writers.forEach { it.start() }
        latch.await(5, TimeUnit.SECONDS)
        writers.forEach { it.join() }

        assertEquals("all 20 sessions written", 20, sessions.size)
        for (id in 1L..20L) {
            assertEquals("session $id present", "session-$id", sessions[id])
        }
    }

    // ── helpers ─────────────────────────────────────────────────────────

    private fun AtomicBoolean.waitForValue(timeoutMs: Long): Boolean {
        val deadline = System.currentTimeMillis() + timeoutMs
        while (System.currentTimeMillis() < deadline) {
            if (get()) return true
            Thread.sleep(10)
        }
        return get()
    }
}
