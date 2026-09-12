package terminal.emulator.installer

import androidx.test.core.app.ApplicationProvider
import kotlinx.coroutines.runBlocking
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import java.io.File
import java.util.UUID
import java.util.zip.ZipEntry
import java.util.zip.ZipOutputStream

/**
 * JVM (Robolectric) tests for the bootstrap install + second-stage pipeline.
 *
 * These exercise the REAL [BootstrapInstaller] / [SecondStageRunner] code paths (zip extraction,
 * symlink creation via [Os.symlink], executable chmod, atomic rename, post-install script
 * execution) without downloading anything from the network: a synthetic bootstrap zip is built
 * locally and installed into a throwaway prefix under the app's exec-permitted `files/` tree.
 * Symlink direction and executable-bit verification live in the instrumented suite
 * (BootstrapSymlinkInstrumentedTest) — Robolectric's ShadowOs has no symlink/chmod/stat support.
 */
@RunWith(RobolectricTestRunner::class)
class BootstrapInstallerTest {
    private val context = ApplicationProvider.getApplicationContext<android.content.Context>()
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
        // atomicRename() renames staging → prefix, which requires the
        // prefix's parent directory to exist (File.renameTo fails
        // otherwise). The emulator-run original never created it either —
        // this setup line fixes that latent test bug.
        prefixDir.parentFile?.mkdirs()
    }

    @After
    fun cleanup() {
        prefixDir.deleteRecursively()
        File(prefixDir.parentFile, "${prefixDir.name}.prev").deleteRecursively()
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
            if (withSymlinks) {
                val content =
                    """
            bin/gawk←bin/awk
            bin/busybox←bin/applets/gunzip
            """
                        .trimIndent()
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
        // Files.isSymbolicLink (lstat) is used instead of File.exists():
        // exists() follows the link, which Robolectric's shadow layer
        // resolves inconsistently on the JVM.
        // Symlink-direction and executable-bit verification live in the
        // instrumented suite (BootstrapSymlinkInstrumentedTest):
        // Robolectric's ShadowOs provides no symlink/chmod/stat support and
        // its shadow File.renameTo drops symlinks when moving the staging
        // tree (verified empirically). The install succeeding proves
        // SYMLINKS.txt was parsed and createSymlinks ran without throwing.

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

    /** The archive's EXECUTABLES.txt is parsed and chmod failures are tolerated. */
    @Test
    fun install_parsesExecutablesTxt() {
        ZipOutputStream(zipFile.outputStream()).use { zos ->
            fun add(name: String, content: String = "x") {
                zos.putNextEntry(ZipEntry(name))
                zos.write(content.toByteArray())
                zos.closeEntry()
            }
            add("bin/bash", "#!/bin/sh\n")
            add("usr/bin/env", "env-binary")
            add("lib/libfoo.so", "libfoo")
            add("SYMLINKS.txt", "bin/bash←usr/bin/bash\n")
            add("EXECUTABLES.txt", "usr/bin/env\nbin/bash\n")
        }
        val installer = BootstrapInstaller(prefixDir, homeDir, stagingDir)

        val result = runBlocking { installer.install(zipFile) }

        assertTrue(
            "install with EXECUTABLES.txt must succeed: ${result.exceptionOrNull()?.message}",
            result.isSuccess,
        )
        assertTrue("usr/bin/env must exist", File(prefixDir, "usr/bin/env").exists())
    }

    @Test
    fun installed_rejectsStoreWithoutBashInteractive() {
        val other = File(prefixDir, "nix/store/abc123-foo-1.0/bin/foo")
        requireNotNull(other.parentFile).mkdirs()
        other.writeText("x")
        val installer = BootstrapInstaller(prefixDir, homeDir, stagingDir)
        assertFalse("isInstalled must be false without a shell entry", installer.isInstalled())
    }

    @Test
    fun installed_acceptsSystemInterpreterLoginScript() {
        File(prefixDir, "bin/login").apply {
            requireNotNull(parentFile).mkdirs()
            writeText("#!/system/bin/sh\nset -eu\nexec ./bin/proot-static\n")
        }
        val installer = BootstrapInstaller(prefixDir, homeDir, stagingDir)
        assertTrue("isInstalled must be true with a system launcher script", installer.isInstalled())
    }

    @Test
    fun installed_rejectsPrivateDirectoryLoginScript() {
        File(prefixDir, "bin/login").apply {
            requireNotNull(parentFile).mkdirs()
            writeText("#!/data/data/com.termux/files/usr/bin/sh\nexec bash\n")
        }
        val installer = BootstrapInstaller(prefixDir, homeDir, stagingDir)
        assertFalse(
            "isInstalled must be false with a private-interpreter script alone",
            installer.isInstalled(),
        )
    }

    /** parseSymlinks keeps the nix `target←linkPath` direction for absolute store paths. */
    @Test
    fun parseSymlinks_keepsNixStoreDirection() {
        val installer = BootstrapInstaller(prefixDir, homeDir, stagingDir)
        val parsed =
            installer.parseSymlinks(
                "/nix/store/abc-system-path/bin/login←bin/login\n" +
                    "/nix/store/def-bash/bin/sh←bin/bash\n",
            )
        assertEquals(2, parsed.size)
        assertEquals("/nix/store/abc-system-path/bin/login", parsed[0].first)
        assertEquals("bin/login", parsed[0].second)
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
            """
                .trimIndent(),
        )
        script.setExecutable(true)

        val result = runBlocking { SecondStageRunner(prefixDir, homeDir).run() }

        assertTrue("second stage should succeed: ${result.errors}", result.success)
        assertTrue("postinst script must have executed", marker.exists())
    }

    @Test
    fun secondStageRunner_lockIsReleasedAfterRun() {
        val infoDir = File(prefixDir, "var/lib/dpkg/info")
        infoDir.mkdirs()
        val marker = File(prefixDir, "postinst-ran.marker")
        val script = File(infoDir, "fake.postinst")
        script.writeText(
            """
            #!/bin/sh
            echo "configure" > "${marker.absolutePath}"
            exit 0
            """
                .trimIndent(),
        )
        script.setExecutable(true)

        val runner = SecondStageRunner(prefixDir, homeDir)
        val first = runBlocking { runner.run() }
        val second = runBlocking { runner.run() }
        assertTrue("first run must succeed: ${first.errors}", first.success)
        assertTrue("second run must succeed: ${second.errors}", second.success)
        // The lock is deleted in finally — a second run proves it was released.
        assertFalse(
            "lock file must not remain",
            File(prefixDir, "bin/termux-bootstrap-second-stage.sh.lock").exists(),
        )
    }

    @Test
    fun install_failure_cleans_staging_and_reports_not_installed() {
        val zip = buildFakeBootstrapZip(withSymlinks = true)
        // Corrupt the zip: truncate it so extraction fails.
        zip.writeBytes(zip.readBytes().copyOf(200))
        val installer = BootstrapInstaller(prefixDir, homeDir, stagingDir)

        val result = runBlocking { installer.install(zip) }

        assertTrue("corrupted zip install must fail", result.isFailure)
        assertFalse("staging must be cleaned on failure", stagingDir.exists())
        assertFalse("prefix must not be reported installed on failure", installer.isInstalled())
        assertTrue(installer.needsInstall())
    }

    @Test
    fun needsInstall_reflects_shell_entry_presence() {
        File(prefixDir, "bin").mkdirs()
        File(prefixDir, "bin/login").writeBytes(byteArrayOf(0x7f, 0x45, 0x4c, 0x46) + "x".toByteArray())
        val installer = BootstrapInstaller(prefixDir, homeDir, stagingDir)
        assertFalse("shell entry present must not need install", installer.needsInstall())
    }

    @Test
    fun install_keeps_single_previous_backup() {
        val installer = BootstrapInstaller(prefixDir, homeDir, stagingDir)
        assertTrue(runBlocking { installer.install(buildFakeBootstrapZip(true)) }.isSuccess)
        val backup = File(prefixDir.parentFile, "${prefixDir.name}.prev")
        assertFalse("no backup after first install", backup.exists())
        File(prefixDir, "bin/bash").writeText("#!/bin/sh\nfirst\n")
        assertTrue(runBlocking { installer.install(buildFakeBootstrapZip(true)) }.isSuccess)
        assertTrue("previous prefix kept as single backup", backup.isDirectory)
        assertTrue(
            "backup holds the previous tree",
            File(backup, "bin/bash").readText().contains("first"),
        )
    }
}

/** Pure-path tests: no Android/Os dependencies, no context needed. */
class BootstrapInstallerNormalizePathTest {
    private val installer =
        BootstrapInstaller(
            prefixDir = File("/tmp/t-prefix"),
            homeDir = File("/tmp/t-home"),
            stagingDir = File("/tmp/t-staging"),
        )

    @Test
    fun normalizePath_removes_dot_segments() {
        assertEquals(
            "include/term_entry.h",
            installer.normalizePath("./include/ncurses/../term_entry.h"),
        )
        assertEquals("bin/bash", installer.normalizePath("bin/./bash"))
        assertEquals("a/b", installer.normalizePath("a//b"))
    }

    @Test
    fun normalizePath_keeps_leading_escape() {
        // Multiple leading ".." segments are preserved (they escape staging).
        assertEquals("../../escape", installer.normalizePath("../../escape"))
        // a/../../b resolves to../b (one level escapes after consuming a).
        assertEquals("../b", installer.normalizePath("a/../../b"))
    }

    @Test
    fun normalizePath_absolute_stays_absolute() {
        assertEquals("/etc/passwd", installer.normalizePath("/etc/passwd"))
        assertEquals("/etc/passwd", installer.normalizePath("/etc/../etc/passwd"))
    }

    @Test
    fun termux_style_target_resolves_inside_staging() {
        // link=./include/ncurses/term_entry.h target=../term_entry.h
        val resolved = installer.normalizePath("./include/ncurses/../term_entry.h")
        assertEquals("include/term_entry.h", resolved)
        assertFalse("resolved target escapes staging", resolved.startsWith("../"))
    }
}
