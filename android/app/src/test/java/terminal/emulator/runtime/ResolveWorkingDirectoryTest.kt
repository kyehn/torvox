package terminal.emulator.runtime

import org.junit.Assert.assertEquals
import org.junit.Test

/**
 * `default-working-directory` 的解析（[resolveWorkingDirectory]），语义对齐 termux：
 * 空值 = 家目录，`~` 展开为家目录，相对路径相对家目录，绝对路径原样。
 *
 * 不做存在性校验是有意的：目录不存在时原样交给 PTY 层报错，用户才能看出自己写错了
 * 哪个字符；静默改回家目录只会表现为「设置没生效」。
 */
class ResolveWorkingDirectoryTest {

    private val home = "/data/user/0/com.termux/files/home"

    @Test
    fun `empty value falls back to the home directory`() {
        assertEquals(home, resolveWorkingDirectory("", home))
        assertEquals(home, resolveWorkingDirectory("   ", home))
    }

    @Test
    fun `tilde expands to home`() {
        assertEquals(home, resolveWorkingDirectory("~", home))
        assertEquals("$home/src", resolveWorkingDirectory("~/src", home))
        assertEquals("$home/src", resolveWorkingDirectory("~/src/", home))
    }

    @Test
    fun `relative paths are relative to home`() {
        assertEquals("$home/src", resolveWorkingDirectory("src", home))
        assertEquals("$home/a/b", resolveWorkingDirectory("a/b", home))
    }

    @Test
    fun `absolute paths are kept verbatim`() {
        assertEquals("/sdcard/downloads", resolveWorkingDirectory("/sdcard/downloads", home))
        assertEquals("/", resolveWorkingDirectory("/", home))
    }

    /**
     * 用户在家目录外写的 `~user` 不展开。
     *
     * 展开别人的家目录需要 passwd 查询，而结果还可能是错的；原样交出去由 shell 报错，
     * 比猜一个路径诚实。
     */
    @Test
    fun `tilde followed by a user name is not expanded`() {
        assertEquals("$home/~other", resolveWorkingDirectory("~other", home))
    }

    /** 前后空白来自配置文件里的手写缩进，不该变成目录名的一部分。 */
    @Test
    fun `surrounding whitespace is trimmed`() {
        assertEquals("/sdcard", resolveWorkingDirectory("  /sdcard  ", home))
    }
}
