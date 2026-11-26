package terminal.emulator.runtime

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertSame
import org.junit.Assert.assertTrue
import org.junit.Test
import terminal.emulator.bridge.TestBridges

/**
 * 会话表的契约：活动会话与列表同源、id 升序、移除不残留。
 */
class SessionRegistryTest {

    private fun entry(sessionId: Long) = SessionEntry(
        sessionId = sessionId,
        bridge = TestBridges.create(),
        renderThreadRef = null,
        running = true,
    )

    @Test
    fun `会话 id 按升序发布`() {
        val registry = SessionRegistry()
        registry.register(entry(7L))
        registry.register(entry(2L))
        registry.register(entry(5L))

        assertEquals(listOf(2L, 5L, 7L), registry.ids())
    }

    @Test
    fun `活动会话条目随活动 id 改变`() {
        val registry = SessionRegistry()
        val first = entry(1L)
        val second = entry(2L)
        registry.register(first)
        registry.register(second)

        registry.setActive(2L)
        assertEquals(2L, registry.activeSessionId)
        assertSame(second, registry.activeEntry())

        registry.setActive(1L)
        assertSame(first, registry.activeEntry())
    }

    @Test
    fun `移除会话后活动条目随之消失`() {
        val registry = SessionRegistry()
        val removed = entry(3L)
        registry.register(removed)
        registry.setActive(3L)

        assertSame(removed, registry.remove(3L))

        assertFalse(registry.contains(3L))
        assertNull(registry.activeEntry())
        assertEquals(0, registry.size)
        assertTrue(registry.isEmpty())
    }

    @Test
    fun `按 bridge 移除只清掉挂在该 bridge 上的会话`() {
        val registry = SessionRegistry()
        val bridge = TestBridges.create()
        val attached =
            SessionEntry(
                sessionId = 1L,
                bridge = bridge,
                renderThreadRef = null,
                running = true,
            )
        registry.register(attached)
        registry.register(entry(2L))

        assertEquals(1, registry.removeByBridge(bridge))

        assertFalse(registry.contains(1L))
        assertTrue(registry.contains(2L))
    }

    @Test
    fun `闸内持有会话总闸`() {
        val registry = SessionRegistry()
        var held = false

        registry.withLock { held = Thread.holdsLock(registry.lock) }

        assertTrue("块必须在会话总闸内执行，否则并发改动会互相穿插", held)
    }

    @Test
    fun `移除活动会话后接替者取剩余会话中 id 最大者`() {
        val registry = SessionRegistry()
        registry.register(entry(2L))
        registry.register(entry(9L))
        registry.register(entry(5L))
        registry.setActive(9L)

        val replacementId = registry.removeAndChooseReplacement(9L)

        assertEquals(5L, replacementId)
        assertFalse(registry.contains(9L))
    }

    @Test
    fun `移除最后一个会话时没有接替者`() {
        val registry = SessionRegistry()
        registry.register(entry(4L))
        registry.setActive(4L)

        val replacementId = registry.removeAndChooseReplacement(4L)

        assertEquals(0L, replacementId)
        assertTrue(registry.isEmpty())
    }

    @Test
    fun `移除非活动会话不产生接替者`() {
        val registry = SessionRegistry()
        registry.register(entry(1L))
        registry.register(entry(2L))
        registry.setActive(1L)

        val replacementId = registry.removeAndChooseReplacement(2L)

        assertNull("被移除的不是活动会话，活动会话保持不变", replacementId)
        assertEquals(1L, registry.activeSessionId)
    }
}
