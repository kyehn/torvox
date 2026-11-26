package terminal.emulator.runtime

import org.junit.After
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

/** 调试开关的短路口径：等级不放行调试级时 [LogUtil.isDebugEnabled] 必须为 false。 */
class LogUtilLevelTest {
    @After
    fun restoreDefaultLevel() {
        LogUtil.setLevel(LogLevel.DEFAULT)
    }

    @Test
    fun debugFlagFollowsLevel() {
        LogUtil.setLevel(LogLevel.ERROR)
        assertFalse("ERROR 级不得放行调试消息的构造", LogUtil.isDebugEnabled)

        LogUtil.setLevel(LogLevel.DEBUG)
        assertTrue("DEBUG 级必须放行调试消息", LogUtil.isDebugEnabled)
    }
}
