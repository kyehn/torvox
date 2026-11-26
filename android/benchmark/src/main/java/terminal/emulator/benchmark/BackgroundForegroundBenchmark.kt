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
        ) { _, budget ->
            val scenarioStartElapsedMillis = SystemClock.elapsedRealtime()
            // S11 是场景矩阵项，预算必然存在。
            val scenarioBudget = checkNotNull(budget) { "S11 必须带场景预算" }
            // 切后台：home 键，等焦点离开。
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
            // 内容层随界面回到前台重建：按属主重新找，不认旧图层名；首帧取重建之后的呈现，
            // 旧图层帧历史里重建之前的帧不算。
            val (restoredLayer, restoreStats) = checkNotNull(
                awaitContentLayerPresentAfter(
                    device = device,
                    packageName = TARGET_PACKAGE,
                    sinceElapsedMillis = restoreStartedAtMillis,
                    timeoutMillis = RESTORE_FIRST_FRAME_TIMEOUT_MILLIS,
                    pollMillis = SUSTAINED_OUTPUT_POLL_MILLIS,
                ),
            ) { "切回前台后内容层没有出帧" }
            val restoreFirstFrameMillis = (
                checkNotNull(firstPresentMillisAfter(restoreStats, restoreStartedAtMillis)) {
                    "切回前台后内容层没有帧记录：图层 $restoredLayer"
                } - restoreStartedAtMillis
                ).toLong()
            // 旋转。本应用在清单里声明了 `configChanges`，旋转不重建 Activity，SurfaceView 的
            // Surface 也不重建（实测旋转前后图层名不变），变的是表面尺寸：系统转到横屏后应用
            // 按新尺寸重画。故计时起点取「系统确认转到横屏」的时刻，不是「发起旋转」的时刻。
            val originalRotation = readRotationSettings(device)
            setRotation(device, 1)
            val rotatedAtMillis = awaitDisplayRotation(device, ROTATION_LANDSCAPE_DISPLAY, scenarioBudget)
            val (rotatedLayer, rotationStats) = checkNotNull(
                awaitContentLayerPresentAfter(
                    device = device,
                    packageName = TARGET_PACKAGE,
                    sinceElapsedMillis = rotatedAtMillis,
                    timeoutMillis = ROTATION_FIRST_FRAME_TIMEOUT_MILLIS,
                    pollMillis = SUSTAINED_OUTPUT_POLL_MILLIS,
                ),
            ) { "旋转后内容层没有出帧" }
            val rotationFirstFrameMillis = (
                checkNotNull(firstPresentMillisAfter(rotationStats, rotatedAtMillis)) {
                    "旋转后内容层没有帧记录：图层 $rotatedLayer"
                } - rotatedAtMillis
                ).toLong()
            restoreRotationSettings(device, originalRotation)
            val elapsedMillis = SystemClock.elapsedRealtime() - scenarioStartElapsedMillis
            printMeasured("backgroundForeground", elapsedMillis, readFrameStats(device))
            emitMeasured(
                measuredLinePrefix("backgroundForeground", elapsedMillis) +
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
            check(restoreStats.presentationSlotRatio >= BACKGROUND_FOREGROUND_MIN_LAYER_SLOT_RATIO) {
                "切回前台内容层刷新槽位占用 ${restoreStats.presentationSlotRatio} 低于下限 $BACKGROUND_FOREGROUND_MIN_LAYER_SLOT_RATIO"
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
        accelerometer = device.executeShellCommand("settings get system accelerometer_rotation").trim().takeIf {
            it !=
                "null"
        },
        userRotation = device.executeShellCommand("settings get system user_rotation").trim().takeIf { it != "null" },
    )

    private fun setRotation(device: UiDevice, rotation: Int) {
        device.executeShellCommand("settings put system accelerometer_rotation 0")
        device.executeShellCommand("settings put system user_rotation $rotation")
    }

    /** 等显示转到指定方向，返回确认到的时刻（单调时钟毫秒）；超时即失败。 */
    private fun awaitDisplayRotation(device: UiDevice, expectedRotation: String, budget: ScenarioBudget): Long {
        val deadline = SystemClock.elapsedRealtime() + minOf(5_000L, budget.remainingMillis())
        while (SystemClock.elapsedRealtime() < deadline) {
            budget.checkDeadline()
            val rotation = parseDisplayRotation(device.executeShellCommand("dumpsys window"))
            if (rotation == expectedRotation) return SystemClock.elapsedRealtime()
            SystemClock.sleep(SUSTAINED_OUTPUT_POLL_MILLIS)
        }
        error("显示旋转未到达 $expectedRotation")
    }

    private fun restoreRotationSettings(device: UiDevice, settings: RotationSettings) {
        device.executeShellCommand("settings put system accelerometer_rotation ${settings.accelerometer ?: "1"}")
        device.executeShellCommand("settings put system user_rotation ${settings.userRotation ?: "0"}")
    }
}

/** `dumpsys window` 里显示当前方向的前缀。 */
private const val CURRENT_ROTATION_MARKER = "mCurrentRotation="

/**
 * 解析显示的当前方向：`mCurrentRotation=ROTATION_90` 行给出 `ROTATION_90`。
 *
 * 取显示方向而不是用户方向设置：设置值只说明「要求转到哪」，显示方向才说明「系统已经
 * 转过去了」——旋转动画期间两者是不同步的。
 */
internal fun parseDisplayRotation(dump: String): String? = dump.lineSequence()
    .map { it.trim() }
    .firstOrNull { it.startsWith(CURRENT_ROTATION_MARKER) }
    ?.substringAfter(CURRENT_ROTATION_MARKER)
