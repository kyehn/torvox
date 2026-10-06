package terminal.emulator.installer

import android.app.Service
import android.content.Intent
import android.os.IBinder
import kotlinx.coroutines.runBlocking
import terminal.emulator.runtime.LogUtil
import terminal.emulator.runtime.isElf
import terminal.emulator.runtime.isSystemShellScript
import terminal.emulator.util.runCatchingCancellable
import java.io.File

/**
 * 在自有进程（`android:process=":install"`）中从本地路径安装引导 zip。
 *
 * 为何独立进程：当应用作为 instrumentation 目标启动时，主进程携带的是
 * 测试包的 SELinux 类目，故对应用 filesDir 的每次写入都被拒绝
 * （mkdirs 静默失败、open() EACCES——模拟器已验证）。
 * 具有独立 android:process 名的进程由应用自身 fork，保留应用自己的 SELinux 域，
 * 因此可以写 filesDir。
 *
 * 由 MainActivity 触发（EXTRA_INSTALL_BOOTSTRAP intent / INSTALL_BOOTSTRAP 广播）。
 * 进度与结果均输出到 logcat。
 */
class BootstrapInstallService : Service() {
    companion object {
        private const val TAG = "BootstrapInstallService"
        const val EXTRA_ZIP_PATH = "zipPath"
    }

    override fun onBind(intent: Intent?): IBinder? = null

    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        val zipPath = intent?.getStringExtra(EXTRA_ZIP_PATH)
        if (zipPath.isNullOrEmpty()) {
            stopSelf(startId)
            return START_NOT_STICKY
        }
        Thread {
            // 用 runCatchingCancellable 而非 try/catch(Exception)：detekt
            // TooGenericExceptionCaught；安装路径返回 Result。
            val result =
                runCatchingCancellable { install(zipPath) }
                    .getOrElse { "FAILED: ${it.message ?: it.javaClass.simpleName}" }
            LogUtil.i(TAG, "install result: $result")
            stopSelf(startId)
        }
            .apply {
                isDaemon = true
                start()
            }
        return START_NOT_STICKY
    }

    private fun install(zipPath: String): String {
        val dirs = bootstrapDirs(this)
        // 安装前把 zip 移到安全位置——它可能位于 home 目录之下。
        val preserved = File(filesDir, "bootstrap-preserved.zip")
        File(zipPath).copyTo(preserved, overwrite = true)
        // 不得预删 prefix/home/staging：原子换入路径负责旧目录随机备份、失败回滚与 staging
        // 安全清理（含符号链接守卫），预删会绕过安全机制并丢失用户数据（服务侧 deleteRecursively 无符号链接守卫）。
        return runBlocking {
            val installer = BootstrapInstaller(dirs.prefix, dirs.home, dirs.staging)
            try {
                val install = installer.install(preserved)
                if (install.isFailure) {
                    install.exceptionOrNull()?.message ?: "install failed"
                } else {
                    val stage = SecondStageRunner(dirs.prefix, dirs.home).run()
                    if (stage.success) {
                        "OK prefix=${dirs.prefix} shell=" +
                            (
                                listOf("bin/login", "bin/bash").firstOrNull {
                                    val entry = File(dirs.prefix, it)
                                    entry.isFile && (isElf(entry) || isSystemShellScript(entry))
                                } ?: "none"
                                ) +
                            " installed=${installer.isInstalled()}" +
                            " needsInstall=${installer.needsInstall()}"
                    } else {
                        "SECOND_STAGE_FAILED: ${stage.errors}"
                    }
                }
            } finally {
                // 始终删除中转包：落在用户数据树内，清除应用数据按规范不得触碰，
                // 不删即永久占用数百MB。
                preserved.delete()
            }
        }
    }
}
