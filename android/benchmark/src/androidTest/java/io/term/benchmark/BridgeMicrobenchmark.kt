package io.term.benchmark

import androidx.benchmark.macro.FrameTimingMetric
import androidx.benchmark.macro.StartupTimingMetric
import androidx.benchmark.macro.junit4.MacrobenchmarkRule
import androidx.test.ext.junit.runners.AndroidJUnit4
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith

@RunWith(AndroidJUnit4::class)
class BridgeMicrobenchmark {
    @get:Rule
    val benchmarkRule = MacrobenchmarkRule()

    @Test
    fun coldStart() {
        benchmarkRule.measureRepeated(
            packageName = "com.termux",
            metrics = listOf(StartupTimingMetric(), FrameTimingMetric()),
            iterations = 10,
            setupBlock = {
                device.pressHome()
            },
            measureBlock = {
                startActivityAndWait()
            },
        )
    }

    @Test
    fun warmStart() {
        benchmarkRule.measureRepeated(
            packageName = "com.termux",
            metrics = listOf(StartupTimingMetric()),
            iterations = 10,
            setupBlock = {
                startActivityAndWait()
                device.waitForIdle()
                device.pressHome()
            },
            measureBlock = {
                startActivityAndWait()
            },
        )
    }

    @Test
    fun terminalOutputTiming() {
        benchmarkRule.measureRepeated(
            packageName = "com.termux",
            metrics = listOf(StartupTimingMetric()),
            iterations = 10,
            setupBlock = {
                device.pressHome()
            },
            measureBlock = {
                startActivityAndWait()
            },
        )
    }
}
