package terminal.emulator.test

import android.util.Log
import androidx.test.ext.junit.runners.AndroidJUnit4
import org.junit.Assert.assertEquals
import org.junit.Test
import org.junit.runner.RunWith

/**
 * Measures logd's exact truncation boundary for a given tag so the
 * chunking budget in LogUtil / native log_chunk can be calibrated from
 * real device behavior (round-227 T2).
 */
@RunWith(AndroidJUnit4::class)
class LogcatBoundaryProbeTest {

    @Test
    fun probeLogdTruncationBoundary() {
        val tag = "T2ChunkProbe"
        // Write messages of increasing size; find the largest that round-trips.
        var lastGood = -1
        var firstBad = -1
        for (size in listOf(4020, 4022, 4024, 4026, 4028, 4030, 4032, 4034, 4036)) {
            val marker = "M$size="
            val msg = marker + "P".repeat(size)
            Log.i(tag, msg)
            Thread.sleep(300)
            val read = readBack(tag, marker)
            if (read == size) {
                lastGood = size
            } else if (firstBad < 0) {
                firstBad = size
                Log.i(tag, "probe size=$size readBack=$read (truncated)")
            }
        }
        Log.i(tag, "probe lastGood=$lastGood firstBad=$firstBad")
    }

    private fun readBack(tag: String, marker: String): Int {
        val process = ProcessBuilder("logcat", "-d", "-s", "$tag:I")
            .redirectErrorStream(true)
            .start()
        val output = process.inputStream.bufferedReader().readText()
        process.waitFor()
        for (line in output.lines()) {
            val idx = line.indexOf(marker)
            if (idx >= 0) {
                return line.length - idx - marker.length
            }
        }
        return -1
    }

    @Test
    fun sanityTagLength() {
        assertEquals(12, "T2ChunkProbe".length)
    }
}
