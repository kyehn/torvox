package terminal.emulator.ui

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * [SurfaceAttachGate] 的挂载门闸不变量。
 *
 * 覆盖「Surface 销毁 → 恢复渲染 → 新窗口挂上」这条时序：门闸存在的唯一理由是
 * 让恢复路径**无权**解除「Surface 销毁」那次暂停——提前解除会让渲染线程在平台
 * 已废弃的 BufferQueue 上取帧（`ERROR_SURFACE_LOST_KHR`，返回后永久黑屏）。
 * 这条时序在设备上无法稳定构造，故在此用纯 JVM 单测跑完。
 */
class SurfaceAttachGateTest {

    /** 与生产同口径的记账：持有者非空时暂停，为空时恢复。 */
    private val pauseChanges = mutableListOf<Boolean>()
    private val ledger = RenderPauseLedger { paused -> pauseChanges += paused }
    private val gate = SurfaceAttachGate(ledger)

    @Test
    fun `destroy acquires a pause that only attach releases`() {
        gate.onSurfaceDestroyed()
        assertTrue(gate.isAwaitingAttach)
        assertEquals(1, ledger.holderCount)
        assertEquals(listOf(true), pauseChanges)

        gate.onSurfaceAttached()
        assertFalse(gate.isAwaitingAttach)
        assertEquals(0, ledger.holderCount)
        assertEquals(listOf(true, false), pauseChanges)
    }

    @Test
    fun `attach without a pending destroy is a no-op`() {
        gate.onSurfaceAttached()
        assertFalse(gate.isAwaitingAttach)
        assertEquals(0, ledger.holderCount)
        assertTrue(pauseChanges.isEmpty())
    }

    @Test
    fun `repeated attach does not release twice`() {
        gate.onSurfaceDestroyed()
        gate.onSurfaceAttached()
        gate.onSurfaceAttached()
        assertEquals(listOf(true, false), pauseChanges)
    }

    @Test
    fun `repeated destroy keeps exactly one pending pause`() {
        // 新窗口迟迟不来又销毁一次：先领新的再归还旧的，持有者数全程非空。
        // 反之会让持有者数经过 0，多出一对暂停/恢复。
        gate.onSurfaceDestroyed()
        gate.onSurfaceDestroyed()
        assertTrue(gate.isAwaitingAttach)
        assertEquals(1, ledger.holderCount)
        assertEquals(listOf(true), pauseChanges)

        gate.onSurfaceAttached()
        assertEquals(0, ledger.holderCount)
        assertEquals(listOf(true, false), pauseChanges)
    }
}
