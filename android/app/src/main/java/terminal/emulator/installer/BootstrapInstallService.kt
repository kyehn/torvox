package terminal.emulator.installer

import android.app.Service
import android.content.Intent
import android.os.IBinder
import kotlinx.coroutines.runBlocking
import terminal.emulator.runtime.LogUtil
import terminal.emulator.runtime.isElf
import java.io.File

/**
 * Installs a bootstrap zip from a local path inside its OWN process
 * (`android:process=":install"`).
 *
 * Why a separate process (round-223): when the app is started as an
 * instrumentation target, the main process carries the TEST package's
 * SELinux category, so every write to the app's filesDir is denied
 * (mkdirs silently fails, open() EACCES — emulator-verified). A process
 * with a distinct android:process name is forked by the app itself and
 * keeps the app's own SELinux domain, so it can write filesDir.
 *
 * Triggered by MainActivity (EXTRA_INSTALL_BOOTSTRAP intent /
 * INSTALL_BOOTSTRAP broadcast). Writes the outcome to
 * files/nix-install-result.txt so shell-side tests can assert on it.
 */
class BootstrapInstallService : Service() {
    companion object {
        private const val TAG = "BootstrapInstallService"
        const val EXTRA_ZIP_PATH = "zipPath"
        private const val RESULT_FILE = "nix-install-result.txt"
        private const val TEST_PREFIX = "usr-nix-test"
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
            // TooGenericExceptionCaught; the install path returns Results,
            // only the marker write can throw IOException.
            val result =
                runCatching { install(zipPath) }
                    .getOrElse { "FAILED: ${it.message ?: it.javaClass.simpleName}" }
            runCatching { File(filesDir, RESULT_FILE).writeText(result) }
                .onFailure { LogUtil.e(TAG, "failed to write result marker", it) }
            LogUtil.i(TAG, "install result: $result")
            stopSelf(startId)
        }.apply {
            isDaemon = true
            start()
        }
        return START_NOT_STICKY
    }

    private fun install(zipPath: String): String {
        val prefixDir = File(filesDir, TEST_PREFIX)
        val homeDir = File(filesDir, "home-nix-test")
        val stagingDir = File(filesDir, "usr-nix-test-staging")
        prefixDir.deleteRecursively()
        homeDir.deleteRecursively()
        stagingDir.deleteRecursively()
        return runBlocking {
            val installer = BootstrapInstaller(prefixDir, homeDir, stagingDir)
            val install = installer.install(File(zipPath))
            if (install.isFailure) {
                install.exceptionOrNull()?.message ?: "install failed"
            } else {
                val stage = SecondStageRunner(prefixDir, homeDir).run()
                if (stage.success) {
                    // needsInstall(zipSha256) verifies the marker round-trip.
                    val zipSha256 = BootstrapInstaller.sha256Of(File(zipPath))
                    "OK prefix=$prefixDir shell=" +
                        (
                            listOf("bin/login", "bin/bash", "bin/zsh", "bin/fish", "bin/sh")
                                .firstOrNull { isElf(File(prefixDir, it)) } ?: "none"
                            ) +
                        " installed=${installer.isInstalled()}" +
                        " pinned=${BootstrapInstaller.readVersionPin(prefixDir) == zipSha256}" +
                        " needsInstall=${installer.needsInstall(zipSha256)}"
                } else {
                    "SECOND_STAGE_FAILED: ${stage.errors}"
                }
            }
        }
    }
}
