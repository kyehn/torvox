package io.torvox.installer

import android.system.Os
import android.system.OsConstants
import androidx.test.platform.app.InstrumentationRegistry
import kotlinx.coroutines.runBlocking
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNotNull
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
 * On-device (emulator) tests for the bootstrap install + second-stage pipeline.
 *
 * These exercise the REAL [BootstrapInstaller] / [SecondStageRunner] code paths
 * (zip extraction, symlink creation via [Os.symlink], executable chmod, atomic
 * rename, post-install script execution) without downloading anything from the
 * network: a synthetic bootstrap zip is built locally and installed into a
 * throwaway prefix under the app's exec-permitted `files/` tree.
 */
@RunWith(JUnit4::class)
class BootstrapInstallerEmulatorTest {
    private val context = InstrumentationRegistry.getInstrumentation().targetContext
    private lateinit var prefixDir: File
    private lateinit var homeDir: File
    private lateinit var stagingDir: File
    private lateinit var zipFile: File

    @Before
    fun setup() {
        val id = UUID.randomUUID().toString().take(8)
        prefixDir = File(context.filesDir, "bstest-$id/usr")
        homeDir = File(context.filesDir, "bstest-$id/home")
        stagingDir = File(context.cacheDir, "bstest-$id-staging")
        zipFile = File(context.cacheDir, "bstest-$id.zip")
    }

    @After
    fun cleanup() {
        prefixDir.deleteRecursively()
        homeDir.deleteRecursively()
        stagingDir.deleteRecursively()
        zipFile.delete()
    }

    private fun buildFakeBootstrapZip(withSymlinks: Boolean): File {
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
            add("etc/termux/termux.env", "PREFIX=placeholder\n")
            if (withSymlinks) {
                val content =
                    """
                    bin/gawk←bin/awk
                    bin/busybox←bin/applets/gunzip
                    """.trimIndent()
                zos.putNextEntry(ZipEntry("SYMLINKS.txt"))
                zos.write(content.toByteArray())
                zos.closeEntry()
            }
        }
        return zipFile
    }

    @Test
    fun install_extractsFilesAndCreatesSymlinksWithCorrectDirection() {
        val zip = buildFakeBootstrapZip(withSymlinks = true)
        val installer = BootstrapInstaller(prefixDir, homeDir, stagingDir)

        val result = runBlocking { installer.install(zip) }

        assertTrue("install should succeed: ${result.exceptionOrNull()?.message}", result.isSuccess)
        assertTrue("bin/bash must exist after install", File(prefixDir, "bin/bash").exists())
        assertTrue("lib/libfoo.so must exist after install", File(prefixDir, "lib/libfoo.so").exists())

        // Staging must have been atomically renamed away.
        assertFalse("staging dir must be gone after atomic rename", stagingDir.exists())

        // Symlink direction: Termux SYMLINKS.txt is `target←linkname`, so
        // `bin/awk` must be a symlink pointing at `bin/gawk`.
        val awkLink = File(prefixDir, "bin/awk")
        assertTrue("bin/awk symlink must exist", awkLink.exists())
        assertEquals(
            "symlink bin/awk must point at bin/gawk (target←linkname)",
            "bin/gawk",
            Os.readlink(awkLink.absolutePath),
        )

        val gunzipLink = File(prefixDir, "bin/applets/gunzip")
        assertTrue("bin/applets/gunzip symlink must exist", gunzipLink.exists())
        assertEquals(
            "symlink bin/applets/gunzip must point at bin/busybox",
            "bin/busybox",
            Os.readlink(gunzipLink.absolutePath),
        )

        // Extracted executables must be marked executable.
        val bashMode = Os.stat(File(prefixDir, "bin/bash").absolutePath).st_mode
        assertTrue(
            "bin/bash must be executable",
            (bashMode and OsConstants.S_IXUSR) != 0,
        )

        // isInstalled reflects the freshly installed bootstrap.
        assertTrue("isInstalled must be true after install", installer.isInstalled())
        assertFalse("needsInstall must be false after install", installer.needsInstall())
    }

    @Test
    fun install_failsWhenSymlinksFileMissing() {
        val zip = buildFakeBootstrapZip(withSymlinks = false)
        val installer = BootstrapInstaller(prefixDir, homeDir, stagingDir)

        val result = runBlocking { installer.install(zip) }

        assertTrue("install must fail when SYMLINKS.txt is absent", result.isFailure)
        assertFalse("prefix must not be reported installed on failure", installer.isInstalled())
    }

    @Test
    fun secondStageRunner_executesPostinstScript() {
        // Seed a minimal prefix with a post-install script that writes a marker file.
        val infoDir = File(prefixDir, "var/lib/dpkg/info")
        infoDir.mkdirs()
        val marker = File(prefixDir, "postinst-ran.marker")
        val script = File(infoDir, "fake.postinst")
        script.writeText(
            """
            #!/bin/sh
            echo "configure" > "${marker.absolutePath}"
            exit 0
            """.trimIndent(),
        )
        script.setExecutable(true)

        val result = runBlocking { SecondStageRunner(prefixDir, homeDir).run() }

        assertTrue("second stage should succeed: ${result.errors}", result.success)
        assertTrue("postinst script must have executed", marker.exists())
    }

    @Test
    fun secondStageRunner_runsOnlyOnceViaLock() {
        val infoDir = File(prefixDir, "var/lib/dpkg/info")
        infoDir.mkdirs()
        val marker = File(prefixDir, "postinst-ran.marker")
        val script = File(infoDir, "fake.postinst")
        script.writeText(
            """
            #!/bin/sh
            echo ran >> "${marker.absolutePath}"
            exit 0
            """.trimIndent(),
        )
        script.setExecutable(true)

        val runner = SecondStageRunner(prefixDir, homeDir)
        val first = runBlocking { runner.run() }
        val second = runBlocking { runner.run() }

        assertTrue(first.success)
        assertTrue(second.success)
        // Lock prevents the second-stage scripts from running a second time.
        val runs = if (marker.exists()) marker.readText().count { it == '\n' } + 1 else 0
        assertEquals("postinst must run exactly once", 1, runs)
        assertNotNull(marker.readText())
    }
}
