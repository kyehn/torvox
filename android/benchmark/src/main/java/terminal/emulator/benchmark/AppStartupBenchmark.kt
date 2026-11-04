package terminal.emulator.benchmark

import androidx.benchmark.macro.FrameTimingMetric
import androidx.benchmark.macro.StartupMode
import androidx.benchmark.macro.StartupTimingMetric
import androidx.benchmark.macro.junit4.MacrobenchmarkRule
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.uiautomator.UiDevice
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith

private const val MEASURE_ITERATION_COUNT = 10

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
                // 冷启动前进程不在，清零帧计数是空操作：读数即进程启动以来的全部帧。
                resetFrameStats(device)
                // 启动耗时只有在界面真的出现时才有意义：首帧到了但被系统对话框
                // 挡住时，指标测的是「弹窗出现」而不是「终端可用」。
                val elapsed = elapsedMillis {
                    startActivityAndWait()
                    assertTerminalVisible(device)
                }
                val frames = readFrameStatsAfterRender(device)
                printMeasured("coldStart", elapsed, frames)
                check(frames.frames >= MIN_ANIMATION_FRAME_COUNT) {
                    "冷启动未产生任何界面帧：frames=${frames.frames}"
                }
                check(elapsed <= COLD_START_BUDGET_MILLIS) {
                    "冷启动到终端可用耗时 ${elapsed}ms 超过预算 ${COLD_START_BUDGET_MILLIS}ms"
                }
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
                resetFrameStats(device)
                val elapsed = elapsedMillis {
                    startActivityAndWait()
                    assertTerminalVisible(device)
                }
                val frames = readFrameStatsAfterRender(device)
                printMeasured("warmStart", elapsed, frames)
                check(frames.frames >= MIN_ANIMATION_FRAME_COUNT) {
                    "热启动未产生任何界面帧：frames=${frames.frames}"
                }
                check(elapsed <= WARM_START_BUDGET_MILLIS) {
                    "热启动到终端可用耗时 ${elapsed}ms 超过预算 ${WARM_START_BUDGET_MILLIS}ms"
                }
            },
        )
    }

    /** 终端界面必须真的出现；只测到「进程被拉起」而界面没到位，启动耗时没有意义。 */
    private fun assertTerminalVisible(device: UiDevice) {
        requireElement(device, TERMINAL_SCREEN_RESOURCE_ID, "终端界面")
    }
}
