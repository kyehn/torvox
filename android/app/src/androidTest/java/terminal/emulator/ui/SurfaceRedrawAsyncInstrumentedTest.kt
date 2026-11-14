package terminal.emulator.ui

import androidx.test.platform.app.InstrumentationRegistry
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith
import org.junit.runners.JUnit4
import terminal.emulator.ComposeTerminalLogcatTest
import terminal.emulator.findTerminalSurface
import java.util.concurrent.atomic.AtomicBoolean

/**
 * `SurfaceHolder.Callback2` 的重绘完成回调必须异步执行。
 *
 * Android 13 的 `SurfaceView.handleSyncNoBuffer` 在把 syncId 记入 `mSyncIds` 之前调用
 * 重绘回调；同步完成会让该 id 在入账前就被移除，之后每次表面同步都去查这个陈旧 id，
 * 打出 `SurfaceSyncer: Failed to find sync for id=N`。本用例锁定「完成回调投递到下一轮
 * UI 循环」这一前提：同步执行即回归。
 */
@RunWith(JUnit4::class)
class SurfaceRedrawAsyncInstrumentedTest : ComposeTerminalLogcatTest() {
    @Test
    fun redrawCompletionRunsAfterTheCurrentUiLoop() {
        composeTestRule.waitForIdle()
        val surface = findTerminalSurface(composeTestRule.activity) as TerminalSurface
        val completed = AtomicBoolean(false)
        var ranSynchronously = false
        InstrumentationRegistry.getInstrumentation().runOnMainSync {
            surface.surfaceRedrawNeededAsync(surface.holder) { completed.set(true) }
            ranSynchronously = completed.get()
        }
        assertFalse("完成回调不得在 surfaceRedrawNeededAsync 内同步执行", ranSynchronously)
        InstrumentationRegistry.getInstrumentation().waitForIdleSync()
        assertTrue("完成回调必须在下一轮 UI 循环执行", completed.get())
    }
}
