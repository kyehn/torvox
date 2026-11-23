package terminal.emulator.benchmark

import android.os.SystemClock
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import androidx.test.uiautomator.UiDevice
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith

/**
 * 稳态空载场景 S4：界面就绪后静置，量空载的帧、CPU 与内存。
 *
 * 空载要判的是「不画东西」：界面停在 shell 提示符上什么都不输出时，应用不该持续
 * 绘制，也不该持续吃 CPU。判据取三个互相独立的面——系统侧逐图层的终端内容帧（终端
 * 内容直绘 SurfaceView，`gfxinfo` 记不到）、应用进程的 `gfxinfo` 帧数、以及
 * `/proc/<pid>/stat` 的进程 CPU 时间。任一面出现持续绘制或持续占用即判红。
 *
 * 本用例是普通仪器测试而不是宏基准：宏基准的帧类指标在「一帧都没出」的窗口里没有
 * 样本，框架会因「读不到任何指标」直接报错；空载要的恰恰是「没有帧」这个事实，数据
 * 面在系统侧逐图层通道与进程计数上，不依赖框架指标。
 */
@RunWith(AndroidJUnit4::class)
class SteadyStateBenchmark {
    @get:Rule
    val environmentPinning = EnvironmentPinningRule(
        UiDevice.getInstance(InstrumentationRegistry.getInstrumentation()),
    )

    @Test
    fun idleLoad() {
        val device = UiDevice.getInstance(InstrumentationRegistry.getInstrumentation())
        val store = openMeasurementStore("idleLoad")
        val launcherActivity = readLauncherActivity(device, TARGET_PACKAGE)
        measureScenario(
            device = device,
            label = "idleLoad",
            store = store,
            frameworkIteration = null,
            scenario = scenarioById("S4"),
            measured = true,
            setup = {
                // 通知权限弹窗会盖住应用：被弹窗盖住时界面停画，空载窗口里零帧是弹窗
                // 造成的，不是应用的空闲表现。
                grantNotificationPermission(device)
                recoverEmulator(device)
                startAndWait(device, launcherActivity)
            },
        ) { _ ->
            // 就绪信号：终端内容层真的画出了第一帧，之后才进静置窗口。
            val (contentLayer, _) = awaitTerminalContentFrames(device)
            // 应用必须在前台：被系统弹窗盖住时界面停画，零帧是遮挡造成的，不是空闲表现。
            assertAppInForeground(device)
            SystemClock.sleep(IDLE_SETTLE_MILLIS)
            // 测量窗口开始前再核对一次：弹窗可能在静置期间抢走前台。
            assertAppInForeground(device)
            clearLayerFrameHistory(device, contentLayer)
            resetFrameStats(device)
            val cpuBefore = readProcessCpuTime(device, TARGET_PACKAGE)
            SystemClock.sleep(IDLE_WINDOW_MILLIS)
            // 窗口结束后再核对一次：弹窗在窗口中途抢走前台时界面停画，
            // 零帧会被误记成空载（本轮作废）。
            assertAppInForeground(device)
            val cpuAfter = readProcessCpuTime(device, TARGET_PACKAGE)
            val frames = readFrameStats(device)
            // 内容层没有帧记录就是「没画」：图层在窗口开始前已确认存在，窗口内进程退出
            // 由退出归因判红，故此处按零帧处理。
            val contentFrames = readLayerFrameStats(device, contentLayer)?.contentFrameCount ?: 0
            val memory = readDeviceMemory(device, TARGET_PACKAGE)
            val processCpuMillis = cpuAfter.totalMillis - cpuBefore.totalMillis
            printMeasured("idleLoad", IDLE_WINDOW_MILLIS, frames)
            check(processCpuMillis >= 0) {
                "静置窗口内进程 CPU 时间倒退：窗口前 ${cpuBefore.totalMillis}ms、" +
                    "窗口后 ${cpuAfter.totalMillis}ms（进程重启过？）"
            }
            check(processCpuMillis <= IDLE_MAX_PROCESS_CPU_MILLIS) {
                "静置窗口内进程 CPU 时间 ${processCpuMillis}ms 超过上限 " +
                    "${IDLE_MAX_PROCESS_CPU_MILLIS}ms"
            }
            check(frames.frames <= IDLE_MAX_FRAMES) {
                "静置窗口内界面出了 ${frames.frames} 帧，超过上限 $IDLE_MAX_FRAMES 帧" +
                    "（空载不该持续绘制）"
            }
            check(contentFrames <= IDLE_MAX_FRAMES) {
                "静置窗口内终端内容层出了 $contentFrames 帧，超过上限 $IDLE_MAX_FRAMES 帧"
            }
            mapOf(
                "app_frames" to frames.frames.toDouble(),
                "content_layer_frames" to contentFrames.toDouble(),
                "process_cpu_millis" to processCpuMillis.toDouble(),
                "pss_kilobytes" to memory.pssKilobytes.toDouble(),
                "rss_kilobytes" to memory.rssKilobytes.toDouble(),
            )
        }
    }
}
