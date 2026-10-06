package terminal.emulator

import android.app.Application
import android.os.StrictMode
import dagger.hilt.android.HiltAndroidApp
import terminal.emulator.runtime.LogUtil

@HiltAndroidApp
open class TerminalApp : Application() {
    override fun onCreate() {
        super.onCreate()
        if (isInstallProcess(Application.getProcessName())) return
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

    private fun installCrashHandler() {
        val defaultHandler = Thread.getDefaultUncaughtExceptionHandler()
        Thread.setDefaultUncaughtExceptionHandler { thread, throwable ->
            try {
                logCrash(thread, throwable)
            } catch (exception: Exception) {
                LogUtil.e("App", "Failed to log crash", exception)
            }
            // 不自行杀进程：下方平台处理器会终止进程并把 FATAL EXCEPTION
            // 堆栈写入 logcat/dropbox（远程崩溃上报），自行 kill 会让堆栈全丢。
            defaultHandler?.uncaughtException(thread, throwable)
        }
    }

    /** 崩溃诊断只进 logcat：DESIGN.md 禁止把日志写入文件。 */
    private fun logCrash(thread: Thread, throwable: Throwable) {
        val causedBy = throwable.cause
        LogUtil.e(
            "App",
            "Uncaught ${throwable.javaClass.name}: ${throwable.message} on thread ${thread.name}" +
                (causedBy?.let { "\nCaused by: $it" } ?: ""),
            throwable,
        )
    }

    companion object {
        private const val INSTALL_PROCESS_SUFFIX = ":install"

        internal fun isInstallProcess(processName: String?): Boolean =
            processName != null && processName.endsWith(INSTALL_PROCESS_SUFFIX)
    }
}
