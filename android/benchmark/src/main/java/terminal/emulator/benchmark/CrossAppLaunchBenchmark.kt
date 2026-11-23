package terminal.emulator.benchmark

import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import androidx.test.uiautomator.UiDevice
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith

/** 每个被测包的冷启动轮数。 */
private const val CROSS_APP_ITERATION_COUNT = 5
/**
 * 跨应用的启动对比面：同一 `am start -W` 口径测任意包的冷启动。
 *
 * 对照对象与本项目不能用各自的启动指标（宏基准指标量的是各目标自身管线的耗时，跨应用
 * 不可比），故统一取系统侧 `am start -W` 的 TotalTime：口径与被测应用无关，可直接横向比。
 *
 * 待测包从仪器参数 `targetPackages` 取（逗号分隔），默认只测本项目。宿主侧轮换安装各
 * 被测包后分别跑本用例，落盘的行按包名分组即是横向对比数据；同一包名只能装一个应用，
 * 故同设备上多包并存时须改包名或轮换安装（见研究报告的「包名撞名的处置」）。
 */
@RunWith(AndroidJUnit4::class)
class CrossAppLaunchBenchmark {
    @get:Rule
    val environmentPinning = EnvironmentPinningRule(
        UiDevice.getInstance(InstrumentationRegistry.getInstrumentation()),
    )

    @Test
    fun coldStartAcrossApps() {
        val device = UiDevice.getInstance(InstrumentationRegistry.getInstrumentation())
        val store = openMeasurementStore("crossAppLaunch")
        readTargetPackages().forEach { packageName -> measureColdStarts(device, packageName, store) }
    }

    /**
     * 测一个包的冷启动：每轮强停后拉起，逐轮记录系统侧耗时与环境。
     *
     * 启动被别的应用接管（权限弹窗等）即判失败：测到的不是被测应用的启动耗时。
     */
    private fun measureColdStarts(device: UiDevice, packageName: String, store: MeasurementStore) {
        val launcherActivity = readLauncherActivity(device, packageName)
        // 预授权限：权限弹窗会抢走启动（系统侧报的是弹窗自己的 Activity）。
        val deniedPermissions = grantRuntimePermissions(device, packageName)
        emitMeasured("[bench-perms] package=$packageName denied=${deniedPermissions.joinToString(",")}")
        // 预热一次：安装后的首次启动含 dex 优化与首次初始化，把它算进测量会污染对比数据。
        forceStop(device, packageName)
        val prewarmStartedAtMillis = System.currentTimeMillis()
        try {
            launchChecked(device, packageName, launcherActivity)
        } catch (thrown: Throwable) {
            // 预热失败也要归因：应用启动即崩溃时退出记录里有原因。
            throw thrown.withProcessExitAttribution(device, prewarmStartedAtMillis, packageName)
        }
        val totalTimes = mutableListOf<Long>()
        val processCpuTimes = mutableListOf<Long>()
        repeat(CROSS_APP_ITERATION_COUNT) {
            measureScenario(
                device = device,
                label = "crossAppLaunch",
                store = store,
                frameworkIteration = null,
                scenario = scenarioById("S1"),
                packageName = packageName,
                measured = true,
                note = "deniedRuntimePermissions=${deniedPermissions.joinToString(",")}",
                setup = { forceStop(device, packageName) },
            ) { iteration, _ ->
                val result = launchChecked(device, packageName, launcherActivity)
                check(result.launchState == "COLD" && result.totalTimeMillis != null) {
                    "系统没测到这次冷启动：启动态=${result.launchState} " +
                        "总耗时=${result.totalTimeMillis}ms 等待=${result.waitTimeMillis}ms"
                }
                val totalTimeMillis = requireNotNull(result.totalTimeMillis)
                val processCpuTime = readProcessCpuTime(device, packageName)
                totalTimes += totalTimeMillis
                processCpuTimes += processCpuTime.totalMillis
                emitMeasured(
                    "[bench-measure] crossAppLaunch package=$packageName iteration=$iteration " +
                        "totalTime=${totalTimeMillis}ms waitTime=${result.waitTimeMillis}ms " +
                        "launchState=${result.launchState} processCpu=${processCpuTime.totalMillis}ms " +
                        "processAge=${processCpuTime.ageMillis}ms",
                )
                mapOf(
                    "total_time_millis" to totalTimeMillis.toDouble(),
                    "wait_time_millis" to result.waitTimeMillis.toDouble(),
                    "process_cpu_millis" to processCpuTime.totalMillis.toDouble(),
                    "process_age_millis" to processCpuTime.ageMillis.toDouble(),
                )
            }
        }
        emitMeasured(
            "[bench-measure] crossAppLaunch-summary package=$packageName count=${totalTimes.size} " +
                "p50=${percentile(totalTimes, 0.5)}ms p95=${percentile(totalTimes, 0.95)}ms " +
                "cpuP50=${percentile(processCpuTimes, 0.5)}ms",
        )
    }

    /** 拉起被测应用并校验拉起的是它自己：被别的应用（权限弹窗等）接管即失败。 */
    private fun launchChecked(device: UiDevice, packageName: String, launcherActivity: String): AppLaunchResult {
        val result = startAndWait(device, launcherActivity)
        check(result.activity.startsWith("$packageName/")) {
            "启动被别的应用接管，测到的不是 $packageName 的启动耗时：${result.activity}"
        }
        return result
    }
}
