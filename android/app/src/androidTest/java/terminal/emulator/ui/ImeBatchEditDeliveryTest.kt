package terminal.emulator.ui

import android.view.inputmethod.EditorInfo
import android.view.inputmethod.InputConnection
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Test
import terminal.emulator.ComposeTerminalLogcatTest
import terminal.emulator.UxTestUtils
import terminal.emulator.awaitBridge
import terminal.emulator.awaitShellPrompt
import terminal.emulator.bridge.NativeBridge
import terminal.emulator.findTerminalSurface
import terminal.emulator.onUiThread
import terminal.emulator.tapToFocus
import terminal.emulator.waitForSession

/**
 * 批量编辑期间的即时送达契约。
 *
 * `ime-editor-contract`「批量编辑如实受理」要求：输入法用
 * `beginBatchEdit` / `endBatchEdit` 给一段编辑分组，分组只是**标记**——终端不得
 * 因此推迟任何一次提交。批量进行中的提交必须当场走既有出口，不得滞留到批量
 * 结束之后。
 *
 * 判别力：把提交改成「攒到 `endBatchEdit` 才发」，本用例在批量结束前等不到标记，
 * 直接判红；正常路径下标记在批量结束前就已回显进终端文本。本仓的实现里批量计数
 * 只被写、从不被读（`BatchEditDepth` 没有旁路），提交路径上也没有任何按批量状态
 * 分支的缓冲——本用例把这条结构保证钉成可观测的行为。
 */
class ImeBatchEditDeliveryTest : ComposeTerminalLogcatTest() {

    private companion object {
        /** 提交标记：必须出现在终端文本里，且不得与其它用例的串冲突。 */
        const val MARKER = "batch-delivery-probe"

        /** 等标记回显的上限（毫秒）。远大于一帧，足以覆盖渲染与回显时延。 */
        const val DELIVERY_TIMEOUT_MS = 10_000L
    }

    @Before
    fun setUp() {
        composeTestRule.waitForSession()
    }

    @Test
    fun commit_during_batch_edit_reaches_the_terminal_before_the_batch_ends() {
        val bridge = composeTestRule.awaitBridge()
        awaitShellPrompt(bridge)

        val surface = onUiThread { findTerminalSurface(composeTestRule.activity) }
        tapToFocus(surface)
        val input = onUiThread { surface.onCreateInputConnection(EditorInfo()) }
        assertNotNull("onCreateInputConnection 返回 null", input)
        val connection = requireNotNull(input)

        // 获焦后的抑制窗口内写入被静默拒收（返回 false 让输入法重发）。先等一次
        // 真的受理，否则后面「等不到标记」分不清是缺陷还是抑制窗口。
        val accepted =
            UxTestUtils.pollUntilTrue(timeoutMs = 5_000, intervalMs = 100) {
                onUiThread { connection.setComposingText("warmup", 1) } == true
            }
        assertNotNull("输入法写入一直被拒（抑制窗口），本用例无判别力", accepted)

        // 批量开始后提交一次。标记必须在**批量结束前**送达，故此处绝不先调
        // `endBatchEdit`——那正是缺陷实现的出口，提前调用会把缺陷掩盖掉。
        onUiThread { assertTrue("beginBatchEdit 未被受理", connection.beginBatchEdit()) }
        try {
            val committed = onUiThread { connection.commitText(MARKER, 1) }
            assertTrue("commitText 在抑制窗口内被拒，标记不会送达", committed)

            val delivered =
                UxTestUtils.pollUntilTrue(timeoutMs = DELIVERY_TIMEOUT_MS, intervalMs = 100) {
                    NativeBridge.pollEvent()
                    bridge.getTerminalText()?.contains(MARKER) == true
                }
            assertNotNull(
                "批量编辑结束前提交仍未送达终端（被延迟到 endBatchEdit 之后才发）",
                delivered,
            )
        } finally {
            onUiThread { connection.endBatchEdit() }
        }
    }
}
