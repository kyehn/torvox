package terminal.emulator.runtime

import android.util.Log
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * 日志等级的放行判据与序号往返。
 *
 * 判别力：等级判据写反时，「关掉日志」关不掉（错误级仍在输出），
 * 或「只看错误」把警告也吞掉——两者都只在实际改等级时才暴露。
 */
class LogLevelTest {
    @Test
    fun `OFF 放行不了任何级别`() {
        assertFalse(LogLevel.OFF.allows(Log.ERROR))
        assertFalse(LogLevel.OFF.allows(Log.WARN))
        assertFalse(LogLevel.OFF.allows(Log.DEBUG))
    }

    @Test
    fun `ERROR 只放行错误及以上`() {
        assertTrue(LogLevel.ERROR.allows(Log.ERROR))
        assertFalse(LogLevel.ERROR.allows(Log.WARN))
        assertFalse(LogLevel.ERROR.allows(Log.INFO))
        assertFalse(LogLevel.ERROR.allows(Log.DEBUG))
    }

    @Test
    fun `WARN 放行警告与错误`() {
        assertTrue(LogLevel.WARN.allows(Log.ERROR))
        assertTrue(LogLevel.WARN.allows(Log.WARN))
        assertFalse(LogLevel.WARN.allows(Log.INFO))
    }

    @Test
    fun `DEBUG 放行调试及以上但不放行详细`() {
        assertTrue(LogLevel.DEBUG.allows(Log.DEBUG))
        assertFalse(LogLevel.DEBUG.allows(Log.VERBOSE))
    }

    @Test
    fun `VERBOSE 放行全部`() {
        assertTrue(LogLevel.VERBOSE.allows(Log.VERBOSE))
        assertTrue(LogLevel.VERBOSE.allows(Log.DEBUG))
    }

    @Test
    fun `序号往返不变`() {
        LogLevel.entries.forEach { level ->
            assertEquals(level, LogLevel.fromOrdinal(level.ordinal))
        }
    }

    @Test
    fun `越界序号回退到缺省等级`() {
        assertEquals(LogLevel.DEFAULT, LogLevel.fromOrdinal(-1))
        assertEquals(LogLevel.DEFAULT, LogLevel.fromOrdinal(LogLevel.entries.size))
    }

    @Test
    fun `Release 的可选等级不含调试与详细`() {
        // Release 构建已剥离调试级调用点（R8），列出它们只会让用户选中
        // 一个不生效的档位。
        assertFalse(LogLevel.releaseLevels.contains(LogLevel.DEBUG))
        assertFalse(LogLevel.releaseLevels.contains(LogLevel.VERBOSE))
        assertTrue(LogLevel.releaseLevels.contains(LogLevel.OFF))
        assertTrue(LogLevel.releaseLevels.contains(LogLevel.ERROR))
    }
}
