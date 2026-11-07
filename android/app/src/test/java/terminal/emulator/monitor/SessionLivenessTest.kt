package terminal.emulator.monitor

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * 挂起层级判定、恢复决策与关闭节流都是纯函数：输入是判定瞬间的快照，不读时钟、
 * 不读全局状态，故每种情形都能由单测直接构造。
 */
class SessionLivenessTest {

    private companion object {
        const val ONE_SECOND_NANOS = 1_000_000_000L
    }

    /** 全部忙标记皆空闲、渲染线程存活的基线输入。 */
    private fun healthyInputs() = SessionLivenessInputs(
        renderFrameBusyNanos = 0L,
        vtBusyNanos = 0L,
        ptySendBusyNanos = 0L,
        renderThreadAlive = true,
    )

    @Test
    fun `idle session is not diagnosed as hung`() {
        val diagnosis = diagnoseSessionLiveness(healthyInputs())
        assertNull("空闲会话不得被判成挂起", diagnosis.hungLayer)
        assertEquals(0L, diagnosis.busyNanos)
    }

    @Test
    fun `render frame in flight past threshold is a render hang`() {
        val inputs = healthyInputs().copy(renderFrameBusyNanos = 11 * ONE_SECOND_NANOS)
        val diagnosis = diagnoseSessionLiveness(inputs)
        assertEquals(HungLayer.RENDER, diagnosis.hungLayer)
        assertEquals(11 * ONE_SECOND_NANOS, diagnosis.busyNanos)
    }

    @Test
    fun `render frame in flight below threshold is not a hang`() {
        val inputs = healthyInputs().copy(renderFrameBusyNanos = 9 * ONE_SECOND_NANOS)
        assertNull(diagnoseSessionLiveness(inputs).hungLayer)
    }

    @Test
    fun `vt thread busy past threshold is a vt hang`() {
        val inputs = healthyInputs().copy(vtBusyNanos = 16 * ONE_SECOND_NANOS)
        val diagnosis = diagnoseSessionLiveness(inputs)
        assertEquals(HungLayer.VT, diagnosis.hungLayer)
        assertEquals(16 * ONE_SECOND_NANOS, diagnosis.busyNanos)
    }

    @Test
    fun `vt thread busy below threshold is not a hang`() {
        val inputs = healthyInputs().copy(vtBusyNanos = 14 * ONE_SECOND_NANOS)
        assertNull(diagnoseSessionLiveness(inputs).hungLayer)
    }

    @Test
    fun `blocked output delivery past threshold is an output backlog`() {
        val inputs = healthyInputs().copy(ptySendBusyNanos = 21 * ONE_SECOND_NANOS)
        val diagnosis = diagnoseSessionLiveness(inputs)
        assertEquals(HungLayer.OUTPUT_BACKLOG, diagnosis.hungLayer)
        assertEquals(21 * ONE_SECOND_NANOS, diagnosis.busyNanos)
    }

    @Test
    fun `blocked output delivery below threshold is not a hang`() {
        val inputs = healthyInputs().copy(ptySendBusyNanos = 19 * ONE_SECOND_NANOS)
        assertNull(diagnoseSessionLiveness(inputs).hungLayer)
    }

    @Test
    fun `dead render thread with running session is a lifecycle hang`() {
        val inputs = healthyInputs().copy(renderThreadAlive = false)
        assertEquals(HungLayer.LIFECYCLE, diagnoseSessionLiveness(inputs).hungLayer)
    }

    /**
     * 层级顺序即优先级：渲染帧在途时其余层的读数多半只是它的下游后果，
     * 命中即止，不得越级报成 VT 层。
     */
    @Test
    fun `render hang wins over vt and output backlog`() {
        val inputs =
            healthyInputs().copy(
                renderFrameBusyNanos = 30 * ONE_SECOND_NANOS,
                vtBusyNanos = 30 * ONE_SECOND_NANOS,
                ptySendBusyNanos = 30 * ONE_SECOND_NANOS,
            )
        assertEquals(HungLayer.RENDER, diagnoseSessionLiveness(inputs).hungLayer)
    }

    @Test
    fun `vt hang wins over output backlog`() {
        val inputs =
            healthyInputs().copy(
                vtBusyNanos = 30 * ONE_SECOND_NANOS,
                ptySendBusyNanos = 30 * ONE_SECOND_NANOS,
            )
        assertEquals(HungLayer.VT, diagnoseSessionLiveness(inputs).hungLayer)
    }

    @Test
    fun `recovery action restarts the render thread for a render hang`() {
        assertEquals(
            LivenessRecovery.RESTART_RENDER_THREAD,
            recoveryActionFor(HungLayer.RENDER),
        )
    }

    @Test
    fun `recovery action closes the session for vt and output backlog`() {
        assertEquals(LivenessRecovery.CLOSE_SESSION, recoveryActionFor(HungLayer.VT))
        assertEquals(LivenessRecovery.CLOSE_SESSION, recoveryActionFor(HungLayer.OUTPUT_BACKLOG))
    }

    @Test
    fun `recovery action only records for lifecycle hang and for no hang`() {
        assertEquals(LivenessRecovery.RECORD_ONLY, recoveryActionFor(HungLayer.LIFECYCLE))
        assertEquals(LivenessRecovery.RECORD_ONLY, recoveryActionFor(null))
    }

    @Test
    fun `never closed session may be closed immediately`() {
        assertTrue(shouldCloseSession(0L, 1_000L))
    }

    @Test
    fun `recently closed session is throttled`() {
        val now = 100 * ONE_SECOND_NANOS
        assertFalse(
            "60s 内的再次关闭必须被节流",
            shouldCloseSession(now - 30 * ONE_SECOND_NANOS, now),
        )
    }

    @Test
    fun `session may be closed again after the throttle window`() {
        val now = 100 * ONE_SECOND_NANOS
        assertTrue(shouldCloseSession(now - SESSION_CLOSE_THROTTLE_NANOS, now))
    }
}
