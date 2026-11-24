package terminal.emulator.benchmark

import android.os.SystemClock
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import androidx.test.uiautomator.UiDevice
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith

/** 切后台后等焦点离开被测应用的上限。 */
private const val CROSS_APP_BACKGROUND_TIMEOUT_MILLIS = 5_000L

/** 等显示方向转过去的上限。 */
private const val CROSS_APP_ROTATION_TIMEOUT_MILLIS = 5_000L

/** 切后台后的稳定等待：刚按 HOME 时焦点状态的短暂空档会把启动态判成 UNKNOWN。 */
private const val CROSS_APP_BACKGROUND_SETTLE_MILLIS = 1_000L

/**
 * 跨应用终端场景：同一负载、同一网格、同一系统侧通道，测任意被测包。
 *
 * 与 [CrossAppLaunchBenchmark] 互补：那条测启动，这条测终端内容面的稳态与输出。
 * 负载经 `input text` 敲进终端，完成信号由应用的 shell 写到 /sdcard 上的文件——不读
 * 被测应用的 logcat，也不依赖应用内 API，故两个被测应用拿到完全相同的命令。
 *
 * 包名撞名的处置见研究报告：同一台设备上 `com.termux` 只能装一个应用，故被测包按应用
 * 轮换安装；承载仪器测试的锚点应用是本仓的 nix 变种（包名 `com.termux.nix`），始终在位。
 *
 * 判定只落在与具体应用无关的事实上：负载完整跑完、内容层确实出帧、网格与参照一致。
 * 帧率与耗时的横向关系由宿主侧按包名分组后比较，不在单次运行里对某个应用下 SLO 结论。
 */
@RunWith(AndroidJUnit4::class)
class CrossAppTerminalBenchmark {
    @get:Rule
    val environmentPinning = EnvironmentPinningRule(
        UiDevice.getInstance(InstrumentationRegistry.getInstrumentation()),
    )

    @Test
    fun hotStartAcrossApps() {
        forEachTarget("crossAppHotStart") { device, packageName, store, paths ->
            measureHotStart(device, packageName, store, paths)
        }
    }

    @Test
    fun idleAcrossApps() {
        forEachTarget("crossAppIdle") { device, packageName, store, paths ->
            measureIdle(device, packageName, store, paths)
        }
    }

    @Test
    fun sustainedOutputAcrossApps() {
        forEachTarget("crossAppSustainedOutput") { device, packageName, store, paths ->
            measureOutput(
                device = device,
                packageName = packageName,
                store = store,
                paths = paths,
                label = "crossAppSustainedOutput",
                scenarioId = "S5",
                load = {
                    sustainedOutputCommand(
                        paths = paths,
                        batchCount = SUSTAINED_OUTPUT_BATCH_COUNT,
                        linesPerBatch = SUSTAINED_OUTPUT_LINES_PER_BATCH,
                        pauseSeconds = SUSTAINED_OUTPUT_BATCH_PAUSE_SECONDS,
                    )
                },
                deliveredBytes = SustainedOutputLoad(
                    SUSTAINED_OUTPUT_BATCH_COUNT,
                    SUSTAINED_OUTPUT_LINES_PER_BATCH,
                    SUSTAINED_OUTPUT_BATCH_PAUSE_SECONDS,
                ).bytesPerBatch() * SUSTAINED_OUTPUT_BATCH_COUNT,
                timeoutMillis = SUSTAINED_OUTPUT_TIMEOUT_MILLIS,
            )
        }
    }

    @Test
    fun wideCharacterAcrossApps() {
        forEachTarget("crossAppWideCharacter") { device, packageName, store, paths ->
            measureOutput(
                device = device,
                packageName = packageName,
                store = store,
                paths = paths,
                label = "crossAppWideCharacter",
                scenarioId = "S8",
                load = {
                    WideCharacterLoad(
                        lineCount = WIDE_CHARACTER_LINE_COUNT,
                        canary = WIDE_CHARACTER_CANARY,
                        completion = "printf $CROSS_APP_DONE_MARKER > ${paths.donePath}",
                    ).command()
                },
                deliveredBytes = WideCharacterLoad(WIDE_CHARACTER_LINE_COUNT, WIDE_CHARACTER_CANARY).bytes(),
                timeoutMillis = WIDE_CHARACTER_TIMEOUT_MILLIS,
            )
        }
    }

    @Test
    fun backgroundForegroundAcrossApps() {
        forEachTarget("crossAppBackgroundForeground") { device, packageName, store, paths ->
            measureBackgroundForeground(device, packageName, store, paths)
        }
    }

    /** 逐个被测包跑同一场景：每个包独立准备、独立落行，行按包名分组即横向对比数据。 */
    private fun forEachTarget(
        label: String,
        scenario: (UiDevice, String, MeasurementStore, CrossAppLoadPaths) -> Unit,
    ) {
        val device = UiDevice.getInstance(InstrumentationRegistry.getInstrumentation())
        val store = openMeasurementStore(label)
        val paths = CrossAppLoadPaths(store.runId)
        readTargetPackages().forEach { packageName ->
            scenario(device, packageName, store, paths)
        }
    }

    /** S3：进程存活切前台，记系统侧总耗时与内容层首帧。 */
    private fun measureHotStart(
        device: UiDevice,
        packageName: String,
        store: MeasurementStore,
        paths: CrossAppLoadPaths,
    ) {
        val launcherActivity = readLauncherActivity(device, packageName)
        measureScenario(
            device = device,
            label = "crossAppHotStart",
            store = store,
            frameworkIteration = null,
            measured = true,
            scenario = scenarioById("S3"),
            packageName = packageName,
            setup = {
                grantRuntimePermissions(device, packageName)
                forceStop(device, packageName)
                startAndWait(device, launcherActivity)
                awaitContentLayerOwnedBy(device, packageName)
                awaitTerminalShellReady(device, paths, SHELL_READY_TIMEOUT_MILLIS)
            },
        ) { _, _ ->
            device.pressHome()
            check(awaitBackgrounded(device, packageName)) {
                "切后台后焦点没有离开 $packageName"
            }
            SystemClock.sleep(CROSS_APP_BACKGROUND_SETTLE_MILLIS)
            val startedElapsedMillis = SystemClock.elapsedRealtime()
            val result = startAndWait(device, launcherActivity)
            check(result.launchState == "HOT" || result.launchState == "WARM") {
                "切回前台 launchState=${result.launchState}，预期 HOT/WARM"
            }
            val firstFrameMillis = firstFrameAfter(
                device = device,
                packageName = packageName,
                sinceElapsedMillis = startedElapsedMillis,
                timeoutMillis = RESTORE_FIRST_FRAME_TIMEOUT_MILLIS,
            )
            val grid = readTerminalGrid(device, paths)
            emitMeasured(
                "[bench-measure] crossAppHotStart package=$packageName grid=${grid.describe()} " +
                    "totalTime=${result.totalTimeMillis}ms firstFrame=${firstFrameMillis}ms",
            )
            mapOf(
                "hot_start_total_time_millis" to requireNotNull(result.totalTimeMillis).toDouble(),
                "first_frame_millis" to firstFrameMillis.toDouble(),
                "grid_rows" to grid.rows.toDouble(),
                "grid_columns" to grid.columns.toDouble(),
            )
        }
    }

    /** S4：静置十秒，记内容层帧数、进程 CPU 时间与 PSS。 */
    private fun measureIdle(device: UiDevice, packageName: String, store: MeasurementStore, paths: CrossAppLoadPaths) {
        val launcherActivity = readLauncherActivity(device, packageName)
        var contentLayer = ""
        var refreshPeriodNanos = 0L
        measureScenario(
            device = device,
            label = "crossAppIdle",
            store = store,
            frameworkIteration = null,
            measured = true,
            scenario = scenarioById("S4"),
            packageName = packageName,
            setup = {
                grantRuntimePermissions(device, packageName)
                forceStop(device, packageName)
                startAndWait(device, launcherActivity)
                contentLayer = awaitContentLayerOwnedBy(device, packageName)
                awaitTerminalShellReady(device, paths, SHELL_READY_TIMEOUT_MILLIS)
                refreshPeriodNanos = checkNotNull(readLayerFrameStats(device, contentLayer)) {
                    "内容层没有帧记录：$contentLayer"
                }.refreshPeriodNanos
                SystemClock.sleep(IDLE_SETTLE_MILLIS)
            },
        ) { _, budget ->
            val cpuBefore = readProcessCpuTime(device, packageName)
            val windowStartElapsedMillis = SystemClock.elapsedRealtime()
            val presents = sampleLayerPresents(
                device = device,
                layerName = contentLayer,
                windowMillis = IDLE_WINDOW_MILLIS,
                pollMillis = SUSTAINED_OUTPUT_POLL_MILLIS,
                budget = budget,
            )
            val windowEndElapsedMillis = SystemClock.elapsedRealtime()
            val window = layerWindowStats(
                presentTimesNanos = presents,
                windowStartNanos = windowStartElapsedMillis * 1_000_000L,
                windowEndNanos = windowEndElapsedMillis * 1_000_000L,
                refreshPeriodNanos = refreshPeriodNanos,
            )
            val cpuAfter = readProcessCpuTime(device, packageName)
            val memory = readDeviceMemory(device, packageName)
            val grid = readTerminalGrid(device, paths)
            emitMeasured(
                "[bench-measure] crossAppIdle package=$packageName grid=${grid.describe()} " +
                    "frames=${window.frameCount} slotRatio=${window.presentationSlotRatio} " +
                    "processCpu=${cpuAfter.totalMillis - cpuBefore.totalMillis}ms pss=${memory.pssKilobytes}KB",
            )
            mapOf(
                "idle_frames" to window.frameCount.toDouble(),
                "idle_slot_ratio" to window.presentationSlotRatio,
                "process_cpu_millis" to (cpuAfter.totalMillis - cpuBefore.totalMillis).toDouble(),
                "pss_kilobytes" to memory.pssKilobytes.toDouble(),
                "grid_rows" to grid.rows.toDouble(),
                "grid_columns" to grid.columns.toDouble(),
            )
        }
    }

    /** S5/S8：同一输出负载，记负载时长、内容层帧面、进程 CPU 与 PSS 峰值。 */
    private fun measureOutput(
        device: UiDevice,
        packageName: String,
        store: MeasurementStore,
        paths: CrossAppLoadPaths,
        label: String,
        scenarioId: String,
        load: () -> String,
        deliveredBytes: Long,
        timeoutMillis: Long,
    ) {
        val launcherActivity = readLauncherActivity(device, packageName)
        var contentLayer = ""
        measureScenario(
            device = device,
            label = label,
            store = store,
            frameworkIteration = null,
            measured = true,
            scenario = scenarioById(scenarioId),
            packageName = packageName,
            setup = {
                grantRuntimePermissions(device, packageName)
                forceStop(device, packageName)
                startAndWait(device, launcherActivity)
                contentLayer = awaitContentLayerOwnedBy(device, packageName)
                awaitTerminalShellReady(device, paths, SHELL_READY_TIMEOUT_MILLIS)
            },
        ) { _, budget ->
            deleteDeviceFile(device, paths.donePath)
            // 刷新周期先读后清：清空帧历史后立刻再读会读到空记录（还没有新帧）。
            val layerBefore = checkNotNull(readLayerFrameStats(device, contentLayer)) {
                "内容层没有帧记录：$contentLayer"
            }
            clearLayerFrameHistory(device, contentLayer)
            val cpuBefore = readProcessCpuTime(device, packageName)
            val baselineMemory = readDeviceMemory(device, packageName)
            val grid = readTerminalGrid(device, paths)
            val loadStartElapsedMillis = SystemClock.elapsedRealtime()
            typeTerminalCommand(device, load())
            val sample = sampleLayerPresentsUntilMarker(
                device = device,
                layerName = contentLayer,
                paths = paths,
                timeoutMillis = timeoutMillis,
                pollMillis = SUSTAINED_OUTPUT_POLL_MILLIS,
                budget = budget,
            )
            check(sample.completed) { "$label 负载未完成：超时内没有写出完成标记" }
            val loadEndElapsedMillis = SystemClock.elapsedRealtime()
            val cpuAfter = readProcessCpuTime(device, packageName)
            val memory = readDeviceMemory(device, packageName)
            val window = layerWindowStats(
                presentTimesNanos = sample.presentTimesNanos,
                windowStartNanos = loadStartElapsedMillis * 1_000_000L,
                windowEndNanos = loadEndElapsedMillis * 1_000_000L,
                refreshPeriodNanos = layerBefore.refreshPeriodNanos,
            )
            check(window.frameCount > 0) { "$label 负载期间内容层没有呈现任何帧" }
            val artifacts = mutableListOf<ArtifactRef>()
            val screenshot = captureScreenshotArtifact(device, store, "$label-$packageName.png", artifacts)
            emitMeasured(
                "[bench-measure] $label package=$packageName grid=${grid.describe()} " +
                    "elapsed=${loadEndElapsedMillis - loadStartElapsedMillis}ms bytes=$deliveredBytes " +
                    "frames=${window.frameCount} p95=${window.p95IntervalMillis}ms " +
                    "slotRatio=${window.presentationSlotRatio} " +
                    "processCpu=${cpuAfter.totalMillis - cpuBefore.totalMillis}ms " +
                    "pssGrowth=${memory.pssKilobytes - baselineMemory.pssKilobytes}KB " +
                    "screenshot=${screenshot.path}",
            )
            mapOf(
                "delivered_bytes" to deliveredBytes.toDouble(),
                "elapsed_millis" to (loadEndElapsedMillis - loadStartElapsedMillis).toDouble(),
                "presented_frames" to window.frameCount.toDouble(),
                "layer_interval_p95_millis" to window.p95IntervalMillis.toDouble(),
                "layer_slot_ratio" to window.presentationSlotRatio,
                "process_cpu_millis" to (cpuAfter.totalMillis - cpuBefore.totalMillis).toDouble(),
                "pss_growth_kilobytes" to (memory.pssKilobytes - baselineMemory.pssKilobytes).toDouble(),
                "grid_rows" to grid.rows.toDouble(),
                "grid_columns" to grid.columns.toDouble(),
            )
        }
    }

    /** S11：切后台再切回、再旋转，记恢复首帧与旋转首帧。 */
    private fun measureBackgroundForeground(
        device: UiDevice,
        packageName: String,
        store: MeasurementStore,
        paths: CrossAppLoadPaths,
    ) {
        val launcherActivity = readLauncherActivity(device, packageName)
        measureScenario(
            device = device,
            label = "crossAppBackgroundForeground",
            store = store,
            frameworkIteration = null,
            measured = true,
            scenario = scenarioById("S11"),
            packageName = packageName,
            setup = {
                grantRuntimePermissions(device, packageName)
                forceStop(device, packageName)
                startAndWait(device, launcherActivity)
                awaitContentLayerOwnedBy(device, packageName)
                awaitTerminalShellReady(device, paths, SHELL_READY_TIMEOUT_MILLIS)
            },
        ) { _, budget ->
            val scenarioBudget = checkNotNull(budget) { "S11 必须带场景预算" }
            device.pressHome()
            check(awaitBackgrounded(device, packageName)) {
                "切后台后焦点没有离开 $packageName"
            }
            SystemClock.sleep(CROSS_APP_BACKGROUND_SETTLE_MILLIS)
            val restoreStartedElapsedMillis = SystemClock.elapsedRealtime()
            val launchResult = startAndWait(device, launcherActivity)
            check(launchResult.launchState == "HOT" || launchResult.launchState == "WARM") {
                "切回前台 launchState=${launchResult.launchState}，预期 HOT/WARM"
            }
            val restoreFirstFrameMillis = firstFrameAfter(
                device = device,
                packageName = packageName,
                sinceElapsedMillis = restoreStartedElapsedMillis,
                timeoutMillis = RESTORE_FIRST_FRAME_TIMEOUT_MILLIS,
            )
            val originalAccelerometer = readSetting(device, "accelerometer_rotation")
            val originalUserRotation = readSetting(device, "user_rotation")
            setRotation(device, 1)
            val rotatedElapsedMillis = awaitDisplayRotation(device, ROTATION_LANDSCAPE_DISPLAY, scenarioBudget)
            val rotationFirstFrameMillis = firstFrameAfter(
                device = device,
                packageName = packageName,
                sinceElapsedMillis = rotatedElapsedMillis,
                timeoutMillis = ROTATION_FIRST_FRAME_TIMEOUT_MILLIS,
            )
            restoreRotationSettings(device, originalAccelerometer, originalUserRotation)
            val grid = readTerminalGrid(device, paths)
            emitMeasured(
                "[bench-measure] crossAppBackgroundForeground package=$packageName grid=${grid.describe()} " +
                    "restoreFirstFrame=${restoreFirstFrameMillis}ms " +
                    "rotationFirstFrame=${rotationFirstFrameMillis}ms",
            )
            mapOf(
                "restore_first_frame_millis" to restoreFirstFrameMillis.toDouble(),
                "rotation_first_frame_millis" to rotationFirstFrameMillis.toDouble(),
                "grid_rows" to grid.rows.toDouble(),
                "grid_columns" to grid.columns.toDouble(),
            )
        }
    }

    /** 等内容层在 [sinceElapsedMillis] 之后呈现首帧，返回相对该时刻的毫秒数。 */
    private fun firstFrameAfter(
        device: UiDevice,
        packageName: String,
        sinceElapsedMillis: Long,
        timeoutMillis: Long,
    ): Long {
        val (layer, stats) = checkNotNull(
            awaitContentLayerPresentAfter(
                device = device,
                packageName = packageName,
                sinceElapsedMillis = sinceElapsedMillis,
                timeoutMillis = timeoutMillis,
                pollMillis = SUSTAINED_OUTPUT_POLL_MILLIS,
            ),
        ) { "$packageName 在 ${sinceElapsedMillis}ms 之后没有内容层出帧" }
        val firstPresentMillis = checkNotNull(firstPresentMillisAfter(stats, sinceElapsedMillis)) {
            "内容层没有帧记录：$layer"
        }
        return firstPresentMillis.toLong() - sinceElapsedMillis
    }

    /** 焦点是否已离开被测应用；超时仍在前台即判失败。 */
    private fun awaitBackgrounded(device: UiDevice, packageName: String): Boolean {
        val deadline = SystemClock.elapsedRealtime() + CROSS_APP_BACKGROUND_TIMEOUT_MILLIS
        while (SystemClock.elapsedRealtime() < deadline) {
            if (!isAppInForeground(device, packageName)) return true
            SystemClock.sleep(SUSTAINED_OUTPUT_POLL_MILLIS)
        }
        return !isAppInForeground(device, packageName)
    }

    /** 读一个系统设置项；未设置时返回 null。 */
    private fun readSetting(device: UiDevice, name: String): String? =
        device.executeShellCommand("settings get system $name").trim().takeIf { it != "null" }

    /** 固定屏幕方向：关掉重力感应并把用户方向设为目标值。 */
    private fun setRotation(device: UiDevice, rotation: Int) {
        device.executeShellCommand("settings put system accelerometer_rotation 0")
        device.executeShellCommand("settings put system user_rotation $rotation")
    }

    /** 等显示转到指定方向，返回确认到的时刻（单调时钟毫秒）；超时即失败。 */
    private fun awaitDisplayRotation(device: UiDevice, expectedRotation: String, budget: ScenarioBudget): Long {
        val deadline = SystemClock.elapsedRealtime() +
            minOf(CROSS_APP_ROTATION_TIMEOUT_MILLIS, budget.remainingMillis())
        while (SystemClock.elapsedRealtime() < deadline) {
            budget.checkDeadline()
            if (parseDisplayRotation(device.executeShellCommand("dumpsys window")) == expectedRotation) {
                return SystemClock.elapsedRealtime()
            }
            SystemClock.sleep(SUSTAINED_OUTPUT_POLL_MILLIS)
        }
        error("显示旋转未到达 $expectedRotation")
    }

    /** 复原方向设置：原先未设置时写回系统默认值。 */
    private fun restoreRotationSettings(device: UiDevice, accelerometer: String?, userRotation: String?) {
        device.executeShellCommand("settings put system accelerometer_rotation ${accelerometer ?: "1"}")
        device.executeShellCommand("settings put system user_rotation ${userRotation ?: "0"}")
    }
}
