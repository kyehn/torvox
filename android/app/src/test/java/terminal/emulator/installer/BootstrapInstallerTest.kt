package terminal.emulator.installer

import androidx.test.core.app.ApplicationProvider
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.Job
import kotlinx.coroutines.launch
import kotlinx.coroutines.runBlocking
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Assert.fail
import org.junit.Before
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import java.io.ByteArrayInputStream
import java.io.ByteArrayOutputStream
import java.io.File
import java.nio.file.Files
import java.util.UUID
import java.util.zip.ZipEntry
import java.util.zip.ZipInputStream
import java.util.zip.ZipOutputStream

/** 真实 [BootstrapInstaller] 路径：构造合成归档并装入临时 prefix。 */
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
        // 否则 detectDeviceArchitecture 按未知 ABI 抛错（生产期望行为）。
        val field = android.os.Build::class.java.getDeclaredField("SUPPORTED_ABIS")
        field.isAccessible = true
        field.set(null, arrayOf("x86_64"))
        val runId = UUID.randomUUID().toString().take(8)
        prefixDir = File(context.filesDir, "bstest-$runId/usr")
        homeDir = File(context.filesDir, "bstest-$runId/home")
        stagingDir = File(context.cacheDir, "bstest-$runId-staging")
        zipFile = File(context.cacheDir, "bstest-$runId.zip")
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

    private fun buildFakeBootstrapZip(withSymlinks: Boolean): File {
        ZipOutputStream(zipFile.outputStream()).use { zipOutputStream ->
            fun add(name: String, content: String = "x") {
                zipOutputStream.putNextEntry(ZipEntry(name))
                zipOutputStream.write(content.toByteArray())
                zipOutputStream.closeEntry()
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
                zipOutputStream.putNextEntry(ZipEntry("SYMLINKS.txt"))
                zipOutputStream.write(content.toByteArray())
                zipOutputStream.closeEntry()
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
            ZipOutputStream(zipFile.outputStream()).use { zipOutputStream ->
                zipOutputStream.putNextEntry(ZipEntry("bin/bash"))
                zipOutputStream.write("#!/system/bin/sh\n".toByteArray())
                zipOutputStream.closeEntry()
                zipOutputStream.putNextEntry(ZipEntry(listing))
                val body = if (listing == "EXECUTABLES.txt") "bin/..\n" else "bin/bash←bin/..\n"
                zipOutputStream.write(body.toByteArray())
                zipOutputStream.closeEntry()
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
    fun install_cancellation_clears_partial_staging_dir() {
        // 进度每 10 项才回报一次；造 12 项才能在 staging 尚未换入时触发取消。
        ZipOutputStream(zipFile.outputStream()).use { zip ->
            repeat(12) { index ->
                zip.putNextEntry(ZipEntry("bin/file$index"))
                zip.write("x".toByteArray())
                zip.closeEntry()
            }
            zip.putNextEntry(ZipEntry("SYMLINKS.txt"))
            zip.write("x←bin/file0".toByteArray())
            zip.closeEntry()
        }
        lateinit var installJob: Job
        var cancelled = false
        val installer =
            BootstrapInstaller(
                prefixDir,
                homeDir,
                stagingDir,
                onProgress = { progress ->
                    if (progress is BootstrapProgress.Extracting && progress.entriesExtracted >= 2) {
                        installJob.cancel()
                    }
                },
            )

        runBlocking {
            installJob =
                launch {
                    try {
                        installer.install(zipFile)
                    } catch (expected: CancellationException) {
                        cancelled = true
                    }
                }
            installJob.join()
        }

        assertTrue("install must observe cancellation", cancelled)
        assertFalse("partially extracted staging dir must be removed", stagingDir.exists())
        assertFalse("cancelled install must not publish a prefix", prefixDir.exists())
    }

    @Test
    fun cancellation_inside_a_large_zip_entry_stops_before_writing_its_payload() {
        val archive =
            ByteArrayOutputStream().use { sink ->
                ZipOutputStream(sink).use { zip ->
                    zip.putNextEntry(ZipEntry("bin/blob"))
                    zip.write(ByteArray(BootstrapInstaller.COPY_BUFFER_SIZE * 4) { 1 })
                    zip.closeEntry()
                }
                sink.toByteArray()
            }
        var checks = 0
        try {
            BootstrapInstaller(prefixDir, homeDir, stagingDir)
                .processZipEntries(
                    ZipInputStream(ByteArrayInputStream(archive)),
                    mutableListOf(),
                    mutableListOf(),
                    shouldAbort = { ++checks >= 2 },
                    onEntryProcessed = {},
                )
            fail("大条目内的取消必须立即中断")
        } catch (_: CancellationException) {
            // 预期路径。
        }
        assertEquals("取消时不得写入单条目的 payload", 0L, File(stagingDir, "bin/blob").length())
    }

    @Test
    fun install_extractsFilesAndCreatesSymlinksWithCorrectDirection() {
        val zipFile = buildFakeBootstrapZip(withSymlinks = true)
        val installer = BootstrapInstaller(prefixDir, homeDir, stagingDir)

        val result = runBlocking { installer.install(zipFile) }

        assertTrue("install should succeed: ${result.exceptionOrNull()?.message}", result.isSuccess)
        assertTrue("bin/bash must exist after install", File(prefixDir, "bin/bash").exists())
        assertTrue("lib/libfoo.so must exist after install", File(prefixDir, "lib/libfoo.so").exists())

        // Staging must have been atomically renamed away.
        assertFalse("staging dir must be gone after atomic rename", stagingDir.exists())
        assertTrue("home must exist after install", homeDir.isDirectory)
        assertTrue("usr/tmp must be published with the prefix", File(prefixDir, "tmp").isDirectory)

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
        val zipFile = buildFakeBootstrapZip(withSymlinks = false)
        val installer = BootstrapInstaller(prefixDir, homeDir, stagingDir)

        val result = runBlocking { installer.install(zipFile) }

        assertTrue("install must fail when SYMLINKS.txt is absent", result.isFailure)
        assertFalse("prefix must not be reported installed on failure", installer.isInstalled())
        // 缺清单曾以 Result.failure 直接返回，跳过失败清理：staging 区留在原地，
        // 而 install 见 staging 已存在即拒绝——一次坏包就把引导永久堵死。
        assertFalse("staging must be cleaned on failure", stagingDir.exists())
        val retry = runBlocking { installer.install(buildFakeBootstrapZip(withSymlinks = true)) }
        assertTrue(
            "retry after a missing SYMLINKS.txt must succeed: ${retry.exceptionOrNull()?.message}",
            retry.isSuccess,
        )
    }

    @Test
    fun install_failsBeforePublishingPrefixWhenHomeCannotBeCreated() {
        homeDir.parentFile?.mkdirs()
        homeDir.writeText("not a directory")
        val installer = BootstrapInstaller(prefixDir, homeDir, stagingDir)

        val result = runBlocking { installer.install(buildFakeBootstrapZip(withSymlinks = true)) }

        assertTrue("install must fail when home cannot be created", result.isFailure)
        assertFalse("failed install must not publish a prefix", prefixDir.exists())
        assertFalse("failed install must clean staging", stagingDir.exists())
    }

    @Test
    fun install_rejectsZipSlipTraversalEntry() {
        // 恶意 bootstrap 压缩包不得写出 staging 目录（对等 ghostty
        // restoreRejectsTraversalEscape）：../ 条目必须导致安装失败，
        // 且 escape 目标文件不得被创建。
        val escapeDir = File(context.cacheDir, "zipslip-escape-${UUID.randomUUID().toString().take(8)}")
        escapeDir.mkdirs()
        try {
            ZipOutputStream(zipFile.outputStream()).use { zipOutputStream ->
                zipOutputStream.putNextEntry(ZipEntry("bin/bash"))
                zipOutputStream.write("#!/system/bin/sh\n".toByteArray())
                zipOutputStream.closeEntry()
                zipOutputStream.putNextEntry(ZipEntry("../zipslip-pwned.txt"))
                zipOutputStream.write("PWNED\n".toByteArray())
                zipOutputStream.closeEntry()
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
        ZipOutputStream(zipFile.outputStream()).use { zipOutputStream ->
            fun add(name: String, content: String = "x") {
                zipOutputStream.putNextEntry(ZipEntry(name))
                zipOutputStream.write(content.toByteArray())
                zipOutputStream.closeEntry()
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
    fun install_failure_cleans_staging_and_reports_not_installed() {
        val zipFile = buildFakeBootstrapZip(withSymlinks = true)
        // Corrupt the zip: truncate it so extraction fails.
        zipFile.writeBytes(zipFile.readBytes().copyOf(200))
        val installer = BootstrapInstaller(prefixDir, homeDir, stagingDir)

        val result = runBlocking { installer.install(zipFile) }

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
     * 「删不掉」由注入的清理器制造：靠权限位在 root 或特殊挂载下不成立，用例只能
     * 被静默跳过，等于没测。去掉守卫时，解压会顺利写进残留树、`atomicRename` 把
     * 「旧残留 + 新条目」发布成 prefix，而安装报成功——本例据此与旧行为区分。
     */
    @Test
    fun install_refuses_when_staging_already_exists() {
        val zipFile = buildFakeBootstrapZip(withSymlinks = true)
        // 模拟「上次安装被杀」留下的残留树。
        val locked = File(stagingDir, "locked")
        locked.mkdirs()
        File(locked, "leftover-from-previous-run").writeText("stale binary")

        val installer =
            BootstrapInstaller(prefixDir, homeDir, stagingDir, clearStaging = { false })

        val result = runBlocking { installer.install(zipFile) }

        assertTrue("staging 已存在时安装必须失败", result.isFailure)
        assertEquals(
            "失败原因须是 staging 区已存在，而不是后续解压步骤",
            "Bootstrap staging dir already exists",
            result.exceptionOrNull()?.message,
        )
        assertFalse("prefix 绝不能被原子换入", installer.isInstalled())
        assertTrue(
            "拒绝安装时不得删除用户的残留树",
            File(stagingDir, "locked/leftover-from-previous-run").exists(),
        )
    }

    /**
     * staging 区已存在即拒绝执行（DESIGN Bootstrap 节）：残留树来源不明，
     * 清空它会删掉用户还没来得及排查的东西，复用它则会把新条目写进旧树。
     */
    @Test
    fun install_refuses_instead_of_clearing_a_deep_stale_staging_tree() {
        val zipFile = buildFakeBootstrapZip(withSymlinks = true)
        val deep = File(stagingDir, "a/b/c/stale")
        deep.parentFile?.mkdirs()
        deep.writeText("stale")
        val installer = BootstrapInstaller(prefixDir, homeDir, stagingDir)

        val result = runBlocking { installer.install(zipFile) }

        assertTrue("staging 已存在时安装必须失败", result.isFailure)
        assertTrue(
            "残留的深层条目不得被清掉——拒绝执行，而不是替用户清理",
            deep.exists(),
        )
        assertFalse(
            "残留条目绝不能混进新 prefix",
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
    /**
     * 沙箱根判据要看「引导包是否提供了那个顶层目录」，故给一个真实的临时目录
     * （用例在其中造出 `nix/` 来模拟 Nix 引导包的顶层目录）。
     */
    private val stagingDir: File = Files.createTempDirectory("t-staging").toFile()

    private val installer =
        BootstrapInstaller(
            prefixDir = File("/tmp/t-prefix"),
            homeDir = File("/tmp/t-home"),
            stagingDir = stagingDir,
        )

    @After
    fun cleanup() {
        stagingDir.deleteRecursively()
    }

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

    @Test
    fun absolute_target_under_files_usr_is_rewritten_to_current_prefix() {
        // 官方包的 SYMLINKS.txt 用旧式绝对路径；二级用户上本机 prefix 在
        // /data/user/<id>/<pkg>/files/usr，照抄建链得到的是断链。
        assertEquals(
            "/data/user/10/com.termux/files/usr/bin/gawk",
            installer.resolveAbsoluteSymlinkTarget(
                "/data/data/com.termux/files/usr/bin/gawk",
                "/data/user/10/com.termux/files/usr",
            ),
        )
    }

    @Test
    fun absolute_target_without_files_usr_segment_is_kept_as_is() {
        // 不含该分段的绝对路径不由本函数改写：接受与否由规范路径校验决定。
        assertEquals(
            "/opt/other/bin/gawk",
            installer.resolveAbsoluteSymlinkTarget(
                "/opt/other/bin/gawk",
                "/data/user/10/com.termux/files/usr",
            ),
        )
    }

    @Test
    fun sandbox_root_absolute_target_is_re_rooted_into_prefix() {
        // 引导包的 SYMLINKS.txt 用沙箱根绝对路径：引导环境把 prefix 的顶层目录
        // 绑成沙箱里的同名路径（见引导包的 bin/login），故沙箱里的路径指的是
        // prefix 下的同名路径。
        // 判别力：原样照抄会指向 prefix 之外，规范路径校验直接判安装失败——
        // 那正是 Nix 变种装不上引导包的原因。
        File(stagingDir, "sandbox").mkdirs()
        assertEquals(
            "/data/user/10/com.termux/files/usr/sandbox/store/abc-bash/bin/sh",
            installer.resolveAbsoluteSymlinkTarget(
                "/sandbox/store/abc-bash/bin/sh",
                "/data/user/10/com.termux/files/usr",
            ),
        )
    }

    @Test
    fun sandbox_root_target_without_that_directory_is_kept_as_is() {
        // 引导包没有提供那个顶层目录时不得改写：改写会造出断链，
        // 而原样保留还能被规范路径校验挡下（安装失败，而不是装出坏链接）。
        assertEquals(
            "/etc/hosts",
            installer.resolveAbsoluteSymlinkTarget(
                "/etc/hosts",
                "/data/user/10/com.termux/files/usr",
            ),
        )
    }

    @Test
    fun absolute_target_keeps_dot_segments_for_the_escape_check() {
        // 重写不得消解 `..`：消解后逃逸校验就看不出越界了。
        assertEquals(
            "/data/user/10/com.termux/files/usr/../../etc/passwd",
            installer.resolveAbsoluteSymlinkTarget(
                "/data/data/com.termux/files/usr/../../etc/passwd",
                "/data/user/10/com.termux/files/usr",
            ),
        )
    }
}
