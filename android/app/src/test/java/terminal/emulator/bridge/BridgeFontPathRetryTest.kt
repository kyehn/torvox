package terminal.emulator.bridge

import org.junit.Assert.assertEquals
import org.junit.Test
import java.util.concurrent.atomic.AtomicReference

class BridgeFontPathRetryTest {
    @Suppress("UNCHECKED_CAST")
    private fun appliedPaths(bridge: Bridge): List<String> {
        val field = Bridge::class.java.getDeclaredField("lastExtraFontPaths")
        field.isAccessible = true
        return (field.get(bridge) as AtomicReference<List<String>>).get()
    }

    @Test
    fun `an unavailable session does not mark font paths as applied`() {
        val bridge = TestBridges.create()

        bridge.setExtraFontPaths(listOf("/home/.termux/fonts"))

        assertEquals("会话未建立时 JNI 被跳过，去重缓存不得前进", emptyList<String>(), appliedPaths(bridge))
    }
}
