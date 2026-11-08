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
import terminal.emulator.bridge.TestBridges
import terminal.emulator.monitor.HungLayer
import terminal.emulator.monitor.RenderWatchDog
import terminal.emulator.monitor.SessionLivenessDiagnosis
import terminal.emulator.settings.SettingsRepository
import terminal.emulator.settings.TermuxPropertiesRepository

/**
 * 挂起判定的处置：只有会话**当前**看门狗的判定才生效。
 *
 * 渲染线程重启会换上新的看门狗实例；旧实例的迟到判定若被采纳，会把新渲染线程标记为
 * 死亡（渲染层），或关闭一个已经恢复的会话（VT/输出投递层）。
 *
 * 判别力：陈旧看门狗的处置若不返回，渲染层用例里 `renderThreadExited` 会变真，
 * 关闭层用例里自动关闭许可会被领取（`lastAutoCloseNanos` 变非零）。
 */
@RunWith(RobolectricTestRunner::class)
class SessionHangVerdictTest {

    private val runtime: TerminalRuntime by lazy {
        TerminalRuntime(
            context = ApplicationProvider.getApplicationContext<Context>(),
            settingsRepository = mockk<SettingsRepository>(relaxed = true),
            termuxProperties = mockk<TermuxPropertiesRepository>(relaxed = true),
        )
    }

    /** 一个从未启动的看门狗：只作为身份标识参与复核。 */
    private fun watchdog(): RenderWatchDog =
        RenderWatchDog(
            diagnose = { SessionLivenessDiagnosis(hungLayer = null, busyNanos = 0L) },
            isRunning = { false },
            onHangDetected = { _, _ -> },
        )

    /** 一个待处置的会话条目。 */
    private fun entry(): SessionEntry =
        SessionEntry(
            sessionId = 1L,
            bridge = TestBridges.create(),
            renderThreadRef = null,
            running = true,
        )

    /** 某层挂起的诊断。 */
    private fun diagnosis(layer: HungLayer): SessionLivenessDiagnosis =
        SessionLivenessDiagnosis(hungLayer = layer, busyNanos = 30_000_000_000L)

    @Test
    fun `the current watchdog marks the render thread for restart`() {
        val sessionEntry = entry()
        val current = watchdog()
        sessionEntry.renderWatchDog = current

        runtime.renderSupervisor.handleSessionHang(sessionEntry, current, diagnosis(HungLayer.RENDER))

        assertTrue("当前看门狗的渲染层判定必须标记线程死亡", sessionEntry.renderThreadExited)
    }

    @Test
    fun `a stale watchdog verdict must not mark the render thread dead`() {
        val sessionEntry = entry()
        val current = watchdog()
        val stale = watchdog()
        sessionEntry.renderWatchDog = current

        runtime.renderSupervisor.handleSessionHang(sessionEntry, stale, diagnosis(HungLayer.RENDER))

        assertFalse("陈旧看门狗的判定不得标记线程死亡", sessionEntry.renderThreadExited)
    }

    @Test
    fun `a stale watchdog verdict must not claim the auto-close permit`() {
        val sessionEntry = entry()
        val current = watchdog()
        val stale = watchdog()
        sessionEntry.renderWatchDog = current

        runtime.renderSupervisor.handleSessionHang(sessionEntry, stale, diagnosis(HungLayer.VT))

        assertEquals("陈旧看门狗的判定不得领取自动关闭许可", 0L, sessionEntry.lastAutoCloseNanos)
    }
}
