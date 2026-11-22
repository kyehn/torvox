package terminal.emulator

import android.app.Application
import android.content.Intent
import androidx.test.core.app.ApplicationProvider
import io.mockk.mockk
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.flow.MutableStateFlow
import org.junit.Assert.assertEquals
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.Shadows.shadowOf
import org.robolectric.annotation.Config
import terminal.emulator.runtime.TerminalRuntime

/**
 * 分享选区的 Intent 载荷上限。
 *
 * 大回滚区全选可达数百万字符，全部塞进 `EXTRA_TEXT` 会超过 Binder 事务上限，
 * `startActivity` 抛 `TransactionTooLargeException`——分享按钮变成崩溃入口。
 */
@RunWith(RobolectricTestRunner::class)
@Config(sdk = [34])
class SelectionShareTest {
    @Test
    fun `share caps the intent text at the clipboard limit`() {
        val application = ApplicationProvider.getApplicationContext<Application>()
        val manager =
            SelectionManager(
                terminalState =
                MutableStateFlow(
                    TerminalState(
                        selection =
                        SelectionState(
                            active = true,
                            start = SelectionAnchor(0, 0),
                            end = SelectionAnchor(0, 1),
                            selectedText = "a".repeat(CLIPBOARD_TEXT_MAX_LENGTH * 3),
                        ),
                    ),
                ),
                runtime = mockk<TerminalRuntime>(relaxed = true),
                context = application,
                scope = CoroutineScope(Dispatchers.Unconfined),
            )

        manager.shareSelection()

        val chooser = shadowOf(application).nextStartedActivity
        val target = chooser.getParcelableExtra(Intent.EXTRA_INTENT, Intent::class.java)
        assertEquals(CLIPBOARD_TEXT_MAX_LENGTH, target?.getStringExtra(Intent.EXTRA_TEXT)?.length)
    }
}
