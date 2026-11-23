package terminal.emulator.benchmark

import android.os.SystemClock
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import androidx.test.uiautomator.UiDevice
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith

@RunWith(AndroidJUnit4::class)
class BackgroundForegroundBenchmark {
    @get:Rule
    val environmentPinning = EnvironmentPinningRule(
        UiDevice.getInstance(InstrumentationRegistry.getInstrumentation()),
    )

    @Test
    fun backgroundForeground() {
        val device = UiDevice.getInstance(InstrumentationRegistry.getInstrumentation())
        val store = openMeasurementStore("backgroundForeground")
        val launcherActivity = readLauncherActivity(device, TARGET_PACKAGE)
        var sessionStartMillis = 0L
        lateinit var contentLayer: String
        measureScenario(
            device = device,
            label = "backgroundForeground",
            store = store,
            frameworkIteration = null,
            scenario = scenarioById("S11"),
            setup = {
                sessionStartMillis = System.currentTimeMillis()
                grantNotificationPermission(device)
                recoverEmulator(device)
                startAndWaitAtDebugLogLevel(device, launcherActivity)
                contentLayer = awaitTerminalContentFrames(device).first
                awaitShellReady(device, sessionStartMillis)
            },
        ) { _, _ ->
            // 切后台：home 键，等焦点离开。
            val restoreStartElapsedMillis = SystemClock.elapsedRealtime()
            device.pressHome()
            val backgrounded = awaitBackgrounded(device, TARGET_PACKAGE, 5_000L)
            check(backgrounded) { "切后台后焦点没有离开被测应用" }
            SystemClock.sleep(1_000L)
            // 切回前台。
            val restoreStartedAtMillis = SystemClock.elapsedRealtime()
            val launchResult = startAndWaitAtDebugLogLevel(device, launcherActivity)
            check(launchResult.launchState == "HOT" || launchResult.launchState == "WARM") {
                "切回前台 launchState=${launchResult.launchState}，预期 HOT/WARM"
            }
            val newContentLayer = awaitContentLayerOwnedBy(device, TARGET_PACKAGE)
            val firstPresentNanos = awaitLayerPresentAfter(
                device = device,
                layerName = newContentLayer,
                sinceMillis = restoreStartedAtMillis,
                timeoutMillis = MAX_RESTORE_FIRST_FRAME_MILLIS,
                pollMillis = SUSTAINED_OUTPUT_POLL_MILLIS,
            )
            val restoreFirstFrameMillis = firstPresentNanos / 1_000_000L - restoreStartedAtMillis
            clearLayerFrameHistory(device, newContentLayer)
            val restoreStats = readLayerFrameStats(device, newContentLayer)
            // 旋转。
            val originalRotation = readRotationSettings(device)
            setRotation(device, 1)
            val rotationStartedAtMillis = SystemClock.elapsedRealtime()
            awaitDisplayRotation(device, 1)
            val rotatedContentLayer = awaitContentLayerOwnedBy(device, TARGET_PACKAGE)
            val rotationFirstPresentNanos = awaitLayerPresentAfter(
                device = device,
                layerName = rotatedContentLayer,
                sinceMillis = rotationStartedAtMillis,
                timeoutMillis = MAX_ROTATION_FIRST_FRAME_MILLIS,
                pollMillis = SUSTAINED_OUTPUT_POLL_MILLIS,
            )
            val rotationFirstFrameMillis = rotationFirstPresentNanos / 1_000_000L - rotationStartedAtMillis
            restoreRotationSettings(device, originalRotation)
            printMeasured("backgroundForeground", restoreFirstFrameMillis, readFrameStats(device))
            emitMeasured(
                measuredLinePrefix("backgroundForeground", restoreFirstFrameMillis) +
                    " restoreFirstFrame=${restoreFirstFrameMillis}ms" +
                    " rotationFirstFrame=${rotationFirstFrameMillis}ms" +
                    " launchState=${launchResult.launchState}",
            )
            check(restoreFirstFrameMillis <= MAX_RESTORE_FIRST_FRAME_MILLIS) {
                "切回前台首帧 ${restoreFirstFrameMillis}ms 超过上限 $MAX_RESTORE_FIRST_FRAME_MILLIS ms"
            }
            check(rotationFirstFrameMillis <= MAX_ROTATION_FIRST_FRAME_MILLIS) {
                "旋转首帧 ${rotationFirstFrameMillis}ms 超过上限 $MAX_ROTATION_FIRST_FRAME_MILLIS ms"
            }
            if (restoreStats != null) {
                check(restoreStats.presentationSlotRatio >= BACKGROUND_FOREGROUND_MIN_LAYER_SLOT_RATIO) {
                    "切回前台内容层刷新槽位占用 ${restoreStats.presentationSlotRatio} 低于下限 $BACKGROUND_FOREGROUND_MIN_LAYER_SLOT_RATIO"
                }
            }
            mapOf(
                "restore_first_frame_millis" to restoreFirstFrameMillis.toDouble(),
                "rotation_first_frame_millis" to rotationFirstFrameMillis.toDouble(),
            )
        }
    }

    private fun awaitBackgrounded(device: UiDevice, packageName: String, timeoutMillis: Long): Boolean {
        val deadline = SystemClock.elapsedRealtime() + timeoutMillis
        while (SystemClock.elapsedRealtime() < deadline) {
            if (!isAppInForeground(device, packageName)) return true
            SystemClock.sleep(SUSTAINED_OUTPUT_POLL_MILLIS)
        }
        return !isAppInForeground(device, packageName)
    }

    private data class RotationSettings(val accelerometer: String?, val userRotation: String?)

    private fun readRotationSettings(device: UiDevice): RotationSettings = RotationSettings(
        accelerometer = device.executeShellCommand("settings get system accelerometer_rotation").trim().takeIf { it != "null" },
        userRotation = device.executeShellCommand("settings get system user_rotation").trim().takeIf { it != "null" },
    )

    private fun setRotation(device: UiDevice, rotation: Int) {
        device.executeShellCommand("settings put system accelerometer_rotation 0")
        device.executeShellCommand("settings put system user_rotation $rotation")
    }

    private fun awaitDisplayRotation(device: UiDevice, expected: Int) {
        val expectedToken = "ROTATION_${expected * 90}"
        val deadline = SystemClock.elapsedRealtime() + 5_000L
        while (SystemClock.elapsedRealtime() < deadline) {
            val dump = device.executeShellCommand("dumpsys window")
            if (dump.contains("mCurrentRotation=$expectedToken")) return
            SystemClock.sleep(SUSTAINED_OUTPUT_POLL_MILLIS)
        }
        check(false) { "显示旋转未到达 $expectedToken" }
    }

    private fun restoreRotationSettings(device: UiDevice, settings: RotationSettings) {
        device.executeShellCommand("settings put system accelerometer_rotation ${settings.accelerometer ?: "1"}")
        device.executeShellCommand("settings put system user_rotation ${settings.userRotation ?: "0"}")
    }
}
