package terminal.emulator.ui

import android.content.Context
import android.view.inputmethod.CorrectionInfo
import android.view.inputmethod.EditorInfo
import androidx.test.core.app.ApplicationProvider
import org.junit.Assert.assertFalse
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config

/**
 * 校正请求诚实性（6.2 按 6.1 结论拒绝）：6.1 已证默认路径不分发校正，
 * 故连接必须返回拒绝；输入法停止调用比谎称生效安全。
 */
@RunWith(RobolectricTestRunner::class)
@Config(sdk = [34])
class ImeCommitCorrectionTest {
    @Test
    fun `commitCorrection is rejected until proven`() {
        val context = ApplicationProvider.getApplicationContext<Context>()
        val surface = TerminalSurface(context)
        val connection = surface.onCreateInputConnection(EditorInfo())
        assertFalse(connection.commitCorrection(CorrectionInfo(0, "a", "b")))
    }
}
