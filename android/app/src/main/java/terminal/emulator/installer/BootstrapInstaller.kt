package terminal.emulator.installer

import android.content.Context
import android.system.Os
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.withContext
import terminal.emulator.runtime.LogUtil
import terminal.emulator.runtime.isElf
import terminal.emulator.runtime.isSystemShellScript
import terminal.emulator.util.TerminalDispatchers
import java.io.File
import java.io.FileInputStream
import java.io.IOException
import java.io.InputStream
import java.io.InterruptedIOException
import java.io.OutputStream
import java.nio.file.Files
import java.nio.file.LinkOption
import java.nio.file.Paths
import java.util.UUID
import java.util.zip.ZipFile
import java.util.zip.ZipInputStream

/**
 * 引导安装用到的三个目录，恒在应用数据之下：操作系统只能经应用数据管理回收它们，
 * 绝不会因缓存压力清理（故不落 cacheDir）。
 *
 * 三者必须同源——目录名只此一处声明，应用内在线安装（`TerminalRuntime`）、
 * 设置页安装（`TerminalViewModel`）与 SAF 离线安装服务三条入口都经它构造。
 */
internal data class BootstrapDirectories(val prefix: File, val home: File, val staging: File)

/** [Context.getFilesDir] 之下的引导目录；`staging` 由原子换入路径负责失败回滚与安全清理。 */
internal fun bootstrapDirectories(context: Context): BootstrapDirectories {
    val filesDir = context.filesDir
    return BootstrapDirectories(File(filesDir, "usr"), File(filesDir, "home"), File(filesDir, "usr-staging"))
}

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
        if (shouldAbort()) throw InterruptedIOException("Bootstrap copy cancelled")
        val read = input.read(buffer)
        if (read == -1) break
        // 先判后写：上限是硬保证，绝不允许「超出一个缓冲才报错」。
        val next = total + read
        if (next > BootstrapInstaller.MAX_BOOTSTRAP_SIZE_BYTES) {
            throw IOException("Bootstrap exceeds ${BootstrapInstaller.MAX_BOOTSTRAP_SIZE_BYTES} bytes")
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
    /**
     * 清空 staging 区的实现，返回是否全部删净。
     *
     * 只用于**本次安装失败**后的清理（删掉自己刚建的 staging 区），不用于安装前的
     * 预处理：staging 已存在时安装直接被拒绝。默认递归删除；测试注入「删不掉」以
     * 覆盖失败路径——靠权限位构造该前提在 root 或特殊挂载下不成立，用例只能被
     * 静默跳过，等于没测。
     */
    private val clearStaging: (File) -> Boolean = ::deleteTree,
) {
    // 原子安装：解压到 stagingDir，完成后原子重命名切换（见 installBootstrap）。
    // 不写任何标记文件、不做校验文件，安装状态只认启动入口存在性。
    companion object {
        private const val TAG = "BootstrapInstaller"
        const val COPY_BUFFER_SIZE = 8096

        /** `SYMLINKS.txt` 与 `EXECUTABLES.txt` 是元数据清单，必须很小；无界读取会 OOM。 */
        const val MAX_METADATA_BYTES = 1024 * 1024

        /** 引导启动入口候选，按优先级排列：`bin/login` 优先，其次 `bin/bash`。 */
        val SHELL_ENTRIES = listOf("bin/login", "bin/bash")

        /** `SYMLINKS.txt` 每行的分隔符（官方包与第三方重建包各用过不同箭头）。 */
        private val SYMLINK_SEPARATOR = Regex("""\s*(?:->|←|→|↔)\s*""")
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
    fun needsInstall(): Boolean = installedShellEntry() == null

    /**
     * 引导启动入口（[SHELL_ENTRIES] 中第一个可用的相对路径），没有则 null。
     *
     * 可用 = 存在且是 ELF 二进制或系统解释器脚本（后者经内核 shebang 直接执行）；
     * 私有目录 shebang 脚本不计入，其解释器本身尚不可用。
     *
     * 安装状态与安装日志都取自这里：两处若各写一份判定，就会出现「日志说装好了、
     * 状态说没装」同时挂在同一次安装上。
     */
    fun installedShellEntry(): String? = SHELL_ENTRIES.firstOrNull(::isUsableShellEntry)

    private fun isUsableShellEntry(relative: String): Boolean {
        val entry = File(prefixDir, relative)
        return entry.isFile && (isElf(entry) || isSystemShellScript(entry))
    }

    /** 安装状态只认启动入口存在性，不写任何标记文件。 */
    fun isInstalled(): Boolean = installedShellEntry() != null

    suspend fun install(zipFile: File): Result<Unit> = withContext(TerminalDispatchers.inputOutput) {
        // staging 区已存在即拒绝执行（DESIGN Bootstrap 节）：上次安装被杀掉留下的
        // 残留树来源不明，清空它可能删掉用户还没来得及排查的东西，复用它则会把新
        // 条目写进旧树、再由 atomicRename 发布成一份混血 prefix。
        //
        // 拒绝发生在 try 之外：失败路径的清理只针对本次创建的 staging 区，不能把
        // 别人的残留一起删掉。
        if (stagingDir.exists()) {
            LogUtil.error(TAG, "Install refused: staging dir already exists")
            return@withContext Result.failure(
                IOException("Bootstrap staging dir already exists"),
            )
        }
        try {
            // 现有 prefix 必须存活到新引导完全解压并原子换入为止
            // （见 atomicRename），否则安装失败会让用户完全没有可用的引导。
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
        } catch (exception: CancellationException) {
            // 取消不是安装失败：当成失败会让调用方继续走重试/报错路径，而协程此刻
            // 已经取消。原样抛出，交给结构化并发收尾。
            throw exception
        } catch (exception: Exception) {
            // 与 BootstrapDownloader 一致只记录异常类名：异常消息可能嵌入用户提供的路径。
            // 截断到 300 字符以便诊断而不泄露完整路径。
            LogUtil.error(
                "BootstrapInstaller",
                "Install failed: ${exception.javaClass.simpleName}: ${exception.message?.take(300)}",
            )
            // 丢弃部分解压的 staging 目录：它可能有数百 MB，
            // 而系统从不清理 filesDir，故安装失败会一直泄漏磁盘直到下次重试。
            try {
                clearStaging(stagingDir)
            } catch (cleanupException: Exception) {
                LogUtil.warn("BootstrapInstaller", "Failed to clean staging dir", cleanupException)
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
        FileInputStream(zipFile).use { fileInputStream ->
            ZipInputStream(fileInputStream).use { zipInputStream ->
                processZipEntries(zipInputStream, symlinks, executables) { entryIndex ->
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
            // 清单里没有「目录条目」形态，故与 zip 处的附加判据同形：
            // 消解后为空（`foo/..`、`.`）意味着目标其实是 staging 目录自身，
            // chmod 到目录上会让它带着 0755 进入 prefix。
            if (escapesStagingDir(executable) || resolvesToStagingRoot(executable)) {
                throw IOException("Unsafe executable path: $executable")
            }
            try {
                Os.chmod(File(stagingDir, executable).absolutePath, EXECUTABLE_FILE_MODE)
            } catch (exception: Exception) {
                LogUtil.warn(TAG, "EXECUTABLES.txt chmod failed for $executable", exception)
            }
        }
        return symlinks
    }

    private fun processZipEntries(
        zipInputStream: ZipInputStream,
        symlinks: MutableList<Pair<String, String>>,
        executables: MutableList<String>,
        onEntryProcessed: (Int) -> Unit,
    ) {
        var entry = zipInputStream.nextEntry
        var entryIndex = 0
        var totalExtractedBytes = 0L
        while (entry != null) {
            val name = entry.name
            // Zip 滑移防护：拒绝绝对路径与任何 ".." 段，
            // 使恶意/被篡改的引导归档无法写入 staging 目录之外
            // （例如覆盖 prefs/logs）。
            // 消解后为空表示条目指向 staging 根：目录条目（`./`）只是无害空操作，
            // 但作为普通文件写出时目标其实是 staging 目录自身，必须拒绝。
            if (escapesStagingDir(name) || (!entry.isDirectory && resolvesToStagingRoot(name))) {
                throw IOException("Unsafe zip entry name: $name")
            }
            if (name == "SYMLINKS.txt") {
                // 有界读取：该条目是元数据，必须很小；
                // 在恶意归档上无界 readBytes() 会让进程 OOM。
                val bytes = zipInputStream.readNBytes(MAX_METADATA_BYTES)
                if (bytes.size >= MAX_METADATA_BYTES) {
                    throw IOException("SYMLINKS.txt exceeds $MAX_METADATA_BYTES bytes")
                }
                symlinks.addAll(parseSymlinks(bytes.decodeToString()))
            } else if (name == "EXECUTABLES.txt") {
                val bytes = zipInputStream.readNBytes(MAX_METADATA_BYTES)
                if (bytes.size >= MAX_METADATA_BYTES) {
                    throw IOException("EXECUTABLES.txt exceeds $MAX_METADATA_BYTES bytes")
                }
                executables.addAll(
                    bytes
                        .decodeToString()
                        .lines()
                        .map { it.trim() }
                        .filter { it.isNotEmpty() },
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
                        val read = zipInputStream.read(buffer)
                        if (read < 0) break
                        entryBytes += read
                        if (entryBytes > MAX_EXTRACTED_BYTES) {
                            throw IOException(
                                "Bootstrap entry $name exceeds $MAX_EXTRACTED_BYTES bytes uncompressed",
                            )
                        }
                        totalExtractedBytes += read
                        // 全归档的 Zip 炸弹防护：1 GiB 的下载在高压缩比下
                        // 跨多个条目可膨胀到 TB 级。此处在累计总量上施加上限，
                        // 而不只是逐条目。
                        if (totalExtractedBytes > MAX_EXTRACTED_BYTES) {
                            throw IOException(
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
            entry = zipInputStream.nextEntry
        }
    }

    private fun isExecutable(name: String): Boolean = EXEC_PREFIXES.any { name.startsWith(it) }

    /**
     * 归档内路径是否逃出 staging 目录：绝对路径，或消解 `.`/`..` 后以 `..` 段开头。
     *
     * 必须真正消解点段后再看结果。此前只对原始字符串判前缀，而
     * `File("foo/..").path` 仍是 `foo/..`，四个前缀条件一个都不命中——
     * 该条目随后被当作普通文件写出，目标其实是 staging 目录自身（见
     * [resolvesToStagingRoot]）。
     *
     * 用 `Path.normalize()` 消解、`Path.startsWith()` 按**路径元素**比较：
     * 字符串前缀会把 `..foo` 误判成 `..`。
     */
    internal fun escapesStagingDir(path: String): Boolean {
        if (path.startsWith("/")) return true
        return Paths.get(path).normalize().startsWith("..")
    }

    /**
     * 消解 `.`/`..` 后是否正好落在 staging 目录自身（`foo/..`、`.`）。
     * 落在自身时 chmod/symlink/文件写出都会作用于 staging 目录，必须拒绝。
     *
     * 消解到底的结果是 `Path` 的空路径单例；它自报 `nameCount == 1`，
     * 故只能与 [Paths.get] 的空路径比较，不能数段。
     */
    internal fun resolvesToStagingRoot(path: String): Boolean = Paths.get(path).normalize() == Paths.get("")

    /**
     * 绝对符号链接目标重写到本机 prefix。
     *
     * 官方包的 SYMLINKS.txt 用旧式绝对路径（`/data/data/com.termux/files/usr/...`）。
     * 二级用户与多用户设备上本机 prefix 位于 `/data/user/<id>/<pkg>/files/usr`，
     * 归档里的绝对路径并不指向本机 prefix——照抄建链得到的是断链。
     *
     * 取 `files/usr/` 之后的后缀拼到本机 prefix；不含该分段的绝对路径原样返回，
     * 由调用方的规范路径校验决定接受与否。
     */
    internal fun resolveAbsoluteSymlinkTarget(target: String, canonicalPrefix: String): String {
        val relativeSuffix = target.substringAfter(FILES_USR_SEGMENT, "")
        return if (relativeSuffix.isNotEmpty()) File(canonicalPrefix, relativeSuffix).path else target
    }

    internal fun parseSymlinks(content: String): List<Pair<String, String>> = content
        .lines()
        .filter { it.isNotBlank() }
        .mapNotNull { line ->
            val parts = line.split(SYMLINK_SEPARATOR)
            if (parts.size == 2) parts[0].trim() to parts[1].trim() else null
        }

    private fun createSymlinks(symlinks: List<Pair<String, String>>) {
        for ((target, linkPath) in symlinks) {
            // 符号链接路径逃逸防护（与 zip 条目名同一规则）：
            // 恶意 SYMLINKS.txt 绝不能创建 staging 目录之外的链接，
            // 也不得让链接落在 staging 目录自身（`foo/..`）。
            if (escapesStagingDir(linkPath) || resolvesToStagingRoot(linkPath)) {
                throw IOException("Unsafe symlink path: $linkPath")
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
            val linkTarget =
                if (target.startsWith("/")) {
                    val canonicalPrefix = prefixDir.canonicalPath
                    val rewritten = resolveAbsoluteSymlinkTarget(target, canonicalPrefix)
                    val resolvedAbsolute =
                        try {
                            File(rewritten).canonicalPath
                        } catch (exception: Exception) {
                            throw IOException(
                                "Unsafe symlink target: $target (${exception.message})",
                                exception,
                            )
                        }
                    if (
                        resolvedAbsolute != canonicalPrefix && !resolvedAbsolute.startsWith("$canonicalPrefix/")
                    ) {
                        throw IOException("Unsafe symlink target: $target")
                    }
                    rewritten
                } else {
                    val linkParent = File(linkPath).parent
                    val resolvedTarget = if (linkParent != null) File(linkParent, target).path else target
                    // File.path 不消解 ".." 段（File("a/../b").path == "a/../b"），
                    // 故先用 Path.normalize 消解，再按路径元素判是否逃出 staging。
                    if (Paths.get(resolvedTarget).normalize().startsWith("..")) {
                        throw IOException("Unsafe symlink target: $target")
                    }
                    target
                }
            val linkFile = File(stagingDir, linkPath)
            linkFile.parentFile?.mkdirs()
            // 建链用重写后的目标：校验与写入必须是同一个路径，否则校验放行、链接却
            // 指向归档里的旧 prefix（换入本机 prefix 后即断链）。
            Os.symlink(linkTarget, linkFile.absolutePath)
        }
    }

    /** 上一次安装的旧目录：随机后缀备份，安装成功后保留，由用户手动删除，从不自动删除。 */
    private fun atomicRename() {
        val staging = stagingDir
        val prefix = prefixDir
        if (prefix.exists()) {
            // 旧目录先整体移入随机后缀备份（同文件系统 rename 为原子操作），再换入新目录；
            // 失败则恢复备份，旧环境保持可用；成功后备份保留，由用户手动删除。
            val backup = File(prefix.parentFile, "${prefix.name}.${UUID.randomUUID().toString().take(8)}")
            if (!prefix.renameTo(backup)) {
                throw IOException("Failed to move old prefix aside: ${prefix.path}")
            }
            val renamed = staging.renameTo(prefix)
            if (!renamed) {
                // 恢复旧 prefix，使先前的引导仍可用。
                if (!backup.renameTo(prefix)) {
                    throw IOException(
                        "Atomic rename failed and rollback failed: staging=${staging.path} prefix=${prefix.path} backup=${backup.path}",
                    )
                }
                throw IOException("Atomic rename failed: ${staging.path} -> ${prefix.path}")
            }
        } else if (!staging.renameTo(prefix)) {
            throw IOException("Atomic rename failed: ${staging.path} -> ${prefix.path}")
        }
    }

    private fun ensureHomeAndTmp() {
        homeDir.mkdirs()
        File(prefixDir, "tmp").mkdirs()
    }
}

/**
 * 删除一棵目录树，返回是否**全部**删净。
 *
 * 返回值必须被调用方检查：清不净的 staging 区会被下一次解压直接写进去，
 * 最终 `atomicRename` 把「旧引导残留 + 新引导条目」这份混血树当成 prefix 发布，
 * 而安装一路报成功。`listFiles()` 返回 null 不是「空目录」而是「列举不出来」
 * （权限/IO 失败）——那意味着一整棵子树根本没被遍历到。
 *
 * 用 `NOFOLLOW_LINKS` 判存在性：断链符号链接的 `exists()` 恒为 false，
 * 用它会让残链永远留在树里。
 */
private fun deleteTree(file: File): Boolean {
    if (!Files.exists(file.toPath(), LinkOption.NOFOLLOW_LINKS)) return true
    var clean = true
    // 删除时绝不跟随符号链接：指向目录的符号链接会解析为 isDirectory=true，
    // 于是列举并递归会删除*目标*的内容（数据丢失），
    // 而自指链接会无限递归（StackOverflowError）。
    // 符号链接只是一个 inode——删它本身，而非其指向。
    if (!Files.isSymbolicLink(file.toPath()) && file.isDirectory) {
        val children = file.listFiles()
        if (children == null) {
            LogUtil.warn("BootstrapInstaller", "Cannot list ${file.name}; subtree left in place")
            clean = false
        } else {
            children.forEach { if (!deleteTree(it)) clean = false }
        }
    }
    if (!file.delete()) {
        // 只记名与父目录名：条目名来自用户提供的 ZIP，带全路径会泄露到 logcat。
        LogUtil.warn("BootstrapInstaller", "Failed to delete ${file.name} under ${file.parentFile?.name}")
        clean = false
    }
    return clean
}
