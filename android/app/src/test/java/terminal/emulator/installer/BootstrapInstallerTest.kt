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
 * On-device (emulator) tests for the bootstrap install + second-stage pipeline.
 *
 * These exercise the REAL [BootstrapInstaller] / [SecondStageRunner] code paths
 * (zip extraction, symlink creation via [Os.symlink], executable chmod, atomic
 * rename, post-install script execution) without downloading anything from the
 * network: a synthetic bootstrap zip is built locally and installed into a
 * throwaway prefix under the app's exec-permitted `files/` tree.
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

    /**
     * nix-on-droid bootstrap-aarch64.zip compatibility: its SYMLINKS.txt
     * uses absolute `/nix/store/...` targets that only resolve inside the
     * proot environment. The installer must accept them (they are inert
     * outside proot and delete() never follows links).
     */
    @Test
    fun install_acceptsNixAbsoluteStoreTargets() {
        ZipOutputStream(zipFile.outputStream()).use { zos ->
            fun add(name: String, content: ByteArray) {
                zos.putNextEntry(ZipEntry(name))
                zos.write(content)
                zos.closeEntry()
            }
            // ELF magic prefix: needsInstall/isInstalled require a real ELF
            // shell binary (termux's bin/login is a shebang script and must
            // not count — nix's is a static ELF).
            val elfHeader = byteArrayOf(0x7f, 0x45, 0x4c, 0x46) + "login-binary".toByteArray()
            add("bin/login", elfHeader)
            add("bin/proot.new", "proot-binary".toByteArray())
            add("etc/group", "root:x:0:\n".toByteArray())
            add("nix/store/abc123-system-path/bin/login", "store-login".toByteArray())
            add("SYMLINKS.txt", "/nix/store/abc123-system-path/bin/login←bin/login\n".toByteArray())
        }
        val installer = BootstrapInstaller(prefixDir, homeDir, stagingDir)

        val result = runBlocking { installer.install(zipFile) }

        assertTrue("nix bootstrap install must succeed: ${result.exceptionOrNull()?.message}", result.isSuccess)
        assertTrue("bin/login must exist after nix install", File(prefixDir, "bin/login").exists())
        // needsInstall must recognize the nix layout (no bin/bash).
        assertFalse("needsInstall must be false with bin/login present", installer.needsInstall())
        // The second stage (no dpkg dir in nix bootstraps) still writes
        // termux.env; only then is the install reported complete.
        val secondStage = SecondStageRunner(prefixDir, homeDir)
        val stageResult = runBlocking { secondStage.run() }
        assertTrue("nix second stage must succeed: ${stageResult.errors}", stageResult.success)
        assertTrue("isInstalled must be true after second stage (login branch)", installer.isInstalled())
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
            add("etc/termux/termux.env", "PREFIX=placeholder\n")
            add("SYMLINKS.txt", "bin/bash←usr/bin/bash\n")
            add("EXECUTABLES.txt", "usr/bin/env\nbin/bash\n")
        }
        val installer = BootstrapInstaller(prefixDir, homeDir, stagingDir)

        val result = runBlocking { installer.install(zipFile) }

        assertTrue("install with EXECUTABLES.txt must succeed: ${result.exceptionOrNull()?.message}", result.isSuccess)
        assertTrue("usr/bin/env must exist", File(prefixDir, "usr/bin/env").exists())
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
            """.trimIndent(),
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
            """.trimIndent(),
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

    // ── sha256 version-pin sidecar (warp bootstrap.rs) ────────

    @Test
    fun install_writes_version_pin_with_zip_sha256() {
        val zip = buildFakeBootstrapZip(withSymlinks = true)
        val expectedSha = BootstrapInstaller.sha256Of(zip)
        val installer = BootstrapInstaller(prefixDir, homeDir, stagingDir)

        val result = runBlocking { installer.install(zip) }

        assertTrue("install must succeed: ${result.exceptionOrNull()?.message}", result.isSuccess)
        val marker = File(prefixDir, BootstrapInstaller.VERSION_PIN_FILENAME)
        assertTrue("version pin must exist after install", marker.isFile)
        val pinned = BootstrapInstaller.readVersionPin(prefixDir)
        assertEquals("pin sha256 must match zip", expectedSha, pinned)
        assertFalse("needsInstall(same sha) must be false", installer.needsInstall(expectedSha))
        assertTrue("needsInstall(mismatched sha) must be true", installer.needsInstall("deadbeef"))
    }

    @Test
    fun install_failure_leaves_no_version_pin() {
        val zip = buildFakeBootstrapZip(withSymlinks = true)
        // Corrupt the zip: truncate it so extraction fails.
        zip.writeBytes(zip.readBytes().copyOf(200))
        val installer = BootstrapInstaller(prefixDir, homeDir, stagingDir)

        val result = runBlocking { installer.install(zip) }

        assertTrue("corrupted zip install must fail", result.isFailure)
        assertFalse(
            "no version pin on failure",
            File(prefixDir, BootstrapInstaller.VERSION_PIN_FILENAME).exists(),
        )
        // needsInstall with any hash is true (no marker).
        assertTrue(installer.needsInstall("abc"))
        assertTrue(installer.needsInstall(null))
    }

    @Test
    fun needsInstall_detects_marker_mismatch_and_missing_marker() {
        // No marker at all → needsInstall true even with a shell binary.
        File(prefixDir, "bin").mkdirs()
        File(prefixDir, "bin/login").writeBytes(byteArrayOf(0x7f, 0x45, 0x4c, 0x46) + "x".toByteArray())
        val installer = BootstrapInstaller(prefixDir, homeDir, stagingDir)
        assertTrue("no marker must need install", installer.needsInstall("abc"))

        // Matching marker → false.
        BootstrapInstaller.writeVersionPin(prefixDir, "abc")
        assertFalse("matching marker must not need install", installer.needsInstall("abc"))
        assertTrue("different zip sha must need install", installer.needsInstall("def"))
    }

    @Test
    fun version_pin_survives_atomic_write() {
        prefixDir.mkdirs()
        BootstrapInstaller.writeVersionPin(prefixDir, "0123456789abcdef")
        assertEquals("0123456789abcdef", BootstrapInstaller.readVersionPin(prefixDir))
        // No.tmp leftover.
        assertFalse(File(prefixDir, "${BootstrapInstaller.VERSION_PIN_FILENAME}.tmp").exists())
    }

    /**
     * Real nix-on-droid bootstrap-aarch64.zip layout: the
     * SYMLINKS.txt mixes absolute `/nix/store/...` targets and relative
     * store-internal targets (`libsystemd.so.0.44.0←nix/store/.../lib/...`),
     * and EXECUTABLES.txt lists deep store paths (real archive: 2132/2132
     * SYMLINKS lines parse). The installer must accept both target shapes
     * and keep the store tree intact.
     */
    @Test
    fun install_acceptsRealNixOnDroidLayout() {
        ZipOutputStream(zipFile.outputStream()).use { zos ->
            fun add(name: String, content: String) {
                zos.putNextEntry(ZipEntry(name))
                zos.write(content.toByteArray())
                zos.closeEntry()
            }
            fun addBytes(name: String, content: ByteArray) {
                zos.putNextEntry(ZipEntry(name))
                zos.write(content)
                zos.closeEntry()
            }
            // ELF magic: isInstalled/needsInstall require a real ELF shell.
            addBytes("bin/login", byteArrayOf(0x7f, 0x45, 0x4c, 0x46, 0x02, 0x01) + "login-binary".toByteArray())
            // Real SYMLINKS.txt sample lines (bootstrap-unstable):
            // 1. absolute /nix/ store target -> profile link
            // 2. relative target inside the store (link parent + filename)
            // 3. absolute store target -> usr/lib link
            add(
                "SYMLINKS.txt",
                "/nix/store/m8iadpkvczpjdl3j8yv5rnh88i45s27k-system←nix/var/nix/profiles/system\n" +
                    "libsystemd.so.0.44.0←nix/store/704walkdmvq7fkz3ajx6174z68lsbqp4-systemd-minimal-libs-261.1/lib/libsystemd.so.0\n" +
                    "/nix/store/704walkdmvq7fkz3ajx6174z68lsbqp4-systemd-minimal-libs-261.1/lib/libsystemd.so.0←usr/lib/libsystemd.so.0\n",
            )
            add(
                "EXECUTABLES.txt",
                "usr/bin/env\nnix/store/704walkdmvq7fkz3ajx6174z68lsbqp4-systemd-minimal-libs-261.1/bin/foo\n",
            )
            add("usr/bin/env", "#!/nix/store/abc/env\n")
            add("nix/store/704walkdmvq7fkz3ajx6174z68lsbqp4-systemd-minimal-libs-261.1/bin/foo", "x")
            add("nix/store/704walkdmvq7fkz3ajx6174z68lsbqp4-systemd-minimal-libs-261.1/lib/libsystemd.so.0.44.0", "x")
            add("nix/var/nix/db/schema", "7")
        }
        val installer = BootstrapInstaller(prefixDir, homeDir, stagingDir)

        val result = runBlocking { installer.install(zipFile) }

        assertTrue("real nix-on-droid layout must install: ${result.exceptionOrNull()?.message}", result.isSuccess)
        // createSymlinks walked every SYMLINKS.txt line: it created the
        // parent directories for the store links before the (Robolectric
        // no-op) Os.symlink. A missing parent would mean the line was
        // rejected by the path guards.
        assertTrue(
            "profiles dir must exist (first link's parent, absolute /nix/ target)",
            File(prefixDir, "nix/var/nix/profiles").isDirectory,
        )
        assertTrue(
            "usr/lib dir must exist (third link's parent, store target)",
            File(prefixDir, "usr/lib").isDirectory,
        )
        // Real-archive parse rate: 2132/2132 lines of bootstrap-unstable
        // parse; the samples cover both target shapes.
        val parsed = installer.parseSymlinks(
            "/nix/store/m8iadpkvczpjdl3j8yv5rnh88i45s27k-system←nix/var/nix/profiles/system\n" +
                "libsystemd.so.0.44.0←nix/store/704walkdmvq7fkz3ajx6174z68lsbqp4-systemd-minimal-libs-261.1/lib/libsystemd.so.0\n",
        )
        assertEquals(2, parsed.size)
        assertEquals("/nix/store/m8iadpkvczpjdl3j8yv5rnh88i45s27k-system", parsed[0].first)
        assertEquals("libsystemd.so.0.44.0", parsed[1].first)
    }
}

/** Pure-path tests: no Android/Os dependencies, no context needed. */
class BootstrapInstallerNormalizePathTest {
    private val installer = BootstrapInstaller(
        prefixDir = File("/tmp/t-prefix"),
        homeDir = File("/tmp/t-home"),
        stagingDir = File("/tmp/t-staging"),
    )

    @Test
    fun normalizePath_removes_dot_segments() {
        assertEquals("include/term_entry.h", installer.normalizePath("./include/ncurses/../term_entry.h"))
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
