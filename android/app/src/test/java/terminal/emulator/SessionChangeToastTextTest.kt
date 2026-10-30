package terminal.emulator

import org.junit.Assert.assertEquals
import org.junit.Test

/**
 * 会话切换提示的文案（`disable-terminal-session-change-toast` 关闭的那条）。
 *
 * 只测纯函数：Toast 本身要有真实窗口，仪器化环境里拿不到；文案是这条提示里唯一
 * 会被用户读到、也唯一可能被 shell 标题污染的部分。
 */
class SessionChangeToastTextTest {
    @Test
    fun `title follows the index label`() {
        assertEquals("[2] vim", sessionChangeToastText("[2]", "vim"))
    }

    @Test
    fun `blank title leaves only the index`() {
        assertEquals("[1]", sessionChangeToastText("[1]", ""))
        assertEquals("[1]", sessionChangeToastText("[1]", "   "))
    }

    /** shell 可以把标题设成多行；提示必须始终是一行，否则顶部会被撑开一大块。 */
    @Test
    fun `line breaks in a terminal title collapse to spaces`() {
        assertEquals("[3] a b c", sessionChangeToastText("[3]", "a\nb\r\nc"))
    }
}
