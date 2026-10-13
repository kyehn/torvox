package terminal.emulator.ui

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertSame
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * 旧视图的 `surfaceDestroyed` 晚于新视图的 `surfaceCreated` 到达时的归属判定。
 *
 * 这条时序在设备上无法稳定构造，而它的后果是终端全黑且无自愈（渲染线程无错误、无
 * 重启），故此处用纯 JVM 单测把整条时序跑完。
 */
class SurfaceOwnershipTest {
    private val oldView = Any()
    private val newView = Any()

    @Test
    fun `stale view destroy after new claim is ignored`() {
        val ownership = SurfaceOwnership()
        ownership.claim(oldView)
        ownership.claim(newView)
        assertFalse(
            "旧视图的迟到销毁 MUST NOT 归还归属",
            ownership.release(oldView),
        )
        assertSame(
            "归属仍属新视图：全局暂停标志不能被旧视图按停",
            newView,
            ownership.currentOwner,
        )
        assertTrue(ownership.release(newView))
    }

    @Test
    fun `destroy before any claim is ignored`() {
        val ownership = SurfaceOwnership()
        assertFalse(ownership.release(oldView))
        assertNull(ownership.currentOwner)
    }

    @Test
    fun `same view can re-claim after its own destroy`() {
        val ownership = SurfaceOwnership()
        ownership.claim(oldView)
        assertTrue(ownership.release(oldView))
        // 视图复用（分屏/折叠/Compose 重挂载）后重新拿到 Surface，仍是自己人。
        ownership.claim(oldView)
        assertTrue(ownership.release(oldView))
    }

    @Test
    fun `a second release by the same view is stale`() {
        val ownership = SurfaceOwnership()
        ownership.claim(oldView)
        assertTrue(ownership.release(oldView))
        assertFalse("重复归还 MUST NOT 误伤下一任持有者", ownership.release(oldView))
    }

    @Test
    fun `claim replaces the previous owner`() {
        val ownership = SurfaceOwnership()
        ownership.claim(oldView)
        ownership.claim(newView)
        assertEquals(newView, ownership.currentOwner)
        assertFalse(ownership.release(oldView))
        assertTrue(ownership.release(newView))
        assertNull(ownership.currentOwner)
    }
}
