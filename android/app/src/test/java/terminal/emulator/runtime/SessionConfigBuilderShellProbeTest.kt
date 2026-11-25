package terminal.emulator.runtime

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test
import java.io.File
import java.nio.file.Files

/**
 * 默认入口探测与登录 shell 语义的直接覆盖。
 *
 * 探测顺序取 DESIGN「Shell」节：`{login, bash, zsh, fish, sh}`，文件存在即启动、
 * 不检查权限；登录语义由 shell 自身的 `-l` 开关取得，唯 `bin/login` 例外——
 * 它是上游 termux-tools 的脚本，自己以 `exec "$SHELL" -l "$@"` 启动 shell，
 * 多带参数会吞掉 motd。
 */
class SessionConfigBuilderShellProbeTest {
    private fun prefixWith(vararg candidates: String): String {
        val prefix = Files.createTempDirectory("shell-probe-prefix").toFile()
        prefix.deleteOnExit()
        File(prefix, "bin").mkdirs()
        for (candidate in candidates) {
            File(prefix, candidate).writeText("#!/system/bin/sh\n")
            File(prefix, candidate).deleteOnExit()
        }
        return prefix.absolutePath
    }

    @Test
    fun `login wins over every other candidate`() {
        val prefix = prefixWith("bin/login", "bin/bash", "bin/zsh", "bin/fish", "bin/sh")
        assertEquals("bin/login", SessionConfigBuilder.defaultShellEntry(prefix))
    }

    @Test
    fun `probe falls through candidates in design order`() {
        assertEquals(
            "bin/bash",
            SessionConfigBuilder.defaultShellEntry(prefixWith("bin/bash", "bin/zsh", "bin/fish", "bin/sh")),
        )
        assertEquals(
            "bin/zsh",
            SessionConfigBuilder.defaultShellEntry(prefixWith("bin/zsh", "bin/fish", "bin/sh")),
        )
        assertEquals("bin/fish", SessionConfigBuilder.defaultShellEntry(prefixWith("bin/fish", "bin/sh")))
        assertEquals("bin/sh", SessionConfigBuilder.defaultShellEntry(prefixWith("bin/sh")))
    }

    @Test
    fun `probe ignores permissions and file kind`() {
        val prefix = prefixWith("bin/bash")
        val bash = File(prefix, "bin/bash")
        bash.setExecutable(false, false)
        assertEquals("bin/bash", SessionConfigBuilder.defaultShellEntry(prefix))
        // 目录不是可执行文件，即使同名也不得被选中。
        File(prefix, "bin/zsh").mkdirs()
        assertEquals("bin/bash", SessionConfigBuilder.defaultShellEntry(prefix))
    }

    @Test
    fun `probe returns null when no candidate exists`() {
        assertNull(SessionConfigBuilder.defaultShellEntry(prefixWith()))
    }

    @Test
    fun `login argument keeps the login script argument free`() {
        assertEquals("", SessionConfigBuilder.loginArgument("bin/login"))
    }

    @Test
    fun `login argument asks shell binaries for login semantics`() {
        assertEquals(" -l", SessionConfigBuilder.loginArgument("bin/bash"))
        assertEquals(" -l", SessionConfigBuilder.loginArgument("bin/zsh"))
        assertEquals(" -l", SessionConfigBuilder.loginArgument("bin/fish"))
        assertEquals(" -l", SessionConfigBuilder.loginArgument("bin/sh"))
    }
}
