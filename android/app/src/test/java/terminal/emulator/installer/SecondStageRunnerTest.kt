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

/**
 * Unit tests for the SELinux linker workaround in SecondStageRunner:
 * postinst scripts and prefix ELFs must be exec'd via /system/bin/linker64
 * with LD_PRELOAD=libtermux-exec.so.
 */
@RunWith(RobolectricTestRunner::class)
@Config(sdk = [34])
class SecondStageRunnerTest {

    @Before
    fun setUp() {
        ShadowBuild.setSupportedAbis(arrayOf("arm64-v8a"))
    }

    private fun runnerWith(prefix: File, home: File = File("/tmp/home")) = SecondStageRunner(prefixDir = prefix, homeDir = home)

    private fun tempPrefix(): File {
        val dir = kotlin.io.path.createTempDirectory("prefix")
        return dir.toFile()
    }

    @Test
    fun postinst_command_uses_linker_for_prefix_shebang() {
        val prefix = tempPrefix()
        val prefixed = File(prefix, "bin/sh").apply {
            requireNotNull(parentFile).mkdirs()
            writeText("x")
        }
        val script = File(prefix, "var/lib/dpkg/info/coreutils.postinst")
        requireNotNull(script.parentFile).mkdirs()
        script.writeText("#!/${prefixed.absolutePath}\n")
        val cmd = runnerWith(prefix).postinstCommand(script)
        assertEquals("/system/bin/linker64", cmd[0])
        assertEquals(prefixed.absolutePath, cmd[1])
        assertEquals(script.absolutePath, cmd[2])
        assertEquals("configure", cmd[3])
    }

    @Test
    fun postinst_command_direct_for_system_shebang() {
        val prefix = tempPrefix()
        val script = File(prefix, "var/lib/dpkg/info/system-script.postinst")
        requireNotNull(script.parentFile).mkdirs()
        script.writeText("#!/bin/sh\n")
        val cmd = runnerWith(prefix).postinstCommand(script)
        // On Linux, /bin/sh resolves to /usr/bin/dash (canonical path).
        // The command[0] must be the canonical path of /bin/sh.
        val expectedInterpreter = File("/bin/sh").canonicalPath
        assertEquals(expectedInterpreter, cmd[0])
        assertEquals(script.absolutePath, cmd[1])
        assertEquals("configure", cmd[2])
    }

    @Test
    fun prefix_environment_includes_termux_exec_preload() {
        val prefix = tempPrefix()
        val env = runnerWith(prefix).prefixEnvironment()
        assertEquals(
            "${prefix.absolutePath}/lib/libtermux-exec.so",
            env["LD_PRELOAD"],
        )
        assertEquals(prefix.absolutePath, env["PREFIX"])
        assertTrue(requireNotNull(env["PATH"]).startsWith("${prefix.absolutePath}/bin"))
    }

    @Test
    fun dpkg_command_runs_via_linker() {
        val prefix = tempPrefix()
        val cmd = runnerWith(prefix).prefixExecutableCommand(
            File(prefix, "bin/dpkg"),
            listOf("--version"),
        )
        assertEquals("/system/bin/linker64", cmd[0])
        assertEquals("${prefix.absolutePath}/bin/dpkg", cmd[1])
        assertEquals("--version", cmd[2])
    }
}
