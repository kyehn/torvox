package terminal.emulator

import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * table-driven tests for [abbreviateDirectory] — the session drawer second line
 * (spec 侧边面板 会话列表 “会话序号 目录路径”: `file://` prefix stripped,
 * home folded to `~`, overlong paths ellipsized in the middle).
 */
class AbbreviateDirectoryTest {

    private val home = "/data/data/com.termux/files/home"

    @Test
    fun stripsFileScheme() {
        assertEquals("/home/user", abbreviateDirectory("file:///home/user", home))
    }

    @Test
    fun foldsHomeToTilde() {
        assertEquals("~", abbreviateDirectory("file://$home", home))
        assertEquals("~/bin", abbreviateDirectory("$home/bin", home))
    }

    @Test
    fun leavesUnrelatedPathUntouched() {
        assertEquals("/system/bin", abbreviateDirectory("/system/bin", home))
    }

    @Test
    fun ellipsizesOverlongPathInMiddle() {
        val long = "/data/data/com.termux/files/usr/" + "a".repeat(60)
        val abbreviated = abbreviateDirectory(long, home)
        assertEquals(MAX_SESSION_DIRECTORY_LENGTH, abbreviated.length)
        assertTrue(abbreviated.contains("…"))
    }

    @Test
    fun shortPathKeepsFullText() {
        assertEquals("~/a", abbreviateDirectory("$home/a", home))
    }
}
