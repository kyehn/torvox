package terminal.emulator.installer

import android.system.Os
import androidx.test.platform.app.InstrumentationRegistry
import kotlinx.coroutines.runBlocking
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Test
import org.junit.runner.RunWith
import org.junit.runners.JUnit4
import java.io.File
import java.util.UUID
import java.util.zip.ZipEntry
import java.util.zip.ZipOutputStream

/**
 * Emulator-only symlink-direction verification for the bootstrap install.
 *
 * The full install pipeline (zip extraction, postinst, lock) runs as
 * Robolectric unit tests (BootstrapInstallerTest); this file keeps ONLY the
 * part Robolectric cannot verify: Robolectric's shadow File.renameTo drops
 * symlinks when moving the staging tree, so link targets must be checked on
 * a real device where rename(2) preserves them.
 */
@RunWith(JUnit4::class)
class BootstrapSymlinkInstrumentedTest {
    private val context = InstrumentationRegistry.getInstrumentation().targetContext
    private lateinit var prefixDir: File
    private lateinit var homeDir: File
    private lateinit var stagingDir: File
    private lateinit var zipFile: File

    @Before
    fun setup() {
        val id = UUID.randomUUID().toString().take(8)
        prefixDir = File(context.filesDir, "bssym-$id/usr")
        prefixDir.parentFile?.mkdirs()
        homeDir = File(context.filesDir, "bssym-$id/home")
        stagingDir = File(context.cacheDir, "bssym-$id-staging")
        zipFile = File(context.cacheDir, "bssym-$id.zip")
    }

    @After
    fun cleanup() {
        prefixDir.deleteRecursively()
        homeDir.deleteRecursively()
        stagingDir.deleteRecursively()
        zipFile.delete()
    }

    @Test
    fun symlinks_point_at_configured_targets_after_install() {
        ZipOutputStream(zipFile.outputStream()).use { zos ->
            fun add(
                name: String,
                content: String = "x",
            ) {
                zos.putNextEntry(ZipEntry(name))
                zos.write(content.toByteArray())
                zos.closeEntry()
            }
            add("bin/bash", "#!/bin/sh\necho bash\n")
            add("bin/gawk", "gawk-binary")
            add("bin/busybox", "busybox-binary")
            add("lib/libfoo.so", "libfoo")
            add(
                "SYMLINKS.txt",
                """
                bin/gawk←bin/awk
                bin/busybox←bin/applets/gunzip
                """.trimIndent(),
            )
        }

        val installer = BootstrapInstaller(prefixDir, homeDir, stagingDir)
        val result = runBlocking { installer.install(zipFile) }
        assertTrue("install should succeed: ${result.exceptionOrNull()?.message}", result.isSuccess)

        assertEquals(
            "symlink bin/awk must point at bin/gawk (target←linkname)",
            "bin/gawk",
            Os.readlink(File(prefixDir, "bin/awk").absolutePath),
        )
        assertEquals(
            "symlink bin/applets/gunzip must point at bin/busybox",
            "bin/busybox",
            Os.readlink(File(prefixDir, "bin/applets/gunzip").absolutePath),
        )

        // Extracted executables must be marked executable (real chmod).
        val bashMode = Os.stat(File(prefixDir, "bin/bash").absolutePath).st_mode
        assertTrue(
            "bin/bash must be executable",
            (bashMode and android.system.OsConstants.S_IXUSR) != 0,
        )
    }
}
