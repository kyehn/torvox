package terminal.emulator.ui

import androidx.compose.runtime.Composable
import androidx.compose.runtime.MutableState
import androidx.compose.runtime.mutableStateOf
import androidx.compose.ui.test.junit4.v2.createComposeRule
import kotlinx.coroutines.awaitCancellation
import org.junit.Assert.assertEquals
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config
import org.robolectric.annotation.GraphicsMode

/**
 * [SettingsRenderPauseEffect] 的凭证归属不变量。
 *
 * 渲染暂停只有一个计数账本：任何一次「领了没人还」都会让计数停在 1，共享渲染器被
 * 永久暂停，终端从此不再出帧（黑屏）。本测试盯住三条路径：
 *
 * - 设置页在本 effect 运行期间关掉：`onDispose` 早已跑过且没拿到凭证，此时 MUST
 *   NOT 再领一次——领了就没有人归还。
 * - 设置页正常关闭：领到的那一次 MUST 被归还，账本计数归零。
 * - 等待渲染器就绪期间被取消：一次都不得领。
 */
@RunWith(RobolectricTestRunner::class)
@GraphicsMode(GraphicsMode.Mode.NATIVE)
@Config(sdk = [34])
class SettingsRenderPauseEffectRobolectricTest {
    @get:Rule val composeRule = createComposeRule()

    private val ledger = RenderPauseLedger { _ -> }
    private var acquireCount = 0
    private var releaseCount = 0

    private fun acquire(): Long {
        acquireCount++
        return ledger.acquire()
    }

    private fun release(token: Long) {
        releaseCount++
        ledger.release(token)
    }

    @Composable
    private fun Effect(enabled: Boolean, stillNeeded: Boolean, awaitReady: suspend () -> Unit = {}) {
        SettingsRenderPauseEffect(
            enabled = enabled,
            stillNeeded = { stillNeeded },
            awaitReady = awaitReady,
            acquire = { acquire() },
            release = { release(it) },
            onReleased = { },
        )
    }

    @Test
    fun `closed before the pause is acquired leaves no dangling pause`() {
        composeRule.setContent {
            // effect 体捕获的 enabled 是启动时的快照：设置页此刻已关掉，快照仍是
            // true。判据必须读当前值——否则这里会领一次没人归还的暂停，账本计数
            // 停在 1，共享渲染器被永久暂停。
            Effect(enabled = true, stillNeeded = false)
        }
        composeRule.waitForIdle()

        assertEquals("设置页已关闭时不得领暂停", 0, acquireCount)
        assertEquals("账本计数必须归零", 0, ledger.holderCount)
        assertEquals(0, releaseCount)
    }

    @Test
    fun `closing the settings releases the pause it holds`() {
        val enabled: MutableState<Boolean> = mutableStateOf(true)
        composeRule.setContent {
            Effect(enabled = enabled.value, stillNeeded = enabled.value)
        }
        composeRule.waitForIdle()

        assertEquals(1, acquireCount)
        assertEquals(1, ledger.holderCount)

        enabled.value = false
        composeRule.waitForIdle()

        assertEquals("关闭设置页必须归还那次暂停", 1, releaseCount)
        assertEquals("账本计数必须归零", 0, ledger.holderCount)
    }

    @Test
    fun `cancelled while waiting for the renderer acquires nothing`() {
        val enabled: MutableState<Boolean> = mutableStateOf(true)
        composeRule.setContent {
            Effect(
                enabled = enabled.value,
                stillNeeded = enabled.value,
                awaitReady = { awaitCancellation() },
            )
        }
        composeRule.waitForIdle()

        enabled.value = false
        composeRule.waitForIdle()

        assertEquals("等待期间被取消不得领暂停", 0, acquireCount)
        assertEquals("账本计数必须归零", 0, ledger.holderCount)
    }
}
