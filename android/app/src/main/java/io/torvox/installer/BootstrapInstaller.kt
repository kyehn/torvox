package io.torvox.installer

import android.system.Os
import android.util.Log
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import java.io.File
import java.io.FileInputStream
import java.util.zip.ZipInputStream

class BootstrapInstaller(
    private val prefixDir: File,
    private val homeDir: File,
    private val stagingDir: File,
) {
    // SYMLINKS.txt format: "target <LEFT_ARROW> linkpath"
    private val symlinksDelimiter = "\u2190"

    companion object {
        const val COPY_BUFFER_SIZE = 8096
        const val EXECUTABLE_FILE_MODE = 0x1ED // rwxr-xr-x
        val EXEC_PREFIXES = listOf("bin/", "libexec/", "lib/apt/apt-helper", "lib/apt/methods/")
    }

    fun needsInstall(): Boolean = !File(prefixDir, "bin/bash").exists()

    fun isInstalled(): Boolean = File(prefixDir, "bin/bash").exists()

    suspend fun install(zipFile: File): Result<Unit> = withContext(Dispatchers.IO) {
        try {
            cleanupOld()
            createDirectories()
            val symlinks = extractZip(zipFile)
            if (symlinks.isEmpty()) {
                return@withContext Result.failure(Exception("No SYMLINKS.txt found in bootstrap ZIP"))
            }
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
        delete(stagingDir)
        if (prefixDir.exists()) {
            delete(prefixDir)
        }
    }

    private fun createDirectories() {
        stagingDir.mkdirs()
        prefixDir.mkdirs()
    }

    private fun extractZip(zipFile: File): List<Pair<String, String>> {
        val symlinks = mutableListOf<Pair<String, String>>()
        FileInputStream(zipFile).use { fis ->
            ZipInputStream(fis).use { zis ->
                processZipEntries(zis, symlinks)
            }
        }
        return symlinks
    }

    private fun processZipEntries(
        zis: ZipInputStream,
        symlinks: MutableList<Pair<String, String>>,
    ) {
        var entry = zis.nextEntry
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
                    Os.chmod(targetFile.absolutePath, EXECUTABLE_FILE_MODE) // rwxr-xr-x
                }
            }
            entry = zis.nextEntry
        }
    }

    private fun parseSymlinks(content: String): List<Pair<String, String>> = content.lines().filter { it.isNotBlank() }.mapNotNull { line ->
        val parts = line.split(symlinksDelimiter)
        if (parts.size == 2) parts[0].trim() to parts[1].trim() else null
    }

    private fun isExecutable(name: String): Boolean = EXEC_PREFIXES.any { name.startsWith(it) } ||
        name.startsWith("lib/apt/methods/")

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
