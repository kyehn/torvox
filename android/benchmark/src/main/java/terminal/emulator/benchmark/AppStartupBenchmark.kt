package terminal.emulator.benchmark

import android.os.SystemClock
import androidx.benchmark.macro.FrameTimingMetric
import androidx.benchmark.macro.MacrobenchmarkScope
import androidx.benchmark.macro.StartupMode
import androidx.benchmark.macro.StartupTimingMetric
import androidx.benchmark.macro.junit4.MacrobenchmarkRule
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import androidx.test.uiautomator.UiDevice
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith

private const val MEASURE_ITERATION_COUNT = 10

/**
 * 一轮启动的实测值。
 *
 * 两段分开记：首帧由系统侧 `am start -W` 口径给出（`startActivityAndWait` 走
 * `am start`），终端界面可见由 UiAutomator 检出，二者受宿主负载的影响不同——
 * 实测宿主超额占用时检测段从空载的约 0.2s 涨到约 0.85s，检测段偏长即测量开销，
 * 不是应用慢。
 */
private data class StartupIteration(
    /** 启动到首帧（系统侧 `am start -W` 口径）。 */
    val launchMillis: Long,
    /** 首帧到终端界面元素可见（UiAutomator 检测口径）。 */
    val visibleMillis: Long,
) {
    /** 启动到终端可用的总耗时。 */
    val totalMillis: Long
        get() = launchMillis + visibleMillis
}

/**
 * 应用级宏基准：冷启动 / 热启动耗时。
 *
 * 命名为 AppStartupBenchmark 而非 Bridge*，因为测量对象是应用生命周期阶段，
 * 不是 JNI bridge 的单次调用开销；后者由 Rust bench 与 JNI 集成测试覆盖，
 * 详见 `DESIGN.md` 的性能条款。
 *
 * 默认 `CompilationMode.Partial(baselineProfileMode = Require)`，即带
 * `assets/dexopt/baseline.prof` 测量；若 profile 未安装会直接报错，
 * 因此这些数字反映的是安装 profile 后的真实表现。
 */
@RunWith(AndroidJUnit4::class)
class AppStartupBenchmark {
    @get:Rule
    val environmentPinning = EnvironmentPinningRule(
        UiDevice.getInstance(InstrumentationRegistry.getInstrumentation()),
    )

    @get:Rule
    val benchmarkRule = MacrobenchmarkRule()

    @Test
    fun coldStart() {
        val store = openMeasurementStore("coldStart")
        val iterations = mutableListOf<StartupIteration>()
        benchmarkRule.measureRepeated(
            packageName = TARGET_PACKAGE,
            metrics = listOf(StartupTimingMetric(), FrameTimingMetric()),
            iterations = MEASURE_ITERATION_COUNT,
            startupMode = StartupMode.COLD,
            setupBlock = {
                grantNotificationPermission(device)
                pressHome()
            },
            measureBlock = {
                measureScenario(device, "coldStart", store, iteration, scenarioById("S1")) { _ ->
                    // 冷启动前进程不在，清零帧计数是空操作：读数即进程启动以来的全部帧。
                    resetFrameStats(device)
                    // 启动耗时只有在界面真的出现时才有意义：首帧到了但被系统对话框
                    // 挡住时，指标测的是「弹窗出现」而不是「终端可用」。
                    val iteration = measureStartup()
                    iterations += iteration
                    val frames = readFrameStatsAfterRender(device)
                    printMeasured("coldStart", iteration.totalMillis, frames)
                    check(frames.frames >= MIN_ANIMATION_FRAME_COUNT) {
                        "冷启动未产生任何界面帧：frames=${frames.frames}"
                    }
                    emptyMap()
                }
            },
        )
        assertStartupBudget("coldStart", iterations, COLD_START_BUDGET_MILLIS, COLD_START_HARD_LIMIT_MILLIS)
    }

    @Test
    fun warmStart() {
        val store = openMeasurementStore("warmStart")
        val iterations = mutableListOf<StartupIteration>()
        benchmarkRule.measureRepeated(
            packageName = TARGET_PACKAGE,
            metrics = listOf(StartupTimingMetric()),
            iterations = MEASURE_ITERATION_COUNT,
            startupMode = StartupMode.WARM,
            setupBlock = {
                grantNotificationPermission(device)
                startActivityAndWait()
                device.waitForIdle()
                pressHome()
            },
            measureBlock = {
                measureScenario(device, "warmStart", store, iteration, scenarioById("S2")) { _ ->
                    resetFrameStats(device)
                    val iteration = measureStartup()
                    iterations += iteration
                    val frames = readFrameStatsAfterRender(device)
                    printMeasured("warmStart", iteration.totalMillis, frames)
                    check(frames.frames >= MIN_ANIMATION_FRAME_COUNT) {
                        "热启动未产生任何界面帧：frames=${frames.frames}"
                    }
                    emptyMap()
                }
            },
        )
        assertStartupBudget("warmStart", iterations, WARM_START_BUDGET_MILLIS, WARM_START_HARD_LIMIT_MILLIS)
    }

    /**
     * 热启动：进程存活，把已有界面切回前台（场景 S3）。
     *
     * 热启动不新建 Activity，宏基准自带的 `startActivityAndWait` 认不出这次启动：系统报
     * 「Activity not started」，框架便退化成固定等两秒再返回（实测每轮都是 2s 起），量到的
     * 是那两秒而不是应用。故这里自己走系统侧 `am start -W`：TotalTime 与冷启动同口径
     * （跨应用可比），启动态顺带证明这轮确实是热启动而非冷启动或温启动。
     *
     * 终端内容层的 Surface 随界面回到前台重新创建：图层在窗口开始前并不存在，故窗口
     * 结束后取名读到的帧全部落在本轮。取名 MUST 放在启动之后——按 home 后图层已被销毁，
     * 窗口前取名会直接失败。
     */
    @Test
    fun hotStart() {
        val device = UiDevice.getInstance(InstrumentationRegistry.getInstrumentation())
        val store = openMeasurementStore("hotStart")
        val iterations = mutableListOf<StartupIteration>()
        val launcherActivity = readLauncherActivity(device, TARGET_PACKAGE)
        benchmarkRule.measureRepeated(
            packageName = TARGET_PACKAGE,
            metrics = listOf(FrameTimingMetric()),
            iterations = MEASURE_ITERATION_COUNT,
            startupMode = StartupMode.HOT,
            setupBlock = {
                grantNotificationPermission(device)
                startAndWait(device, launcherActivity)
                device.waitForIdle()
                pressHome()
            },
            measureBlock = {
                measureScenario(device, "hotStart", store, iteration, scenarioById("S3")) { _ ->
                    resetFrameStats(device)
                    val launch = startAndWait(device, launcherActivity)
                    check(launch.launchState == "HOT") {
                        "热启动这轮的启动态不是 HOT（量到的不是切前台）：${launch.launchState}"
                    }
                    val launchMillis = checkNotNull(launch.totalTimeMillis) {
                        "系统侧没测到这次热启动：$launch"
                    }
                    val visibleMillis = elapsedMillis { assertTerminalVisible(device) }
                    val iteration = StartupIteration(launchMillis, visibleMillis)
                    iterations += iteration
                    val frames = readFrameStatsAfterRender(device)
                    val (contentLayer, layerStats) = awaitTerminalContentFrames(device)
                    // 实测值先于断言输出：断言失败时构建产物里仍留得下这轮的数字。
                    printMeasured("hotStart", iteration.totalMillis, frames)
                    check(frames.frames >= MIN_ANIMATION_FRAME_COUNT) {
                        "热启动未产生任何界面帧：frames=${frames.frames}"
                    }
                    check(layerStats.contentFrameCount >= MIN_ANIMATION_FRAME_COUNT) {
                        "热启动窗口内终端内容层没有帧记录：图层 $contentLayer"
                    }
                    mapOf(
                        "launch_millis" to iteration.launchMillis.toDouble(),
                        "visible_millis" to iteration.visibleMillis.toDouble(),
                        "total_millis" to iteration.totalMillis.toDouble(),
                        "gfxinfo_frames" to frames.frames.toDouble(),
                        "layer_content_frames" to layerStats.contentFrameCount.toDouble(),
                    )
                }
            },
        )
        assertStartupBudget("hotStart", iterations, HOT_START_BUDGET_MILLIS, HOT_START_HARD_LIMIT_MILLIS)
    }

    /**
     * 等终端内容层出帧：图层随界面回到前台重建，出帧比图层出现晚。
     *
     * 等待有上限：等不到即判失败，不把「没出帧」当成通过。
     */
    private fun awaitTerminalContentFrames(device: UiDevice): Pair<String, LayerFrameStats> {
        var failure: Throwable? = null
        repeat(CONTENT_FRAME_WAIT_ATTEMPT_COUNT) {
            try {
                val contentLayer = findTerminalContentLayer(device)
                val stats = readLayerFrameStats(device, contentLayer)
                if (stats != null && stats.contentFrameCount > 0) return contentLayer to stats
                failure = IllegalStateException("终端内容层还没有内容帧：图层 $contentLayer")
            } catch (thrown: Throwable) {
                failure = thrown
            }
            SystemClock.sleep(CONTENT_FRAME_WAIT_INTERVAL_MILLIS)
        }
        throw checkNotNull(failure) { "终端内容层始终没有出现" }
    }

    /** 测一轮启动：先等首帧，再等终端界面元素真的出现在界面上。 */
    private fun MacrobenchmarkScope.measureStartup(): StartupIteration {
        val launchMillis = elapsedMillis { startActivityAndWait() }
        val visibleMillis = elapsedMillis { assertTerminalVisible(device) }
        return StartupIteration(launchMillis, visibleMillis)
    }
}

/** 终端界面必须真的出现；只测到「进程被拉起」而界面没到位，启动耗时没有意义。 */
private fun assertTerminalVisible(device: UiDevice) {
    requireElement(device, TERMINAL_SCREEN_RESOURCE_ID, "终端界面")
}

/**
 * 启动耗时的判定：各轮中位数对预算，最差一轮对硬上限。
 *
 * 预算是按实测中位数校准的，判定也取中位数：宿主被并发任务抢占时整轮分布都会变慢
 * （实测同一构建在超额占用的宿主上冷启动十轮中位数从 2.0s 涨到 3.5s、峰值到 4.5s），
 * 拿最差一轮当判据会把宿主抖动算成应用回归。最差一轮另过硬上限，拦的是「一轮就慢到
 * 不可用」。中位数取排序后的上中位数（轮数为偶数时取靠后的那个），偏保守。
 */
private fun assertStartupBudget(
    label: String,
    iterations: List<StartupIteration>,
    budgetMillis: Long,
    hardLimitMillis: Long,
) {
    check(iterations.isNotEmpty()) { "$label 没有测到任何迭代" }
    val totals = iterations.map { it.totalMillis }.sorted()
    val median = totals[totals.size / 2]
    emitMeasured(
        "[bench-measure] $label-iterations count=${totals.size} median=${median}ms " +
            "min=${totals.first()}ms max=${totals.last()}ms " +
            "launchMedian=${iterations.map { it.launchMillis }.sorted()[iterations.size / 2]}ms " +
            "visibleMedian=${iterations.map { it.visibleMillis }.sorted()[iterations.size / 2]}ms",
    )
    check(median <= budgetMillis) {
        "$label 各轮耗时中位数 ${median}ms 超过预算 ${budgetMillis}ms（各轮：$totals）"
    }
    check(totals.last() <= hardLimitMillis) {
        "$label 最差一轮 ${totals.last()}ms 超过硬上限 ${hardLimitMillis}ms（各轮：$totals）"
    }
}
