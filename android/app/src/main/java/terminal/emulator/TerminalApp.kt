package terminal.emulator

import android.app.Application
import android.os.StrictMode
import android.util.Log
import dagger.hilt.android.HiltAndroidApp
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch
import terminal.emulator.bridge.NativeBridge
import terminal.emulator.monitor.AnrWatchDog
import terminal.emulator.monitor.BootGuard
import terminal.emulator.monitor.MemoryMonitor
import terminal.emulator.monitor.ThermalMonitor

@HiltAndroidApp
open class TerminalApp : Application() {
    private var anrWatchDog: AnrWatchDog? = null
    private var memoryMonitor: MemoryMonitor? = null
    private var thermalMonitor: ThermalMonitor? = null
    private val monitorScope = CoroutineScope(SupervisorJob() + Dispatchers.Default)

    override fun onCreate() {
        super.onCreate()
        val stateDir = getDir("boot_state", MODE_PRIVATE)
        BootGuard(stateDir).check()
        // StrictMode 仅 debug：release 下每次 I/O 的 penaltyLog 拖慢冷启动。
        if (BuildConfig.DEBUG) {
            StrictMode.setThreadPolicy(
                StrictMode.ThreadPolicy
                    .Builder()
                    .detectDiskReads()
                    .detectDiskWrites()
                    .detectNetwork()
                    .detectUnbufferedIo()
                    .penaltyLog()
                    .build(),
            )
            StrictMode.setVmPolicy(
                StrictMode.VmPolicy
                    .Builder()
                    .detectActivityLeaks()
                    .detectLeakedClosableObjects()
                    .detectLeakedRegistrationObjects()
                    .detectFileUriExposure()
                    .detectCleartextNetwork()
                    .penaltyLog()
                    .build(),
            )
        }
        installAnrWatchDog()
        installMemoryMonitor()
        installThermalMonitor()
        installCrashHandler()
        // 原生库缺失/损坏时终端根本无法工作：按 DESIGN 错误策略让异常抛出，
        // 由已安装的崩溃处理器记录 logcat 后终止进程，不得在此层捕获后吞掉。
        // 放在 installCrashHandler 之后，避免与处理器安装产生竞态。
        Thread({ NativeBridge.initLogger() }, "NativeInit").start()
        monitorScope.launch {
            delay(HEALTHY_UPTIME_MS)
            BootGuard(stateDir).markHealthy()
        }
    }

    override fun onTrimMemory(level: Int) {
        super.onTrimMemory(level)
        memoryMonitor?.onTrimMemory(level)
    }

    private fun installAnrWatchDog() {
        // 刻意仅用于 release：在慢速软件渲染模拟器上，
        // 冷启动 Activity 或首个 Compose 帧经常超出 5s 的看门狗窗口；
        // 且在插桩测试中看门狗会杀掉被测进程
        // （表现为无任何原生栈的「Process crashed」）。
        // debug 构建服务于开发/CI，那里不需要这个面向用户的自退保护。
        if (BuildConfig.DEBUG) return
        val stateDir = getDir("boot_state", MODE_PRIVATE)
        anrWatchDog = AnrWatchDog(stateDir, ANR_TIMEOUT_MILLIS).also { it.start() }
    }

    private fun installMemoryMonitor() {
        memoryMonitor =
            MemoryMonitor(this, monitorScope).also {
                it.startPolling()
            }
    }

    private fun installThermalMonitor() {
        val stateDir = getDir("boot_state", MODE_PRIVATE)
        thermalMonitor =
            ThermalMonitor(this) { BootGuard.exit(stateDir, "Thermal CRITICAL+") }
                .also { it.register() }
    }

    private fun installCrashHandler() {
        val defaultHandler = Thread.getDefaultUncaughtExceptionHandler()
        Thread.setDefaultUncaughtExceptionHandler { thread, throwable ->
            try {
                logCrash(thread, throwable)
            } catch (exception: Exception) {
                Log.e("App", "Failed to log crash", exception)
            }
            // 记录退出以供启动循环检测，而不由我们自己杀进程：
            // 下方的平台处理器会终止进程并把 FATAL EXCEPTION 堆栈
            // 写入 logcat/dropbox（远程崩溃上报）。
            // BootGuard.exit() 会先行杀进程，平台处理器因而永不运行，堆栈完全丢失。
            try {
                BootGuard(getDir("boot_state", MODE_PRIVATE)).recordExit()
            } catch (exception: Exception) {
                Log.e("App", "Failed to record boot exit", exception)
            }
            defaultHandler?.uncaughtException(thread, throwable)
        }
    }

    /** 崩溃诊断只进 logcat：DESIGN.md 禁止把日志写入文件。 */
    private fun logCrash(thread: Thread, throwable: Throwable) {
        val causedBy = throwable.cause
        Log.e(
            "App",
            "Uncaught ${throwable.javaClass.name}: ${throwable.message} on thread ${thread.name}" +
                (causedBy?.let { "\nCaused by: $it" } ?: ""),
            throwable,
        )
    }

    companion object {
        private const val ANR_TIMEOUT_MILLIS = 5_000L
        private const val MINUTES_TO_HEALTHY = 10L
        private const val MILLIS_PER_MINUTE = 60_000L
        private const val HEALTHY_UPTIME_MS = MINUTES_TO_HEALTHY * MILLIS_PER_MINUTE
    }
}
