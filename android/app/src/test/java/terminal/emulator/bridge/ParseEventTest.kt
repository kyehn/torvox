package terminal.emulator.bridge

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * Private `Bridge.parseEvent` mapping contract, exercised via reflection
 * (same precedent as `NativeBridgeStaticExportsTest`): JSON event → frame result.
 *
 * Locks the exact shapes the poll loop depends on — especially the empty-clipboard
 * mapping (: `""` → null, i.e. "no clipboard event this frame") and the
 * nullable exit code (unknown must stay null, never 0).
 */
class ParseEventTest {

    // `parseEvent` 是 `Bridge` 的私有实例方法（Bridge 按会话持有配置）；
    // 反射调用它需要一个实例——构造只存配置，无 JNI 副作用。
    private val bridge = TestBridges.create()

    private fun parse(json: String): Bridge.PollResult {
        val method =
            Bridge::class.java.getDeclaredMethod("parseEvent", String::class.java).apply {
                isAccessible = true
            }
        return method.invoke(bridge, json) as Bridge.PollResult
    }

    @Test
    fun `clipboard text maps with session`() {
        val result = parse("""{"event":"clipboard","session_id":7,"text":"hello"}""")
        assertEquals("hello", result.clipboard)
        assertEquals(7L, result.sessionId)
    }

    @Test
    fun `empty clipboard maps to no event`() {
        val result = parse("""{"event":"clipboard","session_id":7,"text":""}""")
        assertNull(result.clipboard)
    }

    @Test
    fun `exit maps code and single entry`() {
        val result = parse("""{"event":"exit","session_id":7,"code":137}""")
        assertTrue(result.exit)
        assertEquals(137, result.exitCode)
        assertEquals(7L, result.sessionId)
        assertEquals(listOf(Bridge.ExitInfo(7L, 137)), result.exits)
    }

    @Test
    fun `unknown exit code stays null`() {
        val result = parse("""{"event":"exit","session_id":7,"code":null}""")
        assertTrue(result.exit)
        assertNull(result.exitCode)
    }

    @Test
    fun `clipboard read maps request fields`() {
        val result =
            parse("""{"event":"clipboard_read","session_id":7,"request_id":9,"selection":"c"}""")
        assertEquals(listOf(Bridge.ClipboardRequest(7L, 9L, "c")), result.clipboardReads)
    }
}
