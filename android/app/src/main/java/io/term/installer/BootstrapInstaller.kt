package io.term.installer

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
    companion object {
        const val COPY_BUFFER_SIZE = 8096
        const val EXECUTABLE_FILE_MODE = 0x1ED
        val EXEC_PREFIXES = listOf("bin/", "libexec/", "lib/apt/apt-helper", "lib/apt/methods/")
        private const val EXTRACT_PROGRESS_INTERVAL = 10
    }

    fun needsInstall(): Boolean = !File(prefixDir, "bin/bash").exists()

    fun isInstalled(): Boolean = File(prefixDir, "bin/bash").exists()

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
            Log.e("BootstrapInstaller", "Install failed", exception)
            Result.failure(exception)
        }
    }

    private fun cleanupOld() {
        // Only clear the staging area. The existing prefix must survive until the
        // new bootstrap is fully extracted and atomically swapped in (see atomicRename),
        // otherwise a failed install would leave the user with no working bootstrap.
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
        while (entry != null) {
            val name = entry.name
            if (name == "SYMLINKS.txt") {
                symlinks.addAll(parseSymlinks(zis.readBytes().decodeToString()))
            } else if (entry.isDirectory) {
                File(stagingDir, name).mkdirs()
            } else {
                val targetFile = File(stagingDir, name)
                targetFile.parentFile?.mkdirs()
                targetFile.outputStream().use { out -> zis.copyTo(out, COPY_BUFFER_SIZE) }
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

    internal fun parseSymlinks(content: String): List<Pair<String, String>> = content.lines().filter { it.isNotBlank() }.mapNotNull { line ->
        val parts = line.split(symlinkSeparator)
        if (parts.size == 2) parts[0].trim() to parts[1].trim() else null
    }

    private fun createSymlinks(symlinks: List<Pair<String, String>>) {
        for ((target, linkPath) in symlinks) {
            val linkFile = File(stagingDir, linkPath)
            linkFile.parentFile?.mkdirs()
            Os.symlink(target, linkFile.absolutePath)
        }
    }

    private fun atomicRename() {
        val staging = stagingDir
        val prefix = prefixDir
        if (prefix.exists()) {
            delete(prefix)
        }
        if (!staging.renameTo(prefix)) {
            throw Exception("Atomic rename failed: ${staging.path} -> ${prefix.path}")
        }
    }

    private fun ensureHomeAndTmp() {
        homeDir.mkdirs()
        File(prefixDir, "tmp").mkdirs()
    }

    private fun delete(file: File) {
        if (file.isDirectory) {
            file.listFiles()?.forEach { delete(it) }
        }
        file.delete()
    }
}
