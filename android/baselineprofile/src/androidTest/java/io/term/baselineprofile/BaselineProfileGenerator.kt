package io.term.baselineprofile

import androidx.benchmark.macro.junit4.BaselineProfileRule
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.uiautomator.By
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith

@RunWith(AndroidJUnit4::class)
class BaselineProfileGenerator {
    @get:Rule
    val baselineProfileRule = BaselineProfileRule()

    @Test
    fun generate() {
        baselineProfileRule.collect(
            packageName = "io.term",
            maxIterations = 15,
        ) {
            startActivityAndWait(getStartIntent())
            device.waitForIdle()
            val terminal = device.findObject(By.depth(0))
            terminal?.let {
                device.executeShellCommand("echo baseline_test")
            }
            device.waitForIdle()
            pressBack()
            device.waitForIdle()
        }
    }

    private fun getStartIntent() = androidx.test.platform.app.InstrumentationRegistry
        .getInstrumentation()
        .targetContext
        .packageManager
        .getLaunchIntentForPackage("io.term")!!
}
