package terminal.emulator.installer

import android.system.Os
import kotlinx.coroutines.withContext
import terminal.emulator.runtime.LogUtil
import terminal.emulator.runtime.isElf
import terminal.emulator.runtime.isSystemShellScript
import terminal.emulator.util.TerminalDispatchers
import java.io.File
import java.io.FileInputStream
import java.io.InputStream
import java.io.OutputStream
import java.util.zip.ZipFile
import java.util.zip.ZipInputStream

/**
 * 带硬上限的归档流复制：累计超过 [BootstrapInstaller.MAX_BOOTSTRAP_SIZE_BYTES] 即抛错。
 *
 * 在线下载与离线 SAF 选择共用它：任一路径都不得把无界流写进 cacheDir。
 * [shouldAbort] 让下载路径在拷贝途中响应取消。
 */
internal fun copyBootstrapArchive(
    input: InputStream,
    output: OutputStream,
    shouldAbort: () -> Boolean = { false },
    onCopied: (Long) -> Unit = {},
) {
    val buffer = ByteArray(BootstrapInstaller.COPY_BUFFER_SIZE)
    var total = 0L
    while (true) {
        if (shouldAbort()) throw java.io.InterruptedIOException("Bootstrap copy cancelled")
        val read = input.read(buffer)
        if (read == -1) break
        // 先判后写：上限是硬保证，绝不允许「超出一个缓冲才报错」。
        val next = total + read
        if (next > BootstrapInstaller.MAX_BOOTSTRAP_SIZE_BYTES) {
            throw java.io.IOException("Bootstrap exceeds ${BootstrapInstaller.MAX_BOOTSTRAP_SIZE_BYTES} bytes")
        }
        output.write(buffer, 0, read)
        total = next
        onCopied(total)
    }
}

class BootstrapInstaller(
    private val prefixDir: File,
    private val homeDir: File,
    private val stagingDir: File,
    private val onProgress: BootstrapProgressCallback? = null,
) {
    // 原子安装：解压到 stagingDir，完成后原子重命名切换（见 installBootstrap）。
    // 不写任何标记文件、不做校验文件，安装状态只认启动入口存在性。
    companion object {
        private const val TAG = "BootstrapInstaller"
        const val COPY_BUFFER_SIZE = 8096
        const val MAX_SYMLINKS_BYTES = 1024 * 1024
        const val EXECUTABLE_FILE_MODE = 0x1ED
        val EXEC_PREFIXES = listOf("bin/", "libexec/", "lib/apt/apt-helper", "lib/apt/methods/")
        private const val EXTRACT_PROGRESS_INTERVAL = 10

        // Zip 炸弹防护：限制解压后总负载。真实引导约 150 MB；
        // 该上限留有余量，同时阻止恶意归档填满数据分区。
        private const val MAX_EXTRACTED_BYTES = 1L * 1024 * 1024 * 1024

        /** 归档体积硬上限：在线下载与离线 SAF 两条路径共用，防止任一路径无界写入 cacheDir。 */
        const val MAX_BOOTSTRAP_SIZE_BYTES = 1_073_741_824L

        // 官方包 SYMLINKS.txt 旧式绝对路径中的分段：取其后缀拼到当前 prefix。
        // 字面量不含 /data/ 前缀，不触硬编码路径规则（见 rust-arch.yaml）。
        private const val FILES_USR_SEGMENT = "files/usr/"
    }

    /** 是否需要（重新）安装 prefix：不存在启动入口时。安装状态不做任何标记文件，只认启动入口存在性。 */
    fun needsInstall(): Boolean = !hasShellBinary()

    /** 启动入口存在性：ELF 二进制或系统解释器启动脚本均可（后者经内核 shebang 直接执行）； 私有目录 shebang 脚本不计入，其解释器本身尚不可用。 */
    private fun hasShellBinary(): Boolean = (
        File(prefixDir, "bin/login").isFile &&
            (
                isElf(File(prefixDir, "bin/login")) ||
                    isSystemShellScript(File(prefixDir, "bin/login"))
                )
        ) ||
        (
            File(prefixDir, "bin/bash").isFile &&
                (
                    isElf(File(prefixDir, "bin/bash")) ||
                        isSystemShellScript(File(prefixDir, "bin/bash"))
                    )
            )

    /** 安装状态只认启动入口存在性，不写任何标记文件。 */
    fun isInstalled(): Boolean = hasShellBinary()

    suspend fun install(zipFile: File): Result<Unit> = withContext(TerminalDispatchers.inputOutput) {
        try {
            // 只清空 staging 区。现有 prefix 必须存活到新引导完全解压并原子换入为止
            // （见 atomicRename），否则安装失败会让用户完全没有可用的引导。
            delete(stagingDir)
            createDirectories()
            onProgress?.onProgress(BootstrapProgress.Extracting(0, 0))
            val symlinks = extractZip(zipFile)
            if (symlinks.isEmpty()) {
                return@withContext Result.failure(Exception("No SYMLINKS.txt found in bootstrap ZIP"))
            }
            onProgress?.onProgress(BootstrapProgress.CreatingSymlinks)
            createSymlinks(symlinks)
            atomicRename()
            ensureHomeAndTmp()
            Result.success(Unit)
        } catch (exception: Exception) {
            // 与 BootstrapDownloader 一致只记录异常类名：异常消息可能嵌入用户提供的路径。
            // 截断到 300 字符以便诊断而不泄露完整路径。
            LogUtil.e(
                "BootstrapInstaller",
                "Install failed: ${exception.javaClass.simpleName}: ${exception.message?.take(300)}",
            )
            // 丢弃部分解压的 staging 目录：它可能有数百 MB，
            // 而系统从不清理 filesDir，故安装失败会一直泄漏磁盘直到下次重试。
            try {
                delete(stagingDir)
            } catch (cleanupException: Exception) {
                LogUtil.w("BootstrapInstaller", "Failed to clean staging dir", cleanupException)
            }
            Result.failure(exception)
        }
    }

    private fun createDirectories() {
        stagingDir.mkdirs()
    }

    private fun extractZip(zipFile: File): List<Pair<String, String>> {
        val symlinks = mutableListOf<Pair<String, String>>()
        val executables = mutableListOf<String>()
        val totalEntries = ZipFile(zipFile).use { it.size() }
        var lastReportedEntry = 0
        FileInputStream(zipFile).use { fis ->
            ZipInputStream(fis).use { zis ->
                processZipEntries(zis, symlinks, executables) { entryIndex ->
                    if (
                        entryIndex - lastReportedEntry >= EXTRACT_PROGRESS_INTERVAL ||
                        entryIndex == totalEntries
                    ) {
                        lastReportedEntry = entryIndex
                        onProgress?.onProgress(BootstrapProgress.Extracting(entryIndex, totalEntries))
                    }
                }
            }
        }
        // EXECUTABLES.txt 中的可执行文件需要 +x 权限——归档内的
        // EXECUTABLES.txt 是权威清单。路径校验与归档条目名同一规则，
        // 恶意条目不得逃出 staging 目录。
        for (executable in executables) {
            val normalizedExecutable = File(executable).path
            if (
                executable.startsWith("/") ||
                normalizedExecutable == ".." ||
                normalizedExecutable.startsWith("../") ||
                normalizedExecutable.contains("/../")
            ) {
                throw java.io.IOException("Unsafe executable path: $executable")
            }
            try {
                Os.chmod(File(stagingDir, executable).absolutePath, EXECUTABLE_FILE_MODE)
            } catch (exception: Exception) {
                LogUtil.w(TAG, "EXECUTABLES.txt chmod failed for $executable", exception)
            }
        }
        return symlinks
    }

    private fun processZipEntries(
        zis: ZipInputStream,
        symlinks: MutableList<Pair<String, String>>,
        executables: MutableList<String>,
        onEntryProcessed: (Int) -> Unit,
    ) {
        var entry = zis.nextEntry
        var entryIndex = 0
        var totalExtractedBytes = 0L
        while (entry != null) {
            val name = entry.name
            // Zip 滑移防护：拒绝绝对路径与任何 ".." 段，
            // 使恶意/被篡改的引导归档无法写入 staging 目录之外
            // （例如覆盖 prefs/logs）。
            val normalized = File(name).path
            if (
                name.startsWith("/") ||
                normalized == ".." ||
                normalized.startsWith("../") ||
                normalized.contains("/../")
            ) {
                throw java.io.IOException("Unsafe zip entry name: $name")
            }
            if (name == "SYMLINKS.txt") {
                // 有界读取：该条目是元数据，必须很小；
                // 在恶意归档上无界 readBytes() 会让进程 OOM。
                val bytes = zis.readNBytes(MAX_SYMLINKS_BYTES)
                if (bytes.size >= MAX_SYMLINKS_BYTES) {
                    throw java.io.IOException("SYMLINKS.txt exceeds $MAX_SYMLINKS_BYTES bytes")
                }
                symlinks.addAll(parseSymlinks(bytes.decodeToString()))
            } else if (name == "EXECUTABLES.txt") {
                val bytes = zis.readNBytes(MAX_SYMLINKS_BYTES)
                if (bytes.size >= MAX_SYMLINKS_BYTES) {
                    throw java.io.IOException("EXECUTABLES.txt exceeds $MAX_SYMLINKS_BYTES bytes")
                }
                executables.addAll(
                    bytes.decodeToString().lines().map { it.trim() }.filter { it.isNotEmpty() },
                )
            } else if (entry.isDirectory) {
                File(stagingDir, name).mkdirs()
            } else {
                val targetFile = File(stagingDir, name)
                targetFile.parentFile?.mkdirs()
                targetFile.outputStream().use { out ->
                    var entryBytes = 0L
                    val buffer = ByteArray(COPY_BUFFER_SIZE)
                    while (true) {
                        val read = zis.read(buffer)
                        if (read < 0) break
                        entryBytes += read
                        if (entryBytes > MAX_EXTRACTED_BYTES) {
                            throw java.io.IOException(
                                "Bootstrap entry $name exceeds $MAX_EXTRACTED_BYTES bytes uncompressed",
                            )
                        }
                        totalExtractedBytes += read
                        // 全归档的 Zip 炸弹防护：1 GiB 的下载在高压缩比下
                        // 跨多个条目可膨胀到 TB 级。此处在累计总量上施加上限，
                        // 而不只是逐条目。
                        if (totalExtractedBytes > MAX_EXTRACTED_BYTES) {
                            throw java.io.IOException(
                                "Bootstrap archive exceeds $MAX_EXTRACTED_BYTES bytes total uncompressed",
                            )
                        }
                        out.write(buffer, 0, read)
                    }
                }
                if (isExecutable(name)) {
                    Os.chmod(targetFile.absolutePath, EXECUTABLE_FILE_MODE)
                }
            }
            entryIndex++
            onEntryProcessed(entryIndex)
            entry = zis.nextEntry
        }
    }

    private fun isExecutable(name: String): Boolean = EXEC_PREFIXES.any { name.startsWith(it) }

    internal val symlinkSeparator = Regex("""\s*(?:->|←|→|↔)\s*""")

    /** Resolve `.`/`..` segments without touching the filesystem. */
    internal fun normalizePath(path: String): String {
        val absolute = path.startsWith("/")
        val stack = ArrayDeque<String>()
        for (part in path.split('/')) {
            when (part) {
                "",
                ".",
                -> {}

                ".." -> {
                    if (stack.isNotEmpty() && stack.last() != "..") {
                        stack.removeLast()
                    } else {
                        stack.addLast("..")
                    }
                }

                else -> stack.addLast(part)
            }
        }
        val joined = stack.joinToString("/")
        return if (absolute) "/$joined" else joined
    }

    internal fun parseSymlinks(content: String): List<Pair<String, String>> = content
        .lines()
        .filter { it.isNotBlank() }
        .mapNotNull { line ->
            val parts = line.split(symlinkSeparator)
            if (parts.size == 2) parts[0].trim() to parts[1].trim() else null
        }

    private fun createSymlinks(symlinks: List<Pair<String, String>>) {
        for ((target, linkPath) in symlinks) {
            // 符号链接路径逃逸防护（与 zip 条目名同一规则）：
            // 恶意 SYMLINKS.txt 绝不能创建 staging 目录之外的链接。
            val normalized = File(linkPath).path
            if (
                linkPath.startsWith("/") ||
                normalized == ".." ||
                normalized.startsWith("../") ||
                normalized.contains("/../")
            ) {
                throw java.io.IOException("Unsafe symlink path: $linkPath")
            }
            // 目标同样由攻击者控制。拒绝绝对路径与路径穿越，使链接不能指向
            // staging 树之外——否则下方的递归 delete()
            // （staging 清理/备份移除）会跟随链接并抹掉任意目录。
            // Termux 的 SYMLINKS.txt 存在两种合法形态：
            //  1. 相对目标，相对「链接的父目录」解析
            //     （`include/ncurses/` 中的 `../term_entry.h` 解析为
            //     staging 内的 `include/term_entry.h`）——天真的
            //     startsWith("../") 检查会错误地拒绝它们；
            //  2. 指向最终 prefix 的绝对目标
            //     （`<home>/usr/share/...`，即 filesDir 之下），在 staging 期间
            //     是断的，但 staging 目录被原子重命名为 `files/usr` 后即有效。
            //     只允许解析后落在规范 prefix 路径之内的绝对目标。
            if (target.startsWith("/")) {
                val canonicalPrefix = prefixDir.canonicalPath
                // 官方包 SYMLINKS.txt 用旧式绝对路径（…/files/usr/…）：取该分段
                // 之后缀拼到当前 prefix。拼后仍走下方规范校验，.. 逃逸会被拒绝；
                // 后缀字面量不含 /data/ 前缀，不触硬编码路径规则。
                val relativeSuffix = target.substringAfter(FILES_USR_SEGMENT, "")
                val canonicalTarget =
                    if (relativeSuffix.isNotEmpty()) {
                        File(canonicalPrefix, relativeSuffix).path
                    } else {
                        target
                    }
                val resolvedAbsolute =
                    try {
                        File(canonicalTarget).canonicalPath
                    } catch (exception: Exception) {
                        throw java.io.IOException(
                            "Unsafe symlink target: $target (${exception.message})",
                            exception,
                        )
                    }
                if (
                    resolvedAbsolute != canonicalPrefix && !resolvedAbsolute.startsWith("$canonicalPrefix/")
                ) {
                    throw java.io.IOException("Unsafe symlink target: $target")
                }
            } else {
                val linkParent = File(linkPath).parent
                val resolvedTarget = if (linkParent != null) File(linkParent, target).path else target
                // Java 的 File.path 不会规范化 ".." 段
                // （File("a/../b").path == "a/../b"），故在逃逸检查前手动解析。
                val normalizedResolved = normalizePath(resolvedTarget)
                if (normalizedResolved.startsWith("../") || normalizedResolved == "..") {
                    throw java.io.IOException("Unsafe symlink target: $target")
                }
            }
            val linkFile = File(stagingDir, linkPath)
            linkFile.parentFile?.mkdirs()
            Os.symlink(target, linkFile.absolutePath)
        }
    }

    /** 上一次安装的旧目录：随机后缀备份，安装成功后保留，由用户手动删除，从不自动删除。 */
    private fun atomicRename() {
        val staging = stagingDir
        val prefix = prefixDir
        if (prefix.exists()) {
            // 旧目录先整体移入随机后缀备份（同文件系统 rename 为原子操作），再换入新目录；
            // 失败则恢复备份，旧环境保持可用；成功后备份保留，由用户手动删除。
            val backup = File(prefix.parentFile, "${prefix.name}.${java.util.UUID.randomUUID().toString().take(8)}")
            if (!prefix.renameTo(backup)) {
                throw Exception("Failed to move old prefix aside: ${prefix.path}")
            }
            val renamed = staging.renameTo(prefix)
            if (!renamed) {
                // 恢复旧 prefix，使先前的引导仍可用。
                if (!backup.renameTo(prefix)) {
                    throw Exception(
                        "Atomic rename failed and rollback failed: staging=${staging.path} prefix=${prefix.path} backup=${backup.path}",
                    )
                }
                throw Exception("Atomic rename failed: ${staging.path} -> ${prefix.path}")
            }
        } else if (!staging.renameTo(prefix)) {
            throw Exception("Atomic rename failed: ${staging.path} -> ${prefix.path}")
        }
    }

    private fun ensureHomeAndTmp() {
        homeDir.mkdirs()
        File(prefixDir, "tmp").mkdirs()
    }

    private fun delete(file: File) {
        // 删除时绝不跟随符号链接：指向目录的符号链接会解析为 isDirectory=true，
        // 于是列举并递归会删除*目标*的内容（数据丢失），
        // 而自指链接会无限递归（StackOverflowError）。
        // 符号链接只是一个 inode——删它本身，而非其指向。
        if (!java.nio.file.Files.isSymbolicLink(file.toPath()) && file.isDirectory) {
            file.listFiles()?.forEach { delete(it) }
        }
        file.delete()
    }
}
