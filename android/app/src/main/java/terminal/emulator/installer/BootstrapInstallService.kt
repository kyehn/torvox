package terminal.emulator.installer

import android.app.Service
import android.content.Intent
import android.os.IBinder
import kotlinx.coroutines.runBlocking
import terminal.emulator.runtime.LogUtil
import terminal.emulator.runtime.isElf
import terminal.emulator.runtime.isSystemShellScript
import java.io.File

/**
 * Installs a bootstrap zip from a local path inside its OWN process (`android:process=":install"`).
 *
 * Why a separate process: when the app is started as an instrumentation target, the main process
 * carries the TEST package's SELinux category, so every write to the app's filesDir is denied
 * (mkdirs silently fails, open() EACCES — emulator-verified). A process with a distinct
 * android:process name is forked by the app itself and keeps the app's own SELinux domain, so it
 * can write filesDir.
 *
 * Triggered by MainActivity (EXTRA_INSTALL_BOOTSTRAP intent / INSTALL_BOOTSTRAP broadcast).
 * Progress and outcome go to logcat;
 */
class BootstrapInstallService : Service() {
    companion object {
        private const val TAG = "BootstrapInstallService"
        const val EXTRA_ZIP_PATH = "zipPath"
        private const val PREFIX_DIR_NAME = "usr"
        private const val HOME_DIR_NAME = "home"
        private const val STAGING_DIR_NAME = "usr-staging"
    }

    override fun onBind(intent: Intent?): IBinder? = null

    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        val zipPath = intent?.getStringExtra(EXTRA_ZIP_PATH)
        if (zipPath.isNullOrEmpty()) {
            stopSelf(startId)
            return START_NOT_STICKY
        }
        Thread {
            // runCatching instead of try/catch(Exception): detekt
            // TooGenericExceptionCaught; the install path returns Results.
            val result =
                runCatching { install(zipPath) }
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
        val prefixDir = File(filesDir, PREFIX_DIR_NAME)
        val homeDir = File(filesDir, HOME_DIR_NAME)
        val stagingDir = File(filesDir, STAGING_DIR_NAME)
        // Move zip to a safe location before deleting homeDir —
        // homeDir.deleteRecursively() would destroy the zip if it lives there.
        val preserved = File(filesDir, "bootstrap-preserved.zip")
        File(zipPath).copyTo(preserved, overwrite = true)
        prefixDir.deleteRecursively()
        homeDir.deleteRecursively()
        stagingDir.deleteRecursively()
        return runBlocking {
            val installer = BootstrapInstaller(prefixDir, homeDir, stagingDir)
            val install = installer.install(preserved)
            if (install.isFailure) {
                install.exceptionOrNull()?.message ?: "install failed"
            } else {
                val stage = SecondStageRunner(prefixDir, homeDir).run()
                if (stage.success) {
                    "OK prefix=$prefixDir shell=" +
                        (
                            listOf("bin/login", "bin/bash").firstOrNull {
                                val entry = File(prefixDir, it)
                                entry.isFile && (isElf(entry) || isSystemShellScript(entry))
                            } ?: "none"
                            ) +
                        " installed=${installer.isInstalled()}" +
                        " needsInstall=${installer.needsInstall()}"
                } else {
                    "SECOND_STAGE_FAILED: ${stage.errors}"
                }
            }
        }
    }
}
