package terminal.emulator.installer

import android.system.Os
import kotlinx.coroutines.withContext
import terminal.emulator.runtime.LogUtil
import terminal.emulator.util.TerminalDispatchers
import terminal.emulator.util.runCatchingCancellable
import java.io.File
import java.util.concurrent.TimeUnit

/**
 * 把子进程管道读到 EOF，绝不抛异常。destroy() 关闭管道时守护排空可能仍阻塞在 read；
 * 裸 Thread 上未捕获的抛出会导致进程 FATAL（设备上已证：
 * 无 dpkg 的 prefix 上子进程先退出时 detectDpkgVersion 复现）。
 * 读取与关闭竞争时返回 null。无人消费的排空结果直接丢弃。
 */
internal fun drainQuietly(stream: java.io.InputStream): String? = runCatchingCancellable {
    stream.bufferedReader().readText()
}.getOrNull()

class SecondStageRunner(
    private val prefixDir: File,
    private val homeDir: File,
    private val onProgress: BootstrapProgressCallback? = null,
) {
    companion object {
        /** 进程已退出后等待 stdout 排空线程收尾的上限；超时即放弃剩余输出（daemon 线程不会泄漏）。 */
        private const val STDIO_DRAIN_JOIN_TIMEOUT_MS = 2_000L
        private const val THREAD_JOIN_TIMEOUT_MS = 5_000L

        // postinst 单脚本最大执行轮次：首轮失败（首运崩溃/偶发超时）重试一次，
        // 连挂两次即确定性失败，不再重试。
        private const val POSTINST_MAX_ATTEMPTS = 2

        /** 执行 app-data ELF 的系统链接器（SELinux 绕过）。出货的两个 ABI 都是 64 位。 */
        internal const val SYSTEM_LINKER = "/system/bin/linker64"
    }

    data class Result(val success: Boolean, val errors: List<String> = emptyList())

    suspend fun run(): Result = withContext(TerminalDispatchers.inputOutput) {
        val lockFile = File(prefixDir, "bin/termux-bootstrap-second-stage.sh.lock")
        if (lockFile.exists() || java.nio.file.Files.isSymbolicLink(lockFile.toPath())) {
            // 该锁是自指符号链接（如下方所创建）。
            // File.exists() 会跟随链接 → ELOOP → 返回 false，
            // 故被杀进程遗留的陈旧锁（SIGKILL 跳过 finally）
            // 必须经 isSymbolicLink 侦测。若短路返回「成功」，
            // 会在 dpkg 保持半配置状态时永久跳过 postinst。
            // postinst 脚本是幂等的（dpkg "configure" 会重跑），
            // 故删除陈旧锁后重试。
            //
            // 注意：此陈旧侦测无法区分存活的并发运行者与陈旧锁
            // ——第二个进程会删掉活跃的锁并并发运行 postinst。
            // 这是有意为之的尽力而为：进程内并发由
            // BootstrapOrchestrator.processInstalling 串行化，
            // 而跨进程重叠可容忍，因为 postinst 脚本是幂等的
            // （dpkg "configure" 语义）。
            LogUtil.w("SecondStageRunner", "Stale lock file found, deleting and retrying postinst")
            lockFile.delete()
        }
        try {
            lockFile.parentFile?.mkdirs()
            Os.symlink(lockFile.absolutePath, lockFile.absolutePath)
        } catch (exception: android.system.ErrnoException) {
            // EEXIST 表示另一个安装进程正持有锁：postinst 根本没跑，不能报成功。
            return@withContext Result(false, listOf("Lock file error: ${exception.message}"))
        }
        try {
            return@withContext runPostInstalls()
        } finally {
            // 总是释放锁：若进程在 postinst 中途被杀，残留锁会让每次重试都短路，
            // 而 dpkg 仍处于半配置状态。删除锁（而非链接目标）以允许真正重试。
            lockFile.delete()
        }
    }

    private suspend fun runPostInstalls(): Result {
        val postinstDir = File(prefixDir, "var/lib/dpkg/info")
        if (!postinstDir.isDirectory) return Result(true)
        // dpkg 版本只用于 DPKG_RUNNING_VERSION 环境变量；探测失败不阻断 postinst
        // （postinst 是普通 shell 脚本，DESIGN「不做无意义检查」）。探测失败本身
        // 已在 detectDpkgVersion 内记警告，不是静默。
        val dpkgVersion = detectDpkgVersion().orEmpty()
        val arch = detectAbi()
        // 按文件名排序：`File.listFiles()` 的返回顺序未定义（实测为目录项顺序），
        // 于是 postinst 的执行次序随文件系统而变。两个后果：进度条的脚本名顺序不可复现，
        // 且依赖彼此的包（dpkg 自身即依赖某些包先就位）可能随机失败。
        // dpkg 记录的文件名本身就是版本化的字典序（如 `libfoo_1.2.postinst`），
        // 故字典序即 dpkg 的惯例序。
        val scripts =
            postinstDir.listFiles()?.filter { it.name.endsWith(".postinst") }?.sortedBy { it.name } ?: emptyList()
        val totalScripts = scripts.size
        val errors = mutableListOf<String>()
        var scriptsCompleted = 0
        scripts.forEach { script ->
            onProgress?.onProgress(
                BootstrapProgress.RunningPostInstall(scriptsCompleted, totalScripts),
            )
            runOnePostinst(script, dpkgVersion, arch, errors)
            scriptsCompleted++
        }
        return Result(errors.isEmpty(), errors)
    }

    /** 在 DPKG_* 环境与经链接器包装的解释器下执行一个 dpkg postinst 脚本（从 runPostInstalls 抽出以满足 detekt LongMethod 限制）。 */
    private suspend fun runOnePostinst(script: File, dpkgVersion: String, arch: String, errors: MutableList<String>) {
        val packageName = script.name.removeSuffix(".postinst")
        // termux dpkg 的 update-alternatives 在状态文件缺失的首次运行时，
        // 正确写完链接与状态文件后 segfault（139，tombstone 落盘，本地 10/10 复现；
        // 有状态后重跑退出 0 且幂等，3/3）。故失败重试一次：确定性失败连挂两次照常上报，
        // 首运崩溃则自愈。首次失败记警告，不隐藏。
        repeat(POSTINST_MAX_ATTEMPTS) { attempt ->
            if (runOnePostinstAttempt(script, packageName, dpkgVersion, arch, errors, attempt)) return
        }
    }

    private suspend fun runOnePostinstAttempt(
        script: File,
        packageName: String,
        dpkgVersion: String,
        arch: String,
        errors: MutableList<String>,
        attempt: Int,
    ): Boolean {
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
                    "PREFIX" to prefixDir.absolutePath,
                )
            // postinst 脚本以 `#!<home>/usr/bin/sh` 开头（filesDir 之下的 prefix sh），
            // 而 Android 15+ 的 SELinux 拒绝 app_data_file 的 execute_no_trans
            // ——即便 shell 本身可用，直接 exec 脚本也会 EACCES。
            // 故与 PTY spawn 路径完全相同地，经系统链接器
            // （system_linker_exec 域）运行解释器。
            val command = postinstCommand(script)
            val envArray = environment.map { "${it.key}=${it.value}" }.toTypedArray()
            LogUtil.w("SecondStageRunner", "postinst exec cmd=${command.toList()}")
            val proc =
                Runtime.getRuntime()
                    .exec(
                        command,
                        envArray,
                        File("/"),
                    )
            proc.outputStream.close()
            // 守护消费者：若 destroyForcibly() 之后 postinst 的孙进程仍持有管道，
            // 阻塞的 readText 线程绝不能比进程活得更久
            // （普通线程会泄漏并绑定 JVM 生命周期）。
            // 排空 lambda 绝不能抛异常：下方的 proc.destroy() 关闭管道时
            // 守护读取线程可能仍阻塞在 readText 中
            // （设备上已证：子进程先退出时 detectDpkgVersion 出现 FATAL）。
            // 裸 Thread 上未捕获的抛出会杀掉应用进程。
            val stdoutThread =
                Thread { drainQuietly(proc.inputStream) }
                    .apply {
                        isDaemon = true
                    }
            val stderrBox = StringBuilder()
            val stderrThread =
                Thread { drainQuietly(proc.errorStream)?.let { stderrBox.append(it) } }
                    .apply { isDaemon = true }
            stdoutThread.start()
            stderrThread.start()
            val exited = proc.waitFor(30, TimeUnit.SECONDS)
            if (!exited) {
                proc.destroyForcibly()
                // Android 的 Process 没有 ProcessHandle API，
                // 故无法直接杀掉孙进程。向下述守护管道消费者
                // 对直接子进程发 SIGKILL 是可得的最佳清理手段；
                // 幸存的孙进程会成孤儿，并在应用进程死亡时被系统回收。
                proc.waitFor(5, TimeUnit.SECONDS)
                stdoutThread.join(THREAD_JOIN_TIMEOUT_MS)
                stderrThread.join(THREAD_JOIN_TIMEOUT_MS)
                val timeoutReport = "$packageName postinst timed out after 30s"
                return recordPostinstFailure(errors, timeoutReport, attempt, POSTINST_MAX_ATTEMPTS)
            }
            stdoutThread.join(THREAD_JOIN_TIMEOUT_MS)
            stderrThread.join(THREAD_JOIN_TIMEOUT_MS)
            val exitCode = proc.exitValue()
            if (exitCode == 0) return true
            val detail = stderrBox.toString().trim().take(400)
            val report =
                "$packageName postinst exited with code $exitCode" +
                    if (detail.isEmpty()) "" else " (stderr: $detail)"
            return recordPostinstFailure(errors, report, attempt, POSTINST_MAX_ATTEMPTS)
        } catch (exception: Exception) {
            val report =
                "$packageName postinst error [${exception.javaClass.simpleName}]: ${exception.message}"
            return recordPostinstFailure(errors, report, attempt, POSTINST_MAX_ATTEMPTS)
        }
    }

    private fun detectDpkgVersion(): String? {
        var proc: Process? = null
        return try {
            proc =
                Runtime.getRuntime()
                    .exec(
                        prefixExecutableCommand(File(prefixDir, "bin/dpkg"), listOf("--version")),
                        prefixEnvironment().map { "${it.key}=${it.value}" }.toTypedArray(),
                        File("/"),
                    )
            proc.outputStream.close()
            // 在守护线程上消费 stderr：损坏的 dpkg 二进制若把 stderr 灌满 64KB 管道缓冲，
            // 会使下方 readText() 永久阻塞（主 postinst 路径有 30s 超时；此辅助函数原本没有）。
            val stderrThread =
                Thread { drainQuietly(proc.errorStream) }
                    .apply {
                        isDaemon = true
                    }
            stderrThread.start()
            // stdout 同样在守护线程上排空，且与 waitFor 并行：dpkg 的任何后代进程若
            // 继承了 stdout 并长期持有管道，waitFor 返回后再读会永远阻塞，
            // finally 的 destroy() 与其后的锁文件清理都执行不到。
            val stdoutText = StringBuilder()
            val stdoutThread =
                Thread {
                    try {
                        val drained = proc.inputStream.bufferedReader().use { it.readText() }
                        // 先读完再入锁：读操作本身可能被永久阻塞（后代进程持有管道），
                        // 持锁读会让主线程紧随其后的 join 超时形同虚设。
                        synchronized(stdoutText) { stdoutText.append(drained) }
                    } catch (exception: Exception) {
                        LogUtil.w("SecondStageRunner", "detectDpkgVersion stdout read failed", exception)
                    }
                }
                    .apply {
                        isDaemon = true
                    }
            stdoutThread.start()
            if (!proc.waitFor(10, TimeUnit.SECONDS)) {
                proc.destroyForcibly()
                LogUtil.w("SecondStageRunner", "detectDpkgVersion timed out")
                return null
            }
            stdoutThread.join(STDIO_DRAIN_JOIN_TIMEOUT_MS)
            if (stdoutThread.isAlive) {
                // 仍有后代进程持有 stdout 管道。`DPKG_RUNNING_VERSION` 会被导成空串，
                // 依赖它的 postinst 分支将拿到空值而非真实版本——必须出声。
                LogUtil.w("SecondStageRunner", "detectDpkgVersion stdout still open after exit, version unavailable")
            }
            val text = synchronized(stdoutText) { stdoutText.toString() }
            val match = Regex("""(\d+\.\d+\.\d+)""").find(text)
            match?.value
        } catch (exception: Exception) {
            LogUtil.w("SecondStageRunner", "detectDpkgVersion failed", exception)
            null
        } finally {
            proc?.destroy()
        }
    }

    private fun detectAbi(): String = terminal.emulator.detectArchFromAbi()

    /** prefix 下可执行文件的基础环境变量：仅含规范白名单内的变量。 */
    internal fun prefixEnvironment(): Map<String, String> = mapOf(
        "HOME" to homeDir.absolutePath,
        "TERMUX_HOME_DIR_PATH" to homeDir.absolutePath,
        "PREFIX" to prefixDir.absolutePath,
        "TERMUX_PREFIX_DIR_PATH" to prefixDir.absolutePath,
        "TMPDIR" to File(prefixDir, "tmp").absolutePath,
        "TERMUX_TMP_PREFIX_DIR_PATH" to File(prefixDir, "tmp").absolutePath,
        "LANG" to "en_US.UTF-8",
        "TERM" to "xterm-256color",
        "COLORTERM" to "truecolor",
        "TERMUX_VERSION" to "0.119.0-beta.3",
    )

    /**
     * 构造 prefix ELF 二进制的 exec argv。在 Android 15+ 上直接 execve 会因
     * SELinux 对 app_data_file 的 execute_no_trans 而 EACCES，
     * 故经位于 system_linker_exec 的系统链接器运行。
     */
    internal fun prefixExecutableCommand(executable: File, args: List<String>): Array<String> = arrayOf(
        SYSTEM_LINKER,
        executable.absolutePath,
    ) + args

    /**
     * 构造 postinst shell 脚本的 exec argv。脚本的 shebang 指向 $PREFIX/bin/sh
     * （指向 bash 的符号链接）；经系统链接器 exec 解释器以便 SELinux 允许，
     * 并透传脚本与参数（bash <script> configure → $0=script，$1=configure）。
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
        // /bin/sh（系统）脚本直接运行；prefix 脚本需要链接器。
        // 比较规范路径：Termux 包把 shebang 硬编码为 <home>/usr/bin/sh
        // （/data/data 与 /data/user/0 两种写法解析到同一 inode）
        // ——朴素的字符串前缀检查会把 prefix 脚本送进直接 exec 路径
        // 而死于 SELinux EACCES。
        val canonicalInterpreter = File(interpreterPath).canonicalPath
        val canonicalPrefix = prefixDir.canonicalPath
        return if (canonicalInterpreter.startsWith(canonicalPrefix)) {
            // 解释器落在 prefix 内（其 ELF 调用已指向 /system/bin/linker64），
            // 故解释器本身也必须经链接器调用，脚本才能正确加载。
            arrayOf(SYSTEM_LINKER, canonicalInterpreter, script.absolutePath, "configure")
        } else {
            arrayOf(canonicalInterpreter, script.absolutePath, "configure")
        }
    }

    /** 读取脚本的 `#!` 解释器；没有时返回 `null`。 */
    private fun readShebang(script: File): String? = script.bufferedReader().use { reader ->
        val firstLine = reader.readLine() ?: return null
        if (firstLine.startsWith("#!")) {
            firstLine.removePrefix("#!").trim().substringBefore(' ')
        } else {
            null
        }
    }
}

/**
 * postinst 三条失败路径（超时 / 非零退出 / 抛异常）共用的处置：只有末次尝试才记入
 * [errors]，否则「首运异常、重试自愈」会被误报为安装失败。恒返回 false，调用方直接
 * 返回即可——三条路径的差别只在 [report] 的措辞，判据必须同源。[maxAttempts] 由调用方
 * 传入：上限是类的私有 companion 常量，顶层函数取不到。
 */
private fun recordPostinstFailure(
    errors: MutableList<String>,
    report: String,
    attempt: Int,
    maxAttempts: Int,
): Boolean {
    if (attempt + 1 >= maxAttempts) {
        errors.add(report)
    } else {
        LogUtil.w("SecondStageRunner", "$report — retrying once")
    }
    return false
}
