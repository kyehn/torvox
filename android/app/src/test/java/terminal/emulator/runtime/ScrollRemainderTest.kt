package terminal.emulator.runtime

import android.content.Context
import androidx.test.core.app.ApplicationProvider
import io.mockk.mockk
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import terminal.emulator.TransientMessageChannel
import terminal.emulator.bridge.TestBridges
import terminal.emulator.settings.SettingsRepository
import terminal.emulator.settings.TermuxPropertiesRepository

/**
 * 亚行滚动余量的记账口径（[TerminalRuntime.setScrollRemainderPx] 与
 * [TerminalRuntime.clearScrollRemainder]）。
 *
 * 两个写入口的语义不同：
 * - 手势写入刷新滚动运动时钟，使渲染循环在手势期间保持活跃节奏；
 * - 清零**不得**刷新它：清零发生在手势结束、会话切换与终端重置等非手势路径上，
 *   刷新会让随后一段时间里的新输出被误判为「用户刚滚动过」而不再自动贴底
 *   （见 `shouldResetScroll` 的 `recentlyScrolled`）。
 *
 * 判别力：把清零改回「经手势写入口写 0」时，「清零不刷新时钟」一条即判红。
 */
@RunWith(RobolectricTestRunner::class)
class ScrollRemainderTest {

    private val runtime: TerminalRuntime by lazy {
        TerminalRuntime(
            context = ApplicationProvider.getApplicationContext<Context>(),
            settingsRepository = mockk<SettingsRepository>(relaxed = true),
            termuxProperties = mockk<TermuxPropertiesRepository>(relaxed = true),
            transientMessages = TransientMessageChannel(),
        )
    }

    /**
     * 挂一个活动会话并返回它的条目。
     *
     * 运行期的活动会话 id 初值为 0（尚未建立会话时的哨兵），故把条目挂在 id 0 上，
     * 两个写入口即命中它——真实路径由 `switchSession` 发布活动会话，那条路要原生库。
     */
    private fun activeSession(): SessionEntry {
        val entry =
            SessionEntry(
                sessionId = 0L,
                bridge = TestBridges.create(),
                renderThreadRef = null,
                running = true,
            )
        runtime.sessionRegistry.register(entry)
        return entry
    }

    @Test
    fun `手势写入余量时刷新滚动运动时钟`() {
        val session = activeSession()

        runtime.setScrollRemainderPx(4.5f)

        assertEquals("余量必须落库", 4.5f, session.scrollRemainderPx, 0f)
        assertTrue("手势必须刷新滚动运动时钟", session.hasScrollMotion())
    }

    @Test
    fun `清零余量不得刷新滚动运动时钟`() {
        val session = activeSession()
        runtime.setScrollRemainderPx(4.5f)
        val stampedAt = session.lastScrollNanos

        runtime.clearScrollRemainder()

        assertEquals("余量必须归零", 0f, session.scrollRemainderPx, 0f)
        assertEquals("清零不是手势，滚动运动时钟不得被刷新", stampedAt, session.lastScrollNanos)
    }

    @Test
    fun `新输出贴底时亚行余量与整行偏移同拍清零`() {
        val session = activeSession()
        session.scrollOffset = 9
        runtime.setScrollRemainderPx(17f)
        val stampedAt = session.lastScrollNanos

        session.resetScrollToBottom()

        assertEquals("整行偏移必须回底", 0, session.scrollOffset)
        assertEquals("亚行余量必须同拍清掉，否则画面先冲过底边再弹回", 0f, session.scrollRemainderPx, 0f)
        assertEquals("贴底不是手势，滚动运动时钟不得被刷新", stampedAt, session.lastScrollNanos)
    }

    @Test
    fun `仅有亚行余量时贴底也必须清零`() {
        val session = activeSession()
        session.scrollOffset = 0
        runtime.setScrollRemainderPx(17f)
        val stampedAt = session.lastScrollNanos

        session.resetScrollToBottom()

        assertEquals("整行偏移应保持在底部", 0, session.scrollOffset)
        assertEquals("仅有亚行余量时也必须同拍清掉", 0f, session.scrollRemainderPx, 0f)
        assertEquals("贴底不是手势，滚动运动时钟不得被刷新", stampedAt, session.lastScrollNanos)
    }

    @Test
    fun `从未滚动过的会话清零后仍无滚动运动`() {
        val session = activeSession()

        runtime.clearScrollRemainder()

        assertFalse("清零不得凭空造出滚动运动", session.hasScrollMotion())
    }
}
