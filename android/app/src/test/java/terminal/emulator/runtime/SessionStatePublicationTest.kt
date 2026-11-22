package terminal.emulator.runtime

import kotlinx.coroutines.flow.MutableStateFlow
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test
import java.util.concurrent.ConcurrentHashMap
import java.util.concurrent.CountDownLatch
import java.util.concurrent.atomic.AtomicBoolean
import java.util.concurrent.atomic.AtomicInteger
import java.util.concurrent.atomic.AtomicReference

/**
 * 运行期会话状态的发布契约：活动会话与会话列表 MUST 同源发布。
 *
 * 二者的增删都在会话总闸内完成。分两次锁外读会拼出「活动会话已关闭却仍留在
 * 列表里」的组合：界面把序号算成 0、抽屉高亮指向不存在的条目，且该状态不会自愈
 * （后续刷新只更新标题，不再改活动会话）。
 */
class SessionStatePublicationTest {

    @Test
    fun `snapshot is taken while holding the session lock`() {
        val state = MutableStateFlow(RuntimeState())
        val lock = Any()
        var held = false
        publishSessionState(state, lock) {
            held = Thread.holdsLock(lock)
            SessionStateSnapshot(isRunning = true, activeSessionId = 1L, sessionIds = listOf(1L))
        }
        assertTrue("快照必须在会话总闸内取出，否则活动会话与列表可能来自两个时刻", held)
    }

    @Test
    fun `active session changed clears the stale title`() {
        val state = MutableStateFlow(RuntimeState(activeSessionId = 1L, title = "旧标题"))
        publishSessionState(state, Any()) {
            SessionStateSnapshot(isRunning = true, activeSessionId = 2L, sessionIds = listOf(1L, 2L))
        }
        val published = state.value
        assertEquals(2L, published.activeSessionId)
        assertEquals("", published.title)
        assertEquals(listOf(1L, 2L), published.sessionIds)
        assertTrue(published.isRunning)
    }

    @Test
    fun `same active session keeps the title`() {
        val state = MutableStateFlow(RuntimeState(activeSessionId = 1L, title = "保留"))
        publishSessionState(state, Any()) {
            SessionStateSnapshot(isRunning = true, activeSessionId = 1L, sessionIds = listOf(1L))
        }
        assertEquals("保留", state.value.title)
    }

    @Test
    fun `no active session clears the title`() {
        val state = MutableStateFlow(RuntimeState(activeSessionId = 1L, title = "旧标题"))
        publishSessionState(state, Any()) {
            SessionStateSnapshot(isRunning = false, activeSessionId = 0L, sessionIds = emptyList())
        }
        val published = state.value
        assertEquals(0L, published.activeSessionId)
        assertEquals("", published.title)
        assertEquals(emptyList<Long>(), published.sessionIds)
        assertTrue(!published.isRunning)
    }

    /**
     * 并发改写会话集合时，发布出去的活动会话 MUST 始终是发布出去的列表的一员。
     *
     * 改写线程在两个自洽状态之间切换（活动会话 1 + [1,2] ↔ 活动会话 2 + [2]），
     * 快照若在锁外取，两次读取之间会跨过这次切换，拼出从未存在过的组合。
     */
    @Test
    fun `published active session always belongs to the published session list`() {
        val state = MutableStateFlow(RuntimeState())
        val lock = Any()
        val start = CountDownLatch(1)
        val sessionIds = ConcurrentHashMap.newKeySet<Long>()
        val active = AtomicReference(0L)
        val running = AtomicBoolean(true)
        val switches = AtomicInteger(0)
        val torn = AtomicReference<String?>(null)

        val mutator = Thread {
            start.await()
            while (running.get()) {
                synchronized(lock) {
                    if (active.get() == 1L) {
                        active.set(2L)
                        sessionIds.clear()
                        sessionIds.add(2L)
                    } else {
                        active.set(1L)
                        sessionIds.clear()
                        sessionIds.add(1L)
                        sessionIds.add(2L)
                    }
                    switches.incrementAndGet()
                }
                Thread.yield()
            }
        }

        val publisher = Thread {
            start.await()
            loop@ for (iteration in 0 until PUBLISH_ITERATIONS) {
                publishSessionState(state, lock) {
                    val activeId = active.get()
                    // 两次读取之间留一个让出点：快照不在锁内取时，
                    // 改写线程会在这里插进来，拼出不同源的组合。
                    Thread.yield()
                    val ids = sessionIds.toSortedSet().toList()
                    SessionStateSnapshot(
                        isRunning = ids.isNotEmpty(),
                        activeSessionId = activeId,
                        sessionIds = ids,
                    )
                }
                val published = state.value
                if (published.activeSessionId != 0L && published.activeSessionId !in published.sessionIds) {
                    torn.compareAndSet(
                        null,
                        "活动会话 ${published.activeSessionId} 不在 ${published.sessionIds} 中",
                    )
                    break@loop
                }
            }
        }

        mutator.start()
        publisher.start()
        start.countDown()
        publisher.join(JOIN_TIMEOUT_MS)
        running.set(false)
        mutator.join(JOIN_TIMEOUT_MS)

        assertTrue("发布线程必须在超时前跑完", !publisher.isAlive)
        assertTrue("改写线程必须真的切换过，否则本用例是空跑", switches.get() > 0)
        assertNull("发布的状态不得把活动会话与列表拼成两个时刻", torn.get())
    }

    private companion object {
        const val PUBLISH_ITERATIONS = 2000
        const val JOIN_TIMEOUT_MS = 30_000L
    }
}
