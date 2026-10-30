package terminal.emulator.runtime

import org.junit.Assert.assertEquals
import org.junit.Test
import java.io.File

/**
 * `default-working-directory` 的解析（[resolveWorkingDirectory]），语义对齐 termux：
 * 空值 = 家目录，`~` 展开为家目录，相对路径相对家目录，绝对路径原样。
 *
 * 目录必须真实存在才生效：写错的路径会让 PTY 失败、会话起不来，配置里的一个笔误
 * 不该让应用「打不开终端」，故不存在的目录回退家目录（与 termux 一致）。
 */
class ResolveWorkingDirectoryTest {

    private val home = kotlin.io.path.createTempDirectory("home").toFile()

    private fun directory(name: String): File = File(home, name).apply { mkdirs() }

    private fun file(name: String): File = File(home, name).apply { writeText("x") }

    @Test
    fun `empty value falls back to the home directory`() {
        assertEquals(home.path, resolveWorkingDirectory("", home.path))
        assertEquals(home.path, resolveWorkingDirectory("   ", home.path))
    }

    @Test
    fun `tilde expands to home`() {
        assertEquals(home.path, resolveWorkingDirectory("~", home.path))
        val source = directory("src")
        assertEquals(source.path, resolveWorkingDirectory("~/src", home.path))
        assertEquals(source.path, resolveWorkingDirectory("~/src/", home.path))
    }

    @Test
    fun `relative paths are relative to home`() {
        val source = directory("src")
        assertEquals(source.path, resolveWorkingDirectory("src", home.path))
        val nested = directory("a/b")
        assertEquals(nested.path, resolveWorkingDirectory("a/b", home.path))
    }

    @Test
    fun `absolute paths are kept verbatim`() {
        val absolute = kotlin.io.path.createTempDirectory("absolute").toFile()
        assertEquals(absolute.path, resolveWorkingDirectory(absolute.path, home.path))
        assertEquals("/", resolveWorkingDirectory("/", home.path))
    }

    /**
     * 不存在的目录回退家目录。
     *
     * 不回退的话，PTY 会带着这个路径去 chdir 并失败：会话起不来，用户看到的是「应用
     * 坏了」，而不是「配置里的路径写错了」。
     */
    @Test
    fun `a missing directory falls back to home`() {
        assertEquals(home.path, resolveWorkingDirectory("/sdcard/does-not-exist", home.path))
        assertEquals(home.path, resolveWorkingDirectory("missing", home.path))
    }

    /** 指向文件而非目录同样无效。 */
    @Test
    fun `a path that is a file falls back to home`() {
        val regularFile = file("notes.txt")
        assertEquals(home.path, resolveWorkingDirectory(regularFile.path, home.path))
    }

    /**
     * 用户在家目录外写的 `~user` 不展开。
     *
     * 展开别人的家目录需要 passwd 查询，而结果还可能是错的；它又不是本进程的家目录，
     * 不存在便回退家目录。
     */
    @Test
    fun `tilde followed by a user name is not expanded`() {
        assertEquals(home.path, resolveWorkingDirectory("~other", home.path))
    }

    /** 前后空白来自配置文件里的手写缩进，不该变成目录名的一部分。 */
    @Test
    fun `surrounding whitespace is trimmed`() {
        val absolute = kotlin.io.path.createTempDirectory("absolute").toFile()
        assertEquals(absolute.path, resolveWorkingDirectory("  ${absolute.path}  ", home.path))
    }
}
