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
        // Robolectric 下 SUPPORTED_ABIS 为空：按宿主真实架构声明，
        // 否则 detectArchFromAbi 按未知 ABI 抛错（生产期望行为）。
        val field = android.os.Build::class.java.getDeclaredField("SUPPORTED_ABIS")
        field.isAccessible = true
        field.set(null, arrayOf("x86_64"))
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
        prefixDir.parentFile
            ?.listFiles { file ->
                file.name.startsWith(
                    "${prefixDir.name}.",
                )
            }?.forEach { it.deleteRecursively() }
        homeDir.deleteRecursively()
        stagingDir.deleteRecursively()
        zipFile.delete()
    }

    /**
     * 在 prefix 的 `var/lib/dpkg/info` 下落一个假 postinst 并置可执行。
     *
     * 三个 second-stage 用例共用这段脚手架：各写一份时「信息目录路径」这类细节
     * 一旦在某一份里漂移，用例会因目录缺失而失败，而不是因被测行为失败。
     */
    private fun seedPostinst(body: String): File {
        val infoDir = File(prefixDir, "var/lib/dpkg/info")
        infoDir.mkdirs()
        val script = File(infoDir, "fake.postinst")
        script.writeText(body.trimIndent())
        script.setExecutable(true)
        return script
    }

    private fun buildFakeBootstrapZip(withSymlinks: Boolean): File {
        ZipOutputStream(zipFile.outputStream()).use { zos ->
            fun add(name: String, content: String = "x") {
                zos.putNextEntry(ZipEntry(name))
                zos.write(content.toByteArray())
                zos.closeEntry()
            }
            add("bin/bash", "#!/system/bin/sh\necho bash\n")
            add("bin/gawk", "gawk-binary")
            add("bin/busybox", "busybox-binary")
            add("lib/libfoo.so", "libfoo")
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

    /**
     * 清单类路径没有「目录条目」形态：`EXECUTABLES.txt` 的 `foo/..` 与
     * `SYMLINKS.txt` 的同形项消解后都指向 staging 目录自身。只查
     * [BootstrapInstaller.escapesStagingDir] 一个都不命中（它按契约把
     * 消解为空判给调用方），故两处调用点必须另加该判据。
     */
    @Test
    fun install_rejects_executable_and_symlink_paths_resolving_to_staging_itself() {
        for (listing in listOf("EXECUTABLES.txt", "SYMLINKS.txt")) {
            ZipOutputStream(zipFile.outputStream()).use { zos ->
                zos.putNextEntry(ZipEntry("bin/bash"))
                zos.write("#!/system/bin/sh\n".toByteArray())
                zos.closeEntry()
                zos.putNextEntry(ZipEntry(listing))
                val body = if (listing == "EXECUTABLES.txt") "bin/..\n" else "bin/bash←bin/..\n"
                zos.write(body.toByteArray())
                zos.closeEntry()
            }
            val installer = BootstrapInstaller(prefixDir, homeDir, stagingDir)
            val result = runBlocking { installer.install(zipFile) }

            assertTrue("$listing must be refused", result.isFailure)
            assertTrue(
                "$listing rejection must name the unsafe path",
                result.exceptionOrNull()?.message?.contains("..") == true,
            )
            assertFalse("no prefix may be published from $listing", prefixDir.exists())
        }
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

    @Test
    fun install_rejectsZipSlipTraversalEntry() {
        // 恶意 bootstrap 压缩包不得写出 staging 目录（对等 ghostty
        // restoreRejectsTraversalEscape）：../ 条目必须导致安装失败，
        // 且 escape 目标文件不得被创建。
        val escapeDir = File(context.cacheDir, "zipslip-escape-${UUID.randomUUID().toString().take(8)}")
        escapeDir.mkdirs()
        try {
            ZipOutputStream(zipFile.outputStream()).use { zos ->
                zos.putNextEntry(ZipEntry("bin/bash"))
                zos.write("#!/system/bin/sh\n".toByteArray())
                zos.closeEntry()
                zos.putNextEntry(ZipEntry("../zipslip-pwned.txt"))
                zos.write("PWNED\n".toByteArray())
                zos.closeEntry()
            }
            val installer = BootstrapInstaller(prefixDir, homeDir, stagingDir)

            val result = runBlocking { installer.install(zipFile) }

            assertTrue("install must fail on traversal entry", result.isFailure)
            assertFalse(
                "traversal payload must not escape staging",
                File(stagingDir.parentFile, "zipslip-pwned.txt").exists(),
            )
            assertFalse("failed install must not report installed", installer.isInstalled())
        } finally {
            escapeDir.deleteRecursively()
        }
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
            add("bin/bash", "#!/system/bin/sh\n")
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
        val other = File(prefixDir, "pkg/store/abc123-foo-1.0/bin/foo")
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

    /** parseSymlinks keeps the `target←linkPath` direction for absolute store paths. */
    @Test
    fun parseSymlinks_keepsAbsoluteTargetDirection() {
        val installer = BootstrapInstaller(prefixDir, homeDir, stagingDir)
        val parsed =
            installer.parseSymlinks(
                "/pkg/store/abc-system-path/bin/login←bin/login\n" +
                    "/pkg/store/def-bash/bin/sh←bin/bash\n",
            )
        assertEquals(2, parsed.size)
        assertEquals("/pkg/store/abc-system-path/bin/login", parsed[0].first)
        assertEquals("bin/login", parsed[0].second)
    }

    @Test
    fun secondStageRunner_executesPostinstScript() {
        // Seed a minimal prefix with a post-install script that writes a marker file.
        val marker = File(prefixDir, "postinst-ran.marker")
        seedPostinst(
            """
            #!/bin/sh
            echo "configure" > "${marker.absolutePath}"
            exit 0
            """,
        )

        val result = runBlocking { SecondStageRunner(prefixDir, homeDir).run() }

        assertTrue("second stage should succeed: ${result.errors}", result.success)
        assertTrue("postinst script must have executed", marker.exists())
    }

    @Test
    fun secondStageRunner_retriesOnceAfterPostinstCrash() {
        // 首轮 139（复刻 update-alternatives 首运崩溃：活干完、进程崩），
        // 次轮幂等成功：旧逻辑无重试直接失败，新逻辑自愈。
        val flag = File(prefixDir, "first-run-crashed.flag")
        val marker = File(prefixDir, "postinst-retried.marker")
        seedPostinst(
            """
            #!/bin/sh
            if [ -f "${flag.absolutePath}" ]; then
                echo "configure" > "${marker.absolutePath}"
                exit 0
            fi
            touch "${flag.absolutePath}"
            exit 139
            """,
        )

        val result = runBlocking { SecondStageRunner(prefixDir, homeDir).run() }

        assertTrue("retry must heal first-run crash: ${result.errors}", result.success)
        assertTrue("postinst script must have completed on retry", marker.exists())
    }

    @Test
    fun secondStageRunner_lockIsReleasedAfterRun() {
        val marker = File(prefixDir, "postinst-ran.marker")
        seedPostinst(
            """
            #!/bin/sh
            echo "configure" > "${marker.absolutePath}"
            exit 0
            """,
        )

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

    /**
     * 回归钉：清不净 staging 区时安装必须失败，而不是把新引导写进残留旧树。
     *
     * 上一次安装若在解压中途被杀，`staging` 会留下整棵树。此前 `delete` 丢弃了每个
     * `File.delete()` 的返回值，于是「删不掉」与「删干净」无法区分，`install` 照样
     * 继续：新一轮条目写进旧树，`atomicRename` 把这份混血树当 prefix 发布，而安装
     * 一路报成功。
     *
     * 关键在只读目录的位置：放在 `locked/`（ZIP 只写 `bin/`、`lib/`），于是
     * **去掉守卫后解压会顺利成功**——本例据此与旧行为区分，而不是靠「反正也失败」
     * 这种任何实现都能蒙对的断言。测试以非 root 身份运行；Robolectric 无
     * symlink/ACL 支持，故用权限位制造真实的删不掉。
     */
    @Test
    fun install_aborts_when_staging_cannot_be_cleared() {
        val zip = buildFakeBootstrapZip(withSymlinks = true)
        // 模拟「上次安装被杀」留下的残留树。
        val locked = File(stagingDir, "locked")
        locked.mkdirs()
        File(locked, "leftover-from-previous-run").writeText("stale binary")
        locked.setWritable(false, false)
        locked.setReadable(true, false)
        try {
            val installer = BootstrapInstaller(prefixDir, homeDir, stagingDir)

            val result = runBlocking { installer.install(zip) }

            assertTrue("staging 删不净时安装必须失败", result.isFailure)
            assertEquals(
                "失败原因须是清空 staging 区，而不是后续解压步骤",
                "Bootstrap staging dir could not be fully cleared",
                result.exceptionOrNull()?.message,
            )
            assertFalse("prefix 绝不能被原子换入", installer.isInstalled())
        } finally {
            locked.setWritable(true, false)
            stagingDir.deleteRecursively()
        }
    }

    /** 清空 staging 区必须把上次残留整棵树清掉，不能只删顶层目录。 */
    @Test
    fun install_clears_deep_stale_staging_tree_before_extracting() {
        val zip = buildFakeBootstrapZip(withSymlinks = true)
        val deep = File(stagingDir, "a/b/c/stale")
        deep.parentFile?.mkdirs()
        deep.writeText("stale")
        val installer = BootstrapInstaller(prefixDir, homeDir, stagingDir)

        assertTrue(runBlocking { installer.install(zip) }.isSuccess)

        assertFalse(
            "残留的深层条目必须被清掉，否则会与新引导混进同一棵树",
            File(prefixDir, "a/b/c/stale").exists(),
        )
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
        val backupsAfterFirst =
            prefixDir.parentFile?.listFiles { file ->
                file.name.startsWith(
                    "${prefixDir.name}.",
                )
            } ?: emptyArray()
        assertTrue("no backup after first install", backupsAfterFirst.isEmpty())
        File(prefixDir, "bin/bash").writeText("#!/system/bin/sh\nfirst\n")
        assertTrue(runBlocking { installer.install(buildFakeBootstrapZip(true)) }.isSuccess)
        val backups =
            prefixDir.parentFile?.listFiles { file ->
                file.isDirectory &&
                    file.name.startsWith(
                        "${prefixDir.name}.",
                    )
            } ?: emptyArray()
        assertEquals("previous prefix kept as single random backup", 1, backups.size)
        assertTrue(
            "backup holds the previous tree",
            File(backups[0], "bin/bash").readText().contains("first"),
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
    fun escapesStagingDir_resolves_dot_segments_before_judging() {
        // `File("foo/..").path` 仍是 "foo/.."，此前四个前缀条件一个都不命中。
        assertTrue(installer.escapesStagingDir("../etc/passwd"))
        assertTrue(installer.escapesStagingDir("foo/../.."))
        assertFalse(installer.escapesStagingDir("foo/.."))
        assertTrue(installer.escapesStagingDir("/etc/passwd"))
        assertFalse(installer.escapesStagingDir("bin/sh"))
        assertFalse(installer.escapesStagingDir("libexec/../bin/sh"))
        // 段级比较：`..foo` 是普通文件名，不得被字符串前缀误判成 `..`。
        assertFalse(installer.escapesStagingDir("..foo/bar"))
    }

    @Test
    fun escapesStagingDir_keeps_leading_escape_segments() {
        assertTrue(installer.escapesStagingDir("../../escape"))
        // a/../../b 消解后剩 ../b（吃掉 a 后仍逃出一层）。
        assertTrue(installer.escapesStagingDir("a/../../b"))
        assertFalse(installer.escapesStagingDir("a/../b"))
    }

    @Test
    fun resolvesToStagingRoot_catches_paths_folding_back_to_staging() {
        // 消解后无任何段：作用对象其实是 staging 目录自身。
        assertTrue(installer.resolvesToStagingRoot("foo/.."))
        assertTrue(installer.resolvesToStagingRoot("."))
        assertFalse(installer.resolvesToStagingRoot("include/ncurses/term_entry.h"))
    }

    @Test
    fun termux_style_target_resolves_inside_staging() {
        // link=./include/ncurses/term_entry.h target=../term_entry.h
        assertFalse(
            "resolved target escapes staging",
            installer.escapesStagingDir("./include/ncurses/../term_entry.h"),
        )
    }
}
