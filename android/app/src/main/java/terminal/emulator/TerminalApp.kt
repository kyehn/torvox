package terminal.emulator

import android.app.Application
import android.content.Context
import android.os.PowerManager
import android.os.StrictMode
import dagger.hilt.android.HiltAndroidApp
import terminal.emulator.bridge.NativeBridge
import terminal.emulator.bridge.nativeClassInitStarted
import terminal.emulator.runtime.LogUtil
import java.util.concurrent.TimeUnit
import kotlin.concurrent.thread

@HiltAndroidApp
open class TerminalApp : Application() {
    override fun onCreate() {
        super.onCreate()
        if (isInstallProcess(Application.getProcessName())) return
        // 功耗偏好必须在 GPU 初始化之前告知：适配器只在初始化时选一次，
        // 之后再改偏好不会换适配器（见 NativeBridge.setPowerPreference）。
        reportPowerPreference()
        // 原生库装载要先读整个 .so：debug 构建实测 471ms 主线程 I/O（StrictMode
        // DiskReadViolation）。先起守护线程触碰 NativeBridge 触发类初始化，与
        // Hilt/Compose 的启动并行；主线程之后首次调用 native 时只等剩余装载时间。
        // 墙钟不会因此变短（首帧仍需等装载完成），去掉的是主线程上的磁盘读。
        thread(isDaemon = true, name = "native-preload") { NativeBridge.warmUp() }
        // 等到预装载线程真正进入类初始化再继续：否则主线程可能先触碰
        // NativeBridge，471ms 的 .so 读取就落回主线程（且装载线程名不再是
        // native-preload）。这里只等一次线程调度，不等装载完成。
        if (!nativeClassInitStarted.await(2, TimeUnit.SECONDS)) {
            LogUtil.warn(
                "TerminalApp",
                "native-preload did not start within 2s; the load may fall back to the main thread",
            )
        }
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
        installCrashHandler()
    }

    /**
     * 按系统省电模式告知适配器功耗偏好。
     *
     * 省电模式开着时偏好集成显卡（移动端 SoC 上通常是唯一选项，省电模式下的
     * 独显会显著耗电）；不在省电模式时偏好独显（桌面/高端设备上集显的呈现延迟
     * 明显更高）；读不到电源状态时不排序（`PowerPreference.None`），按驱动枚举
     * 顺序取第一个可用适配器——偏好只影响排序，取不到适配器的风险为零。
     */
    private fun reportPowerPreference() {
        val preference =
            try {
                val powerManager = getSystemService(Context.POWER_SERVICE) as PowerManager
                if (powerManager.isPowerSaveMode) POWER_PREFERENCE_LOW_POWER
                else POWER_PREFERENCE_HIGH_PERFORMANCE
            } catch (exception: RuntimeException) {
                // 电源服务不可用（如某些定制系统）：按「不排序」继续，不因此拒绝启动。
                LogUtil.warn("App", "power state unavailable", exception)
                POWER_PREFERENCE_NONE
            }
        NativeBridge.setPowerPreference(preference)
    }

    private fun installCrashHandler() {
        val defaultHandler = Thread.getDefaultUncaughtExceptionHandler()
        Thread.setDefaultUncaughtExceptionHandler { crashedThread, throwable ->
            try {
                logCrash(crashedThread, throwable)
            } catch (exception: Exception) {
                LogUtil.error("App", "Failed to log crash", exception)
            }
            // 不自行杀进程：下方平台处理器会终止进程并把 FATAL EXCEPTION
            // 堆栈写入 logcat/dropbox（远程崩溃上报），自行 kill 会让堆栈全丢。
            defaultHandler?.uncaughtException(crashedThread, throwable)
        }
    }

    /** 崩溃诊断只进 logcat：DESIGN.md 禁止把日志写入文件。 */
    private fun logCrash(thread: Thread, throwable: Throwable) {
        val causedBy = throwable.cause
        LogUtil.error(
            "App",
            "Uncaught ${throwable.javaClass.name}: ${throwable.message} on thread ${thread.name}" +
                (causedBy?.let { "\nCaused by: $it" } ?: ""),
            throwable,
        )
    }

    companion object {
        private const val INSTALL_PROCESS_SUFFIX = ":install"

        /** 适配器功耗偏好取值，与 `NativeBridge.setPowerPreference` 同义。 */
        private const val POWER_PREFERENCE_NONE = 0
        private const val POWER_PREFERENCE_LOW_POWER = 1
        private const val POWER_PREFERENCE_HIGH_PERFORMANCE = 2

        internal fun isInstallProcess(processName: String?): Boolean =
            processName != null && processName.endsWith(INSTALL_PROCESS_SUFFIX)
    }
}
