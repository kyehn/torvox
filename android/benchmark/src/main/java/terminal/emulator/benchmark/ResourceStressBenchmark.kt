package terminal.emulator.benchmark

import android.os.SystemClock
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import androidx.test.uiautomator.UiDevice
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith

@RunWith(AndroidJUnit4::class)
class ResourceStressBenchmark {
    @get:Rule
    val environmentPinning = EnvironmentPinningRule(
        UiDevice.getInstance(InstrumentationRegistry.getInstrumentation()),
    )

    @Test
    fun resourceStress() {
        val device = UiDevice.getInstance(InstrumentationRegistry.getInstrumentation())
        val store = openMeasurementStore("resourceStress")
        val launcherActivity = readLauncherActivity(device, TARGET_PACKAGE)
        var sessionStartMillis = 0L
        measureScenario(
            device = device,
            label = "resourceStress",
            store = store,
            frameworkIteration = null,
            scenario = scenarioById("S12"),
            expectProcessExit = true,
            setup = {
                sessionStartMillis = System.currentTimeMillis()
                grantNotificationPermission(device)
                recoverEmulator(device)
                startAndWaitAtDebugLogLevel(device, launcherActivity)
                awaitShellReady(device, sessionStartMillis)
            },
        ) { _, budget ->
            val screenWasAwake = isScreenAwake(device)
            val artifacts = mutableListOf<ArtifactRef>()
            // 采集 batterystats 快照作为制品。
            val batterySnapshot = device.executeShellCommand("dumpsys batterystats --charged $TARGET_PACKAGE")
            store.writeArtifact("batterystats.txt", batterySnapshot.toByteArray())?.let { artifacts += it }
            applyResourceStress(device, TARGET_PACKAGE, screenWasAwake)
            val observeStart = SystemClock.elapsedRealtime()
            var processAlive = true
            while (SystemClock.elapsedRealtime() - observeStart < MAX_RESOURCE_STRESS_OBSERVE_MILLIS) {
                if (!isProcessAlive(device, TARGET_PACKAGE)) {
                    processAlive = false
                    break
                }
                SystemClock.sleep(RESOURCE_STRESS_POLL_MILLIS)
            }
            releaseResourceStress(device, screenWasAwake)
            val batterystatsAfter = device.executeShellCommand("dumpsys batterystats --charged $TARGET_PACKAGE")
            store.writeArtifact("batterystats-after.txt", batterystatsAfter.toByteArray())?.let { artifacts += it }
            val observedMillis = SystemClock.elapsedRealtime() - observeStart
            printMeasured("resourceStress", observedMillis, readFrameStats(device))
            emitMeasured(
                measuredLinePrefix("resourceStress", observedMillis) +
                    " processAlive=$processAlive screenWasAwake=$screenWasAwake",
            )
            // 「活着或死得有归因」均合格；UNKNOWN 归因由 measureScenario 判红。
            mapOf(
                "resource_stress_process_alive" to if (processAlive) 1.0 else 0.0,
            )
        }
    }

    private fun isScreenAwake(device: UiDevice): Boolean {
        val dump = device.executeShellCommand("dumpsys power")
        val match = Regex("mWakefulness=(\\w+)").find(dump)
        return match?.groupValues?.get(1)?.equals("Awake", ignoreCase = true) == true
    }

    private fun isProcessAlive(device: UiDevice, packageName: String): Boolean =
        device.executeShellCommand("pidof $packageName").trim().isNotEmpty()

    private fun applyResourceStress(device: UiDevice, packageName: String, screenWasAwake: Boolean) {
        device.executeShellCommand("am send-trim-memory $packageName COMPLETE")
        device.executeShellCommand("am send-trim-memory $packageName RUNNING_CRITICAL")
        device.executeShellCommand("cmd thermalservice override-status 3")
        device.executeShellCommand("input keyevent 26")
        device.executeShellCommand("dumpsys deviceidle force-idle")
    }

    private fun releaseResourceStress(device: UiDevice, screenWasAwake: Boolean) {
        device.executeShellCommand("dumpsys deviceidle unforce")
        device.executeShellCommand("cmd thermalservice override-status 0")
        // 灭屏的复原按当前状态决定：电源键是切换语义，施加压力时按过一次，屏幕状态与原先
        // 相反；当前状态与原值不同才再按一次，按当前状态复原才不会把设备留在被改过的状态里。
        if (needsScreenStateRestore(screenWasAwake, isScreenAwake(device))) {
            device.executeShellCommand("input keyevent 26")
        }
    }
}

/**
 * 屏幕状态是否需要按电源键复原：当前状态与原值不同即需要。
 *
 * 电源键是切换语义，按一次即翻转一次，故复原只看状态是否与原值一致。
 */
internal fun needsScreenStateRestore(screenWasAwake: Boolean, screenIsAwakeNow: Boolean): Boolean =
    screenWasAwake != screenIsAwakeNow
