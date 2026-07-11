package io.torvox.installer

import android.system.Os
import android.util.Log
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import java.io.File

class SecondStageRunner(
    private val prefixDir: File,
    private val homeDir: File,
) {
    companion object {
        private const val THREAD_JOIN_TIMEOUT_MS = 5_000L
    }

    data class Result(
        val success: Boolean,
        val errors: List<String> = emptyList(),
    )

    suspend fun run(): Result = withContext(Dispatchers.IO) {
        val lockFile = File(prefixDir, "bin/termux-bootstrap-second-stage.sh.lock")
        if (lockFile.exists()) {
            return@withContext Result(true)
        }
        try {
            lockFile.parentFile?.mkdirs()
            Os.symlink(lockFile.absolutePath, lockFile.absolutePath)
        } catch (exception: android.system.ErrnoException) {
            if (exception.errno == android.system.OsConstants.EEXIST) {
                return@withContext Result(true)
            }
            return@withContext Result(false, listOf("Lock file error: ${exception.message}"))
        }
        val dpkgVersion = detectDpkgVersion() ?: "unknown"
        val arch = detectAbi()
        val postinstDir = File(prefixDir, "var/lib/dpkg/info")
        val errors = mutableListOf<String>()
        if (postinstDir.isDirectory) {
            postinstDir
                .listFiles()
                ?.filter { it.name.endsWith(".postinst") }
                ?.forEach { script ->
                    val packageName = script.name.removeSuffix(".postinst")
                    try {
                        Os.chmod(script.absolutePath, BootstrapInstaller.EXECUTABLE_FILE_MODE)
                        val environment =
                            mapOf(
                                "DPKG_MAINTSCRIPT_PACKAGE" to packageName,
                                "DPKG_MAINTSCRIPT_PACKAGE_REFCOUNT" to "1",
                                "DPKG_MAINTSCRIPT_ARCH" to arch,
                                "DPKG_MAINTSCRIPT_NAME" to "postinst",
                                "DPKG_MAINTSCRIPT_DEBUG" to "0",
                                "DPKG_RUNNING_VERSION" to dpkgVersion,
                                "DPKG_FORCE" to "security-mac,downgrade",
                                "DPKG_ADMINDIR" to File(prefixDir, "var/lib/dpkg").absolutePath,
                                "DPKG_ROOT" to "",
                                "HOME" to homeDir.absolutePath,
                                "PATH" to "${File(prefixDir, "bin").absolutePath}:/system/bin:/system/xbin",
                                "PREFIX" to prefixDir.absolutePath,
                            )
                        val proc =
                            Runtime.getRuntime().exec(
                                arrayOf(script.absolutePath, "configure"),
                                environment.map { "${it.key}=${it.value}" }.toTypedArray(),
                                File("/"),
                            )
                        val stdoutThread = Thread { proc.inputStream.bufferedReader().readText() }
                        val stderrThread = Thread { proc.errorStream.bufferedReader().readText() }
                        stdoutThread.start()
                        stderrThread.start()
                        val exitCode = proc.waitFor()
                        stdoutThread.join(THREAD_JOIN_TIMEOUT_MS)
                        stderrThread.join(THREAD_JOIN_TIMEOUT_MS)
                        if (exitCode != 0) {
                            errors.add("$packageName postinst exited with code $exitCode")
                        }
                    } catch (exception: Exception) {
                        errors.add("$packageName postinst error [${exception.javaClass.simpleName}]: ${exception.message}")
                    }
                }
        }
        writeTermuxEnv()
        Result(true, errors)
    }

    private fun detectDpkgVersion(): String? = try {
        val proc = Runtime.getRuntime().exec(arrayOf(File(prefixDir, "bin/dpkg").absolutePath, "--version"))
        val text = proc.inputStream.bufferedReader().readText()
        val match = Regex("""(\d+\.\d+\.\d+)""").find(text)
        match?.value
    } catch (e: Exception) {
        Log.w("SecondStageRunner", "detectDpkgVersion failed", e)
        null
    }

    private fun detectAbi(): String = io.torvox.detectArchFromAbi()

    private fun writeTermuxEnv() {
        val envFile = File(prefixDir, "etc/termux/termux.env")
        envFile.parentFile?.mkdirs()
        envFile.writeText(
            """
HOME=${homeDir.absolutePath}
PREFIX=${prefixDir.absolutePath}
PATH=${File(prefixDir, "bin").absolutePath}:/system/bin:/system/xbin
TMPDIR=${File(prefixDir, "tmp").absolutePath}
SHELL=${File(prefixDir, "bin/bash").absolutePath}
LANG=en_US.UTF-8
TERM=xterm-256color
COLORTERM=truecolor
            """.trimIndent(),
        )
    }
}
