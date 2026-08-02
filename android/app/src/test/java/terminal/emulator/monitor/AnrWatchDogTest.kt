package terminal.emulator.monitor

import android.content.Context
import androidx.test.core.app.ApplicationProvider
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.Shadows.shadowOf
import java.io.File
import java.util.concurrent.CountDownLatch
import java.util.concurrent.TimeUnit
import java.util.concurrent.atomic.AtomicInteger

/**
 * Watchdog kill logic. Robolectric's main Looper is paused by default, so a
 * `mainHandler.post` never runs unless the test idles it — this makes the
 * "main thread stuck" scenario deterministic:
 *
 * - no idle → `completed` never set → timeout → `onAnr` fires;
 * - continuous idle → `completed` set every round → no ANR.
 */
@RunWith(RobolectricTestRunner::class)
class AnrWatchDogTest {
    private val context = ApplicationProvider.getApplicationContext<Context>()
    private val logDir = File(context.cacheDir, "anr_test_${System.nanoTime()}")

    /** Idle the paused main looper until `untilNanos` is reached. */
    private fun idleMainLooper(untilMs: Long) {
        while (System.currentTimeMillis() < untilMs) {
            shadowOf(android.os.Looper.getMainLooper()).idle()
            Thread.sleep(10)
        }
    }

    @Test
    fun `fires onAnr when main thread never responds`() {
        val fired = CountDownLatch(1)
        val dog =
            AnrWatchDog(
                logDir = logDir,
                timeoutMs = 100L,
                warmUpMillis = 0L,
                onAnr = { fired.countDown() },
            )
        dog.start()
        assertTrue("onAnr must fire", fired.await(5, TimeUnit.SECONDS))
        dog.stop()
    }

    @Test
    fun `does not fire when main thread responds`() {
        val fired = AtomicInteger(0)
        val dog =
            AnrWatchDog(
                logDir = logDir,
                timeoutMs = 500L,
                warmUpMillis = 0L,
                onAnr = { fired.incrementAndGet() },
            )
        dog.start()
        // Keep idling the main looper for well past the timeout so every
        // probe round completes before its deadline.
        val deadline = System.currentTimeMillis() + 2_000
        idleMainLooper(deadline)
        assertEquals("ANR must not fire", 0, fired.get())
        dog.stop()
    }

    @Test
    fun `start twice does not spawn two watchers`() {
        val fired = AtomicInteger(0)
        val first = CountDownLatch(1)
        val dog =
            AnrWatchDog(
                logDir = logDir,
                timeoutMs = 100L,
                warmUpMillis = 0L,
                onAnr = {
                    fired.incrementAndGet()
                    first.countDown()
                },
            )
        dog.start()
        dog.start() // second call must be a no-op (CAS)
        assertTrue(first.await(5, TimeUnit.SECONDS))
        // If a second watcher existed it would fire a second time.
        Thread.sleep(300)
        assertEquals("only one firing expected", 1, fired.get())
        dog.stop()
    }

    @Test
    fun `stop before timeout prevents firing`() {
        val fired = AtomicInteger(0)
        val dog =
            AnrWatchDog(
                logDir = logDir,
                timeoutMs = 100L,
                warmUpMillis = 0L,
                onAnr = { fired.incrementAndGet() },
            )
        dog.start()
        Thread.sleep(20)
        dog.stop()
        Thread.sleep(300)
        assertEquals("stopped watchdog must not fire", 0, fired.get())
    }
}
