package terminal.emulator.benchmark

import androidx.benchmark.macro.FrameTimingMetric
import androidx.benchmark.macro.StartupMode
import androidx.benchmark.macro.StartupTimingMetric
import androidx.benchmark.macro.junit4.MacrobenchmarkRule
import androidx.test.ext.junit.runners.AndroidJUnit4
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith

private const val MEASURE_ITERATION_COUNT = 10

/**
 * 应用级宏基准：冷启动 / 热启动耗时。
 *
 * 命名为 AppStartupBenchmark 而非 Bridge*，因为测量对象是应用生命周期阶段，
 * 不是 JNI bridge 的单次调用开销；后者由 Rust bench 与 JNI 集成测试覆盖，
 * 详见 docs/rejected-technologies.md §7c D22。
 *
 * 默认 `CompilationMode.Partial(baselineProfileMode = Require)`，即带
 * `assets/dexopt/baseline.prof` 测量；若 profile 未安装会直接报错，
 * 因此这些数字反映的是安装 profile 后的真实表现。
 */
@RunWith(AndroidJUnit4::class)
class AppStartupBenchmark {
    @get:Rule
    val benchmarkRule = MacrobenchmarkRule()

    @Test
    fun coldStart() {
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
                startActivityAndWait()
            },
        )
    }

    @Test
    fun warmStart() {
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
                startActivityAndWait()
            },
        )
    }
}
