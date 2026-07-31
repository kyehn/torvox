package terminal.emulator.installer

import android.system.Os
import android.util.Log
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import java.io.File
import java.util.concurrent.TimeUnit

class SecondStageRunner(
    private val prefixDir: File,
    private val homeDir: File,
    private val onProgress: BootstrapProgressCallback? = null,
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
        if (lockFile.exists() || java.nio.file.Files.isSymbolicLink(lockFile.toPath())) {
            // The lock is a SELF-REFERENTIAL symlink (created below).
            // File.exists() follows the link → ELOOP → false, so stale
            // locks from a killed process (SIGKILL skips finally) must be
            // detected via isSymbolicLink. Short-circuiting to "success"
            // would permanently skip postinst while dpkg stays half-
            // configured. The postinst scripts are idempotent (dpkg
            // "configure" reruns), so delete the stale lock and retry.
            // NOTE (round-103): this stale-detection cannot distinguish a
            // live concurrent runner from a stale lock — a second process
            // would delete the active lock and run postinst concurrently.
            // This is best-effort by design: process-local concurrency is
            // serialized by BootstrapOrchestrator.processInstalling, and
            // cross-process overlap is tolerated because postinst scripts
            // are idempotent (dpkg "configure" semantics).
            Log.w("SecondStageRunner", "Stale lock file found, deleting and retrying postinst")
            lockFile.delete()
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
        try {
            return@withContext runPostInstalls()
        } finally {
            // Always release the lock: if the process is killed mid-postinst
            // the stale lock would otherwise make every retry short-circuit
            // to "success" while dpkg stays half-configured. Deleting the
            // lock (not the symlink target) allows a genuine retry.
            lockFile.delete()
        }
    }

    private suspend fun runPostInstalls(): Result {
        val dpkgVersion = detectDpkgVersion() ?: "unknown"
        val arch = detectAbi()
        val postinstDir = File(prefixDir, "var/lib/dpkg/info")
        val errors = mutableListOf<String>()
        if (postinstDir.isDirectory) {
            val scripts =
                postinstDir
                    .listFiles()
                    ?.filter { it.name.endsWith(".postinst") }
                    ?.toList()
                    ?: emptyList()
            val totalScripts = scripts.size
            var scriptsCompleted = 0
            scripts.forEach { script ->
                onProgress?.onProgress(
                    BootstrapProgress.RunningPostInstall(scriptsCompleted, totalScripts),
                )
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
                    proc.outputStream.close()
                    // Daemon consumers: if the postinst's grandchildren keep
                    // the pipes open after destroyForcibly(), the blocked
                    // readText threads must not outlive the process (a plain
                    // thread would leak and pin the JVM's lifetime).
                    val stdoutThread =
                        Thread { proc.inputStream.bufferedReader().readText() }.apply {
                            isDaemon = true
                        }
                    val stderrThread =
                        Thread { proc.errorStream.bufferedReader().readText() }.apply {
                            isDaemon = true
                        }
                    stdoutThread.start()
                    stderrThread.start()
                    val exited = proc.waitFor(30, TimeUnit.SECONDS)
                    if (!exited) {
                        proc.destroyForcibly()
                        // Android's Process has no ProcessHandle API, so
                        // grandchildren cannot be killed directly. SIGKILL on
                        // the direct child plus the daemon pipe consumers
                        // below is the best available cleanup; a surviving
                        // grandchild is orphaned and reaped by the system
                        // when the app process dies.
                        proc.waitFor(5, TimeUnit.SECONDS)
                        stdoutThread.join(THREAD_JOIN_TIMEOUT_MS)
                        stderrThread.join(THREAD_JOIN_TIMEOUT_MS)
                        throw RuntimeException("$packageName postinst timed out after 30s")
                    }
                    stdoutThread.join(THREAD_JOIN_TIMEOUT_MS)
                    stderrThread.join(THREAD_JOIN_TIMEOUT_MS)
                    val exitCode = proc.exitValue()
                    if (exitCode != 0) {
                        errors.add("$packageName postinst exited with code $exitCode")
                    }
                } catch (exception: Exception) {
                    errors.add("$packageName postinst error [${exception.javaClass.simpleName}]: ${exception.message}")
                }
                scriptsCompleted++
            }
        }
        writeTermuxEnv()
        return Result(true, errors)
    }

    private fun detectDpkgVersion(): String? {
        var proc: Process? = null
        return try {
            proc = Runtime.getRuntime().exec(arrayOf(File(prefixDir, "bin/dpkg").absolutePath, "--version"))
            proc.outputStream.close()
            // Consume stderr on a daemon thread: a corrupt dpkg binary that
            // floods stderr past the 64KB pipe buffer would otherwise block
            // the readText() below forever (the main postinst path has a 30s
            // timeout; this helper had none).
            val stderrThread =
                Thread { proc.errorStream.bufferedReader().readText() }.apply {
                    isDaemon = true
                }
            stderrThread.start()
            if (!proc.waitFor(10, TimeUnit.SECONDS)) {
                proc.destroyForcibly()
                Log.w("SecondStageRunner", "detectDpkgVersion timed out")
                return null
            }
            val text = proc.inputStream.bufferedReader().readText()
            val match = Regex("""(\d+\.\d+\.\d+)""").find(text)
            match?.value
        } catch (e: Exception) {
            Log.w("SecondStageRunner", "detectDpkgVersion failed", e)
            null
        } finally {
            proc?.destroy()
        }
    }

    private fun detectAbi(): String = terminal.emulator.detectArchFromAbi()

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
