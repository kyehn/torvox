package terminal.emulator.installer

import android.system.Os
import android.util.Log
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import terminal.emulator.runtime.isElf
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
    // — extract to usr.tmp/ then write a.bootstrap-version.json marker whose
    // value is the bootstrap zip's sha256, so a kill-mid-extract can never look
    // "installed" and a corrupted zip is detected on next launch. torvox stages
    // into stagingDir then renames (see installBootstrap); the marker makes
    // the corrupted-zip detection deterministic.
    companion object {
        private const val TAG = "BootstrapInstaller"
        const val COPY_BUFFER_SIZE = 8096
        const val MAX_SYMLINKS_BYTES = 1024 * 1024
        const val EXECUTABLE_FILE_MODE = 0x1ED
        val EXEC_PREFIXES = listOf("bin/", "libexec/", "lib/apt/apt-helper", "lib/apt/methods/")
        private const val EXTRACT_PROGRESS_INTERVAL = 10

        // Zip-bomb guard: cap total uncompressed payload. A real bootstrap
        // is ~150 MB; the limit gives headroom while preventing a hostile
        // archive from filling the data partition.
        private const val MAX_EXTRACTED_BYTES = 1L * 1024 * 1024 * 1024

        // Install marker (warp bootstrap.rs VERSION_PIN_FILENAME analog):
        // stores the sha256 of the zip that produced the prefix, written
        // AFTER the atomic rename so a kill-mid-extract leaves no marker.
        internal const val VERSION_PIN_FILENAME = ".bootstrap-version.json"

        /**
         * SHA-256 of [file], streamed (constant memory; a bootstrap zip is
         * ~300MB). Throws IOException on read failure.
         */
        internal fun sha256Of(file: File): String {
            val digest = java.security.MessageDigest.getInstance("SHA-256")
            FileInputStream(file).use { input ->
                val buffer = ByteArray(COPY_BUFFER_SIZE)
                while (true) {
                    val read = input.read(buffer)
                    if (read < 0) break
                    digest.update(buffer, 0, read)
                }
            }
            return digest.digest().joinToString("") { "%02x".format(it) }
        }

        /** Parse the marker's sha256; null when absent or malformed. */
        internal fun readVersionPin(prefixDir: File): String? {
            val marker = File(prefixDir, VERSION_PIN_FILENAME)
            if (!marker.isFile) return null
            return try {
                val text = marker.readText()
                val key = "\"sha256\":"
                val idx = text.indexOf(key)
                if (idx < 0) {
                    null
                } else {
                    val valueStart = text.indexOf('"', idx + key.length)
                    val valueEnd = valueStart.let { s -> if (s < 0) -1 else text.indexOf('"', s + 1) }
                    if (valueStart < 0 || valueEnd < 0) null else text.substring(valueStart + 1, valueEnd)
                }
            } catch (_: Exception) {
                null
            }
        }

        /** Atomically write the version pin (temp + rename, no torn marker). */
        internal fun writeVersionPin(prefixDir: File, sha256: String) {
            val marker = File(prefixDir, VERSION_PIN_FILENAME)
            val tmp = File(prefixDir, "$VERSION_PIN_FILENAME.tmp")
            tmp.writeText(
                """{"sha256":"$sha256","installedAt":${System.currentTimeMillis()}}""",
            )
            if (!tmp.renameTo(marker)) {
                tmp.delete()
                throw java.io.IOException("Failed to write version pin")
            }
        }
    }

    /**
     * True when the prefix must be (re-)installed. With [zipSha256] (the
     * sha256 of the zip about to be installed): marker missing OR marker
     * mismatch → true; matching marker → false. Without it, falls back to
     * the shell-binary check so callers that never pass a hash (e.g. the
     * second-stage-only path) keep working.
     */
    fun needsInstall(zipSha256: String? = null): Boolean {
        if (zipSha256 != null) {
            val pinned = readVersionPin(prefixDir) ?: return true
            return pinned != zipSha256
        }
        return !(
            (File(prefixDir, "bin/login").isFile && isElf(File(prefixDir, "bin/login"))) ||
                File(prefixDir, "bin/bash").exists()
            )
    }

    /**
     * A bootstrap is installed when its shell binary and the second-stage
     * termux.env both exist. The shell binary is login-first (nix-on-droid)
     * with bash fallback (termux), mirroring TerminalRuntime's resolution;
     * only real ELF binaries qualify (termux's bin/login is a shebang
     * script, nix's is a static ELF).
     */
    fun isInstalled(): Boolean = // termux.env is written last by the second stage; requiring it here
        // means a failed/wedged second stage (e.g. writeTermuxEnv hitting a
        // full disk) is not reported as a healthy install, so the retry path
        // stays open instead of leaving a permanently broken environment.
        (
            (File(prefixDir, "bin/login").isFile && isElf(File(prefixDir, "bin/login"))) ||
                File(prefixDir, "bin/bash").exists()
            ) &&
            File(prefixDir, "etc/termux/termux.env").exists()

    suspend fun install(zipFile: File): Result<Unit> = withContext(Dispatchers.IO) {
        // hash BEFORE extraction so a corrupted zip is detected
        // even if extraction never completes (and so needsInstall(zipSha256)
        // can compare). ~1s per 300MB on emulator — first-launch-only.
        val zipSha256 =
            try {
                sha256Of(zipFile)
            } catch (exception: Exception) {
                return@withContext Result.failure(Exception("Failed to hash bootstrap zip: ${exception.message}"))
            }
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
            // Marker AFTER the atomic rename: a kill-mid-extract leaves no
            // marker, so the next launch re-installs instead of trusting a
            // half-extracted tree (warp bootstrap.rs step 8).
            writeVersionPin(prefixDir, zipSha256)
            Result.success(Unit)
        } catch (exception: Exception) {
            // Log the class only, consistent with BootstrapDownloader: the
            // exception message can embed user-supplied paths.
            // TEMP-DEBUG: include the message truncated to
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
        val executables = mutableListOf<String>()
        val totalEntries = ZipFile(zipFile).use { it.size() }
        var lastReportedEntry = 0
        FileInputStream(zipFile).use { fis ->
            ZipInputStream(fis).use { zis ->
                processZipEntries(zis, symlinks, executables) { entryIndex ->
                    if (entryIndex - lastReportedEntry >= EXTRACT_PROGRESS_INTERVAL || entryIndex == totalEntries) {
                        lastReportedEntry = entryIndex
                        onProgress?.onProgress(BootstrapProgress.Extracting(entryIndex, totalEntries))
                    }
                }
            }
        }
        // nix-on-droid bootstraps keep most executables under
        // nix/store/<hash>/bin/ and usr/bin/, which the EXEC_PREFIXES
        // prefix match cannot see — the archive's EXECUTABLES.txt is the
        // authoritative list for those (matches termux-app
        // TermuxInstaller.java:233-240).
        for (executable in executables) {
            try {
                Os.chmod(File(stagingDir, executable).absolutePath, EXECUTABLE_FILE_MODE)
            } catch (exception: Exception) {
                Log.w(TAG, "EXECUTABLES.txt chmod failed for $executable", exception)
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
            //     ;
            //  2. ABSOLUTE targets into the final prefix
            //     (`/data/data/com.termux/files/usr/share/...`), which are
            //     broken during staging but become valid once the staging
            //     dir is atomically renamed to `files/usr`. Only allow
            //     absolute targets that resolve inside the canonical
            //     prefix path.
            if (target.startsWith("/")) {
                // nix-on-droid bootstrap (bootstrap-aarch64.zip): SYMLINKS.txt
                // uses absolute targets like `/nix/store/<hash>-<pkg>/bin/...`
                // that only resolve inside the proot environment whose root is
                // the prefix. They are safe here because delete() never follows
                // symlinks (only the link inode is removed), and the links are
                // inert until a proot session resolves them. Restrict to the
                // /nix/ tree — anything else keeps the strict prefix check.
                if (!target.startsWith("/nix/")) {
                    val canonicalPrefix = prefixDir.canonicalPath
                    val resolvedAbsolute =
                        try {
                            File(target).canonicalPath
                        } catch (exception: Exception) {
                            throw java.io.IOException("Unsafe symlink target: $target (${exception.message})", exception)
                        }
                    if (resolvedAbsolute != canonicalPrefix &&
                        !resolvedAbsolute.startsWith("$canonicalPrefix/")
                    ) {
                        throw java.io.IOException("Unsafe symlink target: $target")
                    }
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
