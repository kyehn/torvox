package terminal.emulator

import android.annotation.SuppressLint
import android.app.Application
import android.content.ClipboardManager
import android.content.Context
import android.content.Intent
import androidx.test.core.app.ApplicationProvider
import io.mockk.mockk
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.flow.MutableStateFlow
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.Shadows.shadowOf
import org.robolectric.annotation.Config
import terminal.emulator.runtime.TerminalRuntime

/**
 * 选区复制与分享的载荷上限。
 *
 * 大回滚区全选可达数百万字符，全部塞进 `EXTRA_TEXT` 会超过 Binder 事务上限，
 * `startActivity` 抛 `TransactionTooLargeException`——分享按钮变成崩溃入口。
 *
 * 截断点落在代理对中间时多取一字符保住完整码点：孤立的代理项进剪贴板或
 * Intent 后会被目标应用显示成替换字符，等于在边界处凭空造出一个乱码。
 */
@RunWith(RobolectricTestRunner::class)
@Config(sdk = [34])
class SelectionPayloadLimitTest {
    private fun managerWithText(text: String): SelectionManager {
        val application = ApplicationProvider.getApplicationContext<Application>()
        return SelectionManager(
            terminalState =
            MutableStateFlow(
                TerminalState(
                    selection =
                    SelectionState(
                        active = true,
                        start = SelectionAnchor(0, 0),
                        end = SelectionAnchor(0, 1),
                        selectedText = text,
                    ),
                ),
            ),
            runtime = mockk<TerminalRuntime>(relaxed = true),
            context = application,
            scope = CoroutineScope(Dispatchers.Unconfined),
        )
    }

    @Test
    fun `share caps the intent text at the clipboard limit`() {
        val application = ApplicationProvider.getApplicationContext<Application>()
        val manager = managerWithText("a".repeat(CLIPBOARD_TEXT_MAX_LENGTH * 3))

        manager.shareSelection()

        val chooser = shadowOf(application).nextStartedActivity
        val target = chooser.getParcelableExtra(Intent.EXTRA_INTENT, Intent::class.java)
        assertEquals(CLIPBOARD_TEXT_MAX_LENGTH, target?.getStringExtra(Intent.EXTRA_TEXT)?.length)
    }

    @Test
    fun `share keeps the surrogate pair intact at the truncation boundary`() {
        val application = ApplicationProvider.getApplicationContext<Application>()
        // 上限恰好切在表情中间：高代理项落在末位，低代理项落在界外。
        val manager = managerWithText("a".repeat(CLIPBOARD_TEXT_MAX_LENGTH - 1) + "😀" + "b")

        manager.shareSelection()

        val chooser = shadowOf(application).nextStartedActivity
        val target = chooser.getParcelableExtra(Intent.EXTRA_INTENT, Intent::class.java)
        val shared = target?.getStringExtra(Intent.EXTRA_TEXT)
        assertEquals(CLIPBOARD_TEXT_MAX_LENGTH + 1, shared?.length)
        assertTrue("截断不得留下半个代理项", shared?.endsWith("😀") == true)
    }

    @Test
    @SuppressLint("DeprecatedCall") // primaryClip/getPrimaryClip: no @Deprecated in API 37; slack-lint rule data lag
    fun `copy keeps the surrogate pair intact at the truncation boundary`() {
        val application = ApplicationProvider.getApplicationContext<Application>()
        val manager = managerWithText("a".repeat(CLIPBOARD_TEXT_MAX_LENGTH - 1) + "😀" + "b")

        manager.copySelectionToClipboard()

        val clipboard = application.getSystemService(Context.CLIPBOARD_SERVICE) as ClipboardManager
        val copied = clipboard.primaryClip?.getItemAt(0)?.text?.toString()
        assertEquals(CLIPBOARD_TEXT_MAX_LENGTH + 1, copied?.length)
        assertTrue("截断不得留下半个代理项", copied?.endsWith("😀") == true)
    }
}
