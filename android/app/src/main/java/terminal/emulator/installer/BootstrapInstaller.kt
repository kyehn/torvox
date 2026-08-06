package terminal.emulator.installer

import android.system.Os
import android.util.Log
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import java.io.File
import java.io.FileInputStream
import java.util.zip.ZipFile
import java.util.zip.ZipInputStream

class BootstrapInstaller(
    private val prefixDir: File,
    private val homeDir: File,
    private val stagingDir: File,
    private val onProgress: BootstrapProgressCallback? = null,
) {
    // Atomic-install reference: warp-mobile-android crates/android-host/src/bootstrap.rs:1-48
    // — extract to usr.tmp/ then write a .bootstrap-version.json marker whose
    // value is the bootstrap zip's sha256, so a kill-mid-extract can never look
    // "installed" and a corrupted zip is detected on next launch. torvox stages
    // into stagingDir then renames (see installBootstrap); adding the sha256
    // sidecar is a P0 hardening item (docs/reference/research-warp.md §3).
    companion object {
        const val COPY_BUFFER_SIZE = 8096
        const val MAX_SYMLINKS_BYTES = 1024 * 1024
        const val EXECUTABLE_FILE_MODE = 0x1ED
        val EXEC_PREFIXES = listOf("bin/", "libexec/", "lib/apt/apt-helper", "lib/apt/methods/")
        private const val EXTRACT_PROGRESS_INTERVAL = 10

        // Zip-bomb guard: cap total uncompressed payload. A real bootstrap
        // is ~150 MB; the limit gives headroom while preventing a hostile
        // archive from filling the data partition.
        private const val MAX_EXTRACTED_BYTES = 1L * 1024 * 1024 * 1024
    }

    fun needsInstall(): Boolean = !File(prefixDir, "bin/bash").exists()

    fun isInstalled(): Boolean = // termux.env is written last by the second stage; requiring it here
        // means a failed/wedged second stage (e.g. writeTermuxEnv hitting a
        // full disk) is not reported as a healthy install, so the retry path
        // stays open instead of leaving a permanently broken environment.
        File(prefixDir, "bin/bash").exists() &&
            File(prefixDir, "etc/termux/termux.env").exists()

    suspend fun install(zipFile: File): Result<Unit> = withContext(Dispatchers.IO) {
        try {
            cleanupOld()
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
            // Log the class only, consistent with BootstrapDownloader: the
            // exception message can embed user-supplied paths (round-104).
            // TEMP-DEBUG (round-215): include the message truncated to
            // diagnose the emulator IOException.
            Log.e(
                "BootstrapInstaller",
                "Install failed: ${exception.javaClass.simpleName}: ${exception.message?.take(300)}",
            )
            // Discard the partially extracted staging dir: it can be
            // hundreds of MB and the system never clears filesDir, so a
            // failed install would leak disk until the next retry.
            try {
                delete(stagingDir)
            } catch (cleanupException: Exception) {
                Log.w("BootstrapInstaller", "Failed to clean staging dir", cleanupException)
            }
            Result.failure(exception)
        }
    }

    private fun cleanupOld() {
        // Only clear the staging area. The existing prefix must survive until the
        // new bootstrap is fully extracted and atomically swapped in (see atomicRename),
        // otherwise a failed install would leave the user with no working bootstrap.
        // This staging + atomic-swap design matches termux TermuxInstaller.java:137-257
        // (staging dir + SYMLINKS.txt + renameTo atomic switch + rollback).
        delete(stagingDir)
    }

    private fun createDirectories() {
        stagingDir.mkdirs()
    }

    private fun extractZip(zipFile: File): List<Pair<String, String>> {
        val symlinks = mutableListOf<Pair<String, String>>()
        val totalEntries = ZipFile(zipFile).use { it.size() }
        var lastReportedEntry = 0
        FileInputStream(zipFile).use { fis ->
            ZipInputStream(fis).use { zis ->
                processZipEntries(zis, symlinks) { entryIndex ->
                    if (entryIndex - lastReportedEntry >= EXTRACT_PROGRESS_INTERVAL || entryIndex == totalEntries) {
                        lastReportedEntry = entryIndex
                        onProgress?.onProgress(BootstrapProgress.Extracting(entryIndex, totalEntries))
                    }
                }
            }
        }
        return symlinks
    }

    private fun processZipEntries(
        zis: ZipInputStream,
        symlinks: MutableList<Pair<String, String>>,
        onEntryProcessed: (Int) -> Unit,
    ) {
        var entry = zis.nextEntry
        var entryIndex = 0
        var totalExtractedBytes = 0L
        while (entry != null) {
            val name = entry.name
            // Zip-slip guard: reject absolute paths and any ".." segment
            // so a malicious/tampered bootstrap archive cannot write
            // outside the staging directory (e.g. overwrite prefs/logs).
            val normalized = File(name).path
            if (name.startsWith("/") || normalized == ".." ||
                normalized.startsWith("../") || normalized.contains("/../")
            ) {
                throw java.io.IOException("Unsafe zip entry name: $name")
            }
            if (name == "SYMLINKS.txt") {
                // Bounded read: the entry is metadata and must be small;
                // an unbounded readBytes() on a hostile archive OOMs the
                // process.
                val bytes = zis.readNBytes(MAX_SYMLINKS_BYTES)
                if (bytes.size >= MAX_SYMLINKS_BYTES) {
                    throw java.io.IOException("SYMLINKS.txt exceeds $MAX_SYMLINKS_BYTES bytes")
                }
                symlinks.addAll(parseSymlinks(bytes.decodeToString()))
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
                            throw java.io.IOException("Bootstrap entry $name exceeds $MAX_EXTRACTED_BYTES bytes uncompressed")
                        }
                        totalExtractedBytes += read
                        // Zip-bomb guard across the whole archive: a 1 GiB
                        // download with a high compression ratio can expand
                        // to TBs across many entries. The cap is enforced
                        // here on the cumulative total, not just per entry.
                        if (totalExtractedBytes > MAX_EXTRACTED_BYTES) {
                            throw java.io.IOException("Bootstrap archive exceeds $MAX_EXTRACTED_BYTES bytes total uncompressed")
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

    private fun isExecutable(name: String): Boolean = EXEC_PREFIXES.any { name.startsWith(it) } ||
        name.startsWith("lib/apt/methods/")

    internal val symlinkSeparator = Regex("""\s*(?:->|←|→|↔)\s*""")

    /** Resolve `.`/`..` segments without touching the filesystem. */
    internal fun normalizePath(path: String): String {
        val absolute = path.startsWith("/")
        val stack = ArrayDeque<String>()
        for (part in path.split('/')) {
            when (part) {
                "", "." -> {}

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

    internal fun parseSymlinks(content: String): List<Pair<String, String>> = content.lines().filter { it.isNotBlank() }.mapNotNull { line ->
        val parts = line.split(symlinkSeparator)
        if (parts.size == 2) parts[0].trim() to parts[1].trim() else null
    }

    private fun createSymlinks(symlinks: List<Pair<String, String>>) {
        for ((target, linkPath) in symlinks) {
            // Symlink path escape guard (same rule as zip entry names):
            // a hostile SYMLINKS.txt must not be able to create links
            // outside the staging directory.
            val normalized = File(linkPath).path
            if (linkPath.startsWith("/") || normalized == ".." ||
                normalized.startsWith("../") || normalized.contains("/../")
            ) {
                throw java.io.IOException("Unsafe symlink path: $linkPath")
            }
            // The target is also attacker-controlled. Reject absolute
            // paths and traversal so a link cannot point outside the
            // staging tree — otherwise the recursive delete() below
            // (staging cleanup / backup removal) would follow the link
            // and wipe arbitrary directories.
            // Two legitimate Termux SYMLINKS.txt shapes exist:
            //  1. relative targets resolved against the LINK's parent dir
            //     (`../term_entry.h` from `include/ncurses/` resolves to
            //     `include/term_entry.h`, inside staging) — the naive
            //     startsWith("../") check wrongly rejected those
            //     (round-215);
            //  2. ABSOLUTE targets into the final prefix
            //     (`/data/data/com.termux/files/usr/share/...`), which are
            //     broken during staging but become valid once the staging
            //     dir is atomically renamed to `files/usr`. Only allow
            //     absolute targets that resolve inside the canonical
            //     prefix path.
            if (target.startsWith("/")) {
                val canonicalPrefix = prefixDir.canonicalPath
                val resolvedAbsolute =
                    try {
                        File(target).canonicalPath
                    } catch (exception: Exception) {
                        throw java.io.IOException("Unsafe symlink target: $target")
                    }
                if (resolvedAbsolute != canonicalPrefix &&
                    !resolvedAbsolute.startsWith("$canonicalPrefix/")
                ) {
                    throw java.io.IOException("Unsafe symlink target: $target")
                }
            } else {
                val linkParent = File(linkPath).parent
                val resolvedTarget =
                    if (linkParent != null) File(linkParent, target).path else target
                // Java File.path does NOT normalize ".." segments
                // (File("a/../b").path == "a/../b"), so resolve them manually
                // before the escape check.
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

    private fun atomicRename() {
        val staging = stagingDir
        val prefix = prefixDir
        if (prefix.exists()) {
            // Move the old prefix aside first (rename is atomic on the same
            // filesystem), then swap in the new one. Deleting the old prefix
            // before renaming leaves a window where a process kill loses the
            // working bootstrap with no way to recover except a 150 MB
            // re-download.
            val backupName = "${prefix.name}.old-${System.currentTimeMillis()}"
            val backup = File(prefix.parentFile, backupName)
            if (!prefix.renameTo(backup)) {
                throw Exception("Failed to move old prefix aside: ${prefix.path}")
            }
            val renamed = staging.renameTo(prefix)
            if (!renamed) {
                // Restore the old prefix so the previous bootstrap stays usable.
                backup.renameTo(prefix)
                throw Exception("Atomic rename failed: ${staging.path} -> ${prefix.path}")
            }
            delete(backup)
        } else if (!staging.renameTo(prefix)) {
            throw Exception("Atomic rename failed: ${staging.path} -> ${prefix.path}")
        }
    }

    private fun ensureHomeAndTmp() {
        homeDir.mkdirs()
        File(prefixDir, "tmp").mkdirs()
    }

    private fun delete(file: File) {
        // Never follow symlinks while deleting: a symlink pointing at a
        // directory resolves as isDirectory=true, so listing and recursing
        // would delete the *target's* contents (data loss) and a self-
        // referential link would recurse forever (StackOverflowError).
        // A symlink is just an inode — delete it, not its destination.
        if (!java.nio.file.Files.isSymbolicLink(file.toPath()) && file.isDirectory) {
            file.listFiles()?.forEach { delete(it) }
        }
        file.delete()
    }
}
