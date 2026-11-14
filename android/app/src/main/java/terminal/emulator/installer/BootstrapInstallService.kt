package terminal.emulator.installer

import android.app.Service
import android.content.Intent
import android.os.IBinder
import kotlinx.coroutines.runBlocking
import terminal.emulator.runtime.LogUtil
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
 * 由 MainActivity 触发（EXTRA_INSTALL_BOOTSTRAP intent，仅 debug 构建）。
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
        if (!isInsideAppPrivateDir(File(zipPath), listOf(filesDir, cacheDir))) {
            // 归档内容会被写进 files/usr 并以 shell 身份执行：路径放开即等于让任何应用
            // 借本应用装进自己的二进制。判据失败即拒绝，不落回宽松路径；路径本身不入日志。
            LogUtil.warn(TAG, "拒绝安装应用私有目录之外的引导包")
            stopSelf(startId)
            return START_NOT_STICKY
        }
        Thread {
            // 用 runCatchingCancellable 而非 try/catch(Exception)：detekt
            // TooGenericExceptionCaught；安装路径返回 Result。
            val result =
                runCatchingCancellable { install(zipPath) }
                    .getOrElse { "FAILED: ${it.message ?: it.javaClass.simpleName}" }
            LogUtil.info(TAG, "install result: $result")
            stopSelf(startId)
        }.apply {
            isDaemon = true
            start()
        }
        return START_NOT_STICKY
    }

    private fun install(zipPath: String): String {
        val directories = bootstrapDirectories(this)
        // 安装前把 zip 移到安全位置——它可能位于 home 目录之下。
        // 不得预删 prefix/home/staging：原子换入路径负责旧目录随机备份、失败回滚与 staging
        // 安全清理（含符号链接守卫），预删会绕过安全机制并丢失用户数据（服务侧 deleteRecursively 无符号链接守卫）。
        return withPreservedArchive(File(filesDir, "bootstrap-preserved.zip"), File(zipPath)) { preserved ->
            runBlocking {
                val installer = BootstrapInstaller(directories.prefix, directories.home, directories.staging)
                installer.install(preserved).exceptionOrNull()?.let { failure ->
                    return@runBlocking failure.message ?: "install failed"
                }
                val stage = SecondStageRunner(directories.prefix, directories.home).run()
                if (!stage.success) return@runBlocking "SECOND_STAGE_FAILED: ${stage.errors}"
                "OK prefix=${directories.prefix} shell=${installer.installedShellEntry() ?: "none"}" +
                    " installed=${installer.isInstalled()} needsInstall=${installer.needsInstall()}"
            }
        }
    }
}

/**
 * 引导归档的来源路径是否落在应用私有目录内。
 *
 * 判据取规范路径的包含性：`..` 段与指向站外的符号链接都归一到真实目标，
 * 故两者都判为站外；规范解析失败即拒绝——检查失败不得落回宽松路径。
 */
internal fun isInsideAppPrivateDir(candidate: File, roots: List<File>): Boolean = runCatchingCancellable {
    val canonical = candidate.canonicalFile
    roots.any { root ->
        val canonicalRoot = root.canonicalFile
        canonical == canonicalRoot || canonical.path.startsWith(canonicalRoot.path + File.separator)
    }
}.getOrDefault(false)
