package terminal.emulator.runtime

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * Deterministic unit tests for the input→echo latency probe
 * (emulator-performance-verification spec). All timestamps are synthetic —
 * the probe reads no clocks itself.
 */
class LatencyProbeTest {

    @Test
    fun `no input stamp yields no sample`() {
        val probe = LatencyProbe()
        assertNull(probe.onEchoFrame(1_000L))
    }

    @Test
    fun `echo pairs with newest unconsumed input`() {
        val probe = LatencyProbe()
        probe.onInputWritten(1_000L)
        assertEquals(5_000L, probe.onEchoFrame(6_000L))
    }

    @Test
    fun `same stamp is never consumed twice`() {
        val probe = LatencyProbe()
        probe.onInputWritten(1_000L)
        assertEquals(4_000L, probe.onEchoFrame(5_000L))
        assertNull(probe.onEchoFrame(6_000L))
    }

    @Test
    fun `stale older stamp is skipped not paired`() {
        val probe = LatencyProbe()
        probe.onInputWritten(1_000L)
        probe.onInputWritten(9_000L) // newer overwrite
        // An echo arriving now must pair against 9000, not the stale 1000.
        assertEquals(1_000L, probe.onEchoFrame(10_000L))
        assertNull(probe.onEchoFrame(11_000L))
    }

    @Test
    fun `negative diff is discarded without recording`() {
        val probe = LatencyProbe(capacity = 8)
        probe.onInputWritten(Long.MAX_VALUE / 2)
        // Simulate a caller clock anomaly: now < input. Must not record.
        assertNull(probe.onEchoFrame(0L))
        assertEquals(0, probe.sampleCount)
    }

    @Test
    fun `ring buffer keeps most recent samples`() {
        val probe = LatencyProbe(capacity = 3)
        var t = 0L
        for (i in 1..5) {
            t += 10L
            probe.onInputWritten(t)
            t += i * 100L // latency grows: 100,200,300,400,500
            probe.onEchoFrame(t)
        }
        assertEquals(listOf(300L, 400L, 500L), probe.snapshot())
    }

    @Test
    fun `percentile below minimum samples reports null`() {
        val probe = LatencyProbe()
        repeat(LatencyProbe.MIN_SAMPLES - 1) { i ->
            probe.onInputWritten(i * 1_000L)
            probe.onEchoFrame(i * 1_000L + 500L)
        }
        assertNull(probe.percentile(50.0))
        assertTrue(probe.report().startsWith("latency NOT MEASURED"))
    }

    @Test
    fun `percentile nearest rank at minimum samples`() {
        val probe = LatencyProbe()
        // 30 samples: latencies 1..30 ms (in nanos).
        var t = 0L
        for (ms in 1..30L) {
            t += 1_000_000L
            probe.onInputWritten(t)
            t += ms * 1_000_000L
            probe.onEchoFrame(t)
        }
        assertEquals(30, probe.sampleCount)
        // Nearest-rank p50 of 1..30: ceil(0.5*30)=15th → 15ms.
        assertEquals(15_000_000L, probe.percentile(50.0))
        // p95: ceil(0.95*30)=ceil(28.5)=29th → 29ms.
        assertEquals(29_000_000L, probe.percentile(95.0))
    }

    @Test
    fun `report formats millis at and above threshold`() {
        val probe = LatencyProbe()
        var t = 0L
        for (ms in 1..LatencyProbe.MIN_SAMPLES) {
            t += 1_000_000L
            probe.onInputWritten(t)
            t += ms * 1_000_000L
            probe.onEchoFrame(t)
        }
        val report = probe.report()
        assertTrue(report, report.startsWith("latency n=30"))
        assertTrue(report, report.contains("p50="))
        assertTrue(report, report.contains("p95="))
    }
}
