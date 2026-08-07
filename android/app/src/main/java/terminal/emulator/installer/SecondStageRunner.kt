package terminal.emulator.installer

import android.system.Os
import android.util.Log
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import terminal.emulator.runtime.isElf
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
                runOnePostinst(script, dpkgVersion, arch, errors)
                scriptsCompleted++
            }
        }
        writeTermuxEnv()
        return Result(true, errors)
    }

    /**
     * Execute one dpkg postinst script with the DPKG_* environment and the
     * linker-wrapped interpreter (round-215; extracted from runPostInstalls
     * for the detekt LongMethod limit).
     */
    private suspend fun runOnePostinst(
        script: File,
        dpkgVersion: String,
        arch: String,
        errors: MutableList<String>,
    ) {
        val packageName = script.name.removeSuffix(".postinst")
        try {
            Os.chmod(script.absolutePath, BootstrapInstaller.EXECUTABLE_FILE_MODE)
            // Patch postinst: replace bare `update-alternatives` with
            // linker-wrapped invocation so SELinux allows execution from
            // untrusted_app domain. The script uses the bare name, which
            // triggers a direct execve of app_data_file (denied by SELinux).
            patchPostinstForLinker(script)
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
                    "LD_PRELOAD" to File(prefixDir, "lib/libtermux-exec.so").absolutePath,
                )
            // Round-215: postinst scripts start with
            // `#!/data/data/com.termux/files/usr/bin/sh`, and Android
            // 15+ SELinux denies execute_no_trans of app_data_file —
            // direct exec of the script fails EACCES even when the
            // shell itself works. Run the interpreter through the
            // system linker (system_linker_exec domain) exactly like
            // the PTY spawn path.
            val command = postinstCommand(script)
            val envArray = environment.map { "${it.key}=${it.value}" }.toTypedArray()
            val ldPreload = environment["LD_PRELOAD"]
            val path = environment["PATH"]
            Log.w("SecondStageRunner", "postinst exec cmd=${command.toList()}")
            Log.w("SecondStageRunner", "postinst env PATH=$path LD_PRELOAD=$ldPreload")
            val proc =
                Runtime.getRuntime().exec(
                    command,
                    envArray,
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
            val stderrBox = StringBuilder()
            val stderrThread =
                Thread {
                    stderrBox.append(proc.errorStream.bufferedReader().readText())
                }.apply { isDaemon = true }
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
                val detail = stderrBox.toString().trim().take(400)
                errors.add(
                    "$packageName postinst exited with code $exitCode" +
                        if (detail.isEmpty()) "" else " (stderr: $detail)",
                )
            }
        } catch (exception: Exception) {
            errors.add("$packageName postinst error [${exception.javaClass.simpleName}]: ${exception.message}")
        }
    }

    private fun detectDpkgVersion(): String? {
        var proc: Process? = null
        return try {
            proc =
                Runtime.getRuntime().exec(
                    prefixExecutableCommand(File(prefixDir, "bin/dpkg"), listOf("--version")),
                    prefixEnvironment().map { "${it.key}=${it.value}" }.toTypedArray(),
                    File("/"),
                )
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

    /** The system linker used to exec app-data ELFs (SELinux workaround). */
    internal fun systemLinker(): String = if (terminal.emulator.is64BitAbi()) "/system/bin/linker64" else "/system/bin/linker"

    /** Base env for prefix executables: PREFIX + termux-exec preload. */
    internal fun prefixEnvironment(): Map<String, String> = mapOf(
        "HOME" to homeDir.absolutePath,
        "PATH" to "${File(prefixDir, "bin").absolutePath}:/system/bin:/system/xbin",
        "PREFIX" to prefixDir.absolutePath,
        "TMPDIR" to File(prefixDir, "tmp").absolutePath,
        "LD_PRELOAD" to File(prefixDir, "lib/libtermux-exec.so").absolutePath,
    )

    /**
     * Build the exec argv for a prefix ELF binary. Direct execve fails with
     * EACCES on Android 15+ (SELinux execute_no_trans on app_data_file), so
     * run it through the system linker which lives in system_linker_exec.
     */
    internal fun prefixExecutableCommand(executable: File, args: List<String>): Array<String> = arrayOf(systemLinker(), executable.absolutePath) + args

    /**
     * Build the exec argv for a postinst shell script. The script's shebang
     * points at $PREFIX/bin/sh (a symlink to bash); exec the interpreter via
     * the system linker so SELinux permits it, with the script + args passed
     * through (bash <script> configure -> $0=script, $1=configure).
     */
    internal fun postinstCommand(script: File): Array<String> {
        val shebang = readShebang(script)
        val interpreter =
            if (shebang != null) {
                File(shebang)
            } else {
                File(prefixDir, "bin/sh")
            }
        val interpreterPath =
            if (interpreter.isAbsolute) {
                interpreter.path
            } else {
                File(prefixDir, interpreter.path).path
            }
        // /bin/sh (system) scripts run directly; prefix scripts need the
        // linker. Compare canonical paths: Termux packages hardcode the
        // shebang as /data/data/com.termux/files/usr/bin/sh, which is the
        // same inode as /data/user/0/com.termux/files/usr/bin/sh — a plain
        // string prefix check would send prefix scripts down the direct
        // exec path and die with SELinux EACCES.
        val canonicalInterpreter = File(interpreterPath).canonicalPath
        val canonicalPrefix = prefixDir.canonicalPath
        return if (canonicalInterpreter.startsWith(canonicalPrefix)) {
            // Scripts are patched (patchPostinstForLinker) to route prefix
            // ELF calls through /system/bin/linker64. The interpreter itself
            // is invoked via the linker so the script can load correctly.
            arrayOf(systemLinker(), canonicalInterpreter, script.absolutePath, "configure")
        } else {
            arrayOf(canonicalInterpreter, script.absolutePath, "configure")
        }
    }

    /**
     * Patch a postinst script to route `update-alternatives` through the
     * system linker. On Android 15+, SELinux denies direct exec of
     * app_data_file from the untrusted_app domain. The linker runs in
     * system_linker_exec domain which is allowed. We replace bare
     * `update-alternatives` invocations with a linker-wrapped form.
     */
    private fun patchPostinstForLinker(script: File) {
        try {
            val linker = systemLinker()
            val uaPath = File(prefixDir, "bin/update-alternatives").absolutePath
            val content = script.readText()
            // Replace bare "update-alternatives" that are NOT already
            // preceded by a path (to avoid double-patching). Match at
            // word boundary: start of line, space, tab, or semicolon.
            val patched = content.replace(
                Regex("""(?<![/\w])update-alternatives\b"""),
                "$linker $uaPath",
            )
            if (patched != content) {
                script.writeText(patched)
            }
        } catch (exception: Exception) {
            Log.w("SecondStageRunner", "patchPostinstForLinker failed for ${script.name}", exception)
        }
    }

    /** Read the `#!` interpreter from a script, or null if absent. */
    private fun readShebang(script: File): String? = try {
        script.bufferedReader().use { reader ->
            val firstLine = reader.readLine() ?: return null
            if (firstLine.startsWith("#!")) {
                firstLine.removePrefix("#!").trim().substringBefore(' ')
            } else {
                null
            }
        }
    } catch (exception: Exception) {
        Log.w("SecondStageRunner", "readShebang failed for ${script.name}", exception)
        null
    }

    private fun writeTermuxEnv() {
        val envFile = File(prefixDir, "etc/termux/termux.env")
        envFile.parentFile?.mkdirs()
        // Same login-first resolution as TerminalRuntime: nix-on-droid
        // bootstraps provide bin/login, termux bootstraps provide bin/bash.
        // Only real ELF binaries qualify (termux's bin/login is a shebang
        // script that the linker-wrapper spawn cannot load).
        val shellBinary =
            listOf("bin/login", "bin/bash", "bin/zsh", "bin/fish", "bin/sh")
                .firstOrNull { candidate ->
                    val file = File(prefixDir, candidate)
                    file.isFile && isElf(file)
                }
                ?: "bin/bash"
        envFile.writeText(
            """
HOME=${homeDir.absolutePath}
PREFIX=${prefixDir.absolutePath}
PATH=${File(prefixDir, "bin").absolutePath}:/system/bin:/system/xbin
TMPDIR=${File(prefixDir, "tmp").absolutePath}
SHELL=${File(prefixDir, shellBinary).absolutePath}
LANG=en_US.UTF-8
TERM=xterm-256color
COLORTERM=truecolor
            """.trimIndent(),
        )
    }
}
