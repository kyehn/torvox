package terminal.emulator.installer

import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config
import org.robolectric.shadows.ShadowBuild
import java.io.File
import java.io.PipedInputStream
import java.io.PipedOutputStream

/**
 * Unit tests for the SELinux linker workaround in SecondStageRunner: postinst scripts and prefix
 * ELFs must be exec'd via /system/bin/linker64; the process environment follows the spec whitelist.
 */
@RunWith(RobolectricTestRunner::class)
@Config(sdk = [34])
class SecondStageRunnerTest {

    @Before
    fun setUp() {
        ShadowBuild.setSupportedAbis(arrayOf("arm64-v8a"))
    }

    private fun runnerWith(
        prefix: File,
        home: File = File(
            "/tmp/home",
        ),
    ) = SecondStageRunner(prefixDir = prefix, homeDir = home)

    private fun tempPrefix(): File {
        val prefixDir = kotlin.io.path.createTempDirectory("prefix")
        return prefixDir.toFile()
    }

    @Test
    fun postinst_command_uses_linker_for_prefix_shebang() {
        val prefix = tempPrefix()
        val prefixed =
            File(prefix, "bin/sh").apply {
                requireNotNull(parentFile).mkdirs()
                writeText("x")
            }
        val script = File(prefix, "var/lib/dpkg/info/coreutils.postinst")
        requireNotNull(script.parentFile).mkdirs()
        script.writeText("#!/${prefixed.absolutePath}\n")
        val command = runnerWith(prefix).postinstCommand(script)
        assertEquals("/system/bin/linker64", command[0])
        assertEquals(prefixed.absolutePath, command[1])
        assertEquals(script.absolutePath, command[2])
        assertEquals("configure", command[3])
    }

    @Test
    fun postinst_command_direct_for_system_shebang() {
        val prefix = tempPrefix()
        val script = File(prefix, "var/lib/dpkg/info/system-script.postinst")
        requireNotNull(script.parentFile).mkdirs()
        script.writeText("#!/bin/sh\n")
        val command = runnerWith(prefix).postinstCommand(script)
        // On Linux, /bin/sh is a symlink: command[0] must be its canonical path.
        val expectedInterpreter = File("/bin/sh").canonicalPath
        assertEquals(expectedInterpreter, command[0])
        assertEquals(script.absolutePath, command[1])
        assertEquals("configure", command[2])
    }

    @Test
    fun prefix_environment_matches_spec_whitelist() {
        val prefix = tempPrefix()
        val environment = runnerWith(prefix).prefixEnvironment()
        assertEquals(prefix.absolutePath, environment["PREFIX"])
        assertEquals(prefix.absolutePath, environment["TERMUX_PREFIX_DIR_PATH"])
        assertEquals(SecondStageRunner.TERMUX_VERSION, environment["TERMUX_VERSION"])
        assertEquals("en_US.UTF-8", environment["LANG"])
        for (key in listOf("PATH", "SHELL", "LD_PRELOAD", "LD_LIBRARY_PATH", "PWD")) {
            assertTrue("$key must not be set", !environment.containsKey(key))
        }
    }

    @Test
    fun drainQuietly_reads_content() {
        assertEquals("hello", drainQuietly("hello".byteInputStream()))
    }

    @Test
    fun drainQuietly_returns_null_on_closed_pipe() {
        val writer = PipedOutputStream()
        val reader = PipedInputStream(writer)
        reader.close()
        writer.close()
        assertEquals(null, drainQuietly(reader))
    }

    @Test
    fun drainQuietly_bounds_kept_bytes_but_still_reads_to_eof() {
        // 读必须走到 EOF（否则子进程卡在写满的管道上），但保留量必须有界：
        // 崩溃的脚本循环刷 stderr 时，无界累积会把进程读成 OOM。
        val total = 4 * 1024 * 1024
        var consumed = 0
        val stream =
            object : java.io.InputStream() {
                override fun read(): Int = if (consumed++ < total) 0 else -1

                override fun read(buffer: ByteArray, offset: Int, length: Int): Int {
                    if (consumed >= total) return -1
                    val count = minOf(length, total - consumed)
                    consumed += count
                    return count
                }
            }
        val text = drainQuietly(stream)
        assertEquals(DRAIN_KEEP_BYTES, text!!.length)
        assertEquals(total, consumed)
    }

    @Test
    fun drain_daemon_survives_raced_close() {
        // Device-proven shape (SecondStageRunner FATAL on a dpkg-less
        // prefix): destroy() closes the pipe while a daemon drain is
        // blocked in read. Both interleavings (blocked-then-closed and
        // read-after-close) throw inside read and must die quietly —
        // never an uncaught throw off the raw thread.
        val writer = PipedOutputStream()
        val reader = PipedInputStream(writer)
        var uncaught: Throwable? = null
        val daemon = Thread { drainQuietly(reader) }
        daemon.uncaughtExceptionHandler = Thread.UncaughtExceptionHandler { _, error ->
            uncaught = error
        }
        daemon.isDaemon = true
        daemon.start()
        reader.close()
        daemon.join(5_000)
        assertEquals(null, uncaught)
    }

    @Test
    fun dpkg_command_runs_via_linker() {
        val prefix = tempPrefix()
        val command =
            runnerWith(prefix)
                .prefixExecutableCommand(
                    File(prefix, "bin/dpkg"),
                    listOf("--version"),
                )
        assertEquals("/system/bin/linker64", command[0])
        assertEquals("${prefix.absolutePath}/bin/dpkg", command[1])
        assertEquals("--version", command[2])
    }
}
