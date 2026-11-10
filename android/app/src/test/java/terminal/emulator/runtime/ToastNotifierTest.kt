package terminal.emulator.runtime

import android.content.Context
import androidx.test.core.app.ApplicationProvider
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.Shadows
import org.robolectric.annotation.Config
import org.robolectric.shadows.ShadowToast

/**
 * [ToastNotifier] 的「同一时刻只留一条」约束。
 *
 * 判别力：平台对同一 UID 同时只允许一个 Toast 窗口，旧窗口未摘除时再挂一条会抛
 * `BadTokenException`，平台同时打出 W 级告警。取消仍在显示的那条，平台才会先摘
 * 旧窗口再挂新窗口——这正是连续切换字体时触发过的那条告警的根因。
 */
@RunWith(RobolectricTestRunner::class)
@Config(sdk = [34])
class ToastNotifierTest {

    private fun context(): Context = ApplicationProvider.getApplicationContext()

    @Test
    fun `a new toast cancels the one still showing`() {
        ToastNotifier.show(context(), "第一条")
        val first = ShadowToast.getLatestToast()
        assertNotNull(first)

        ToastNotifier.show(context(), "第二条")

        assertTrue(Shadows.shadowOf(first).isCancelled)
    }

    @Test
    fun `the latest toast carries the newest text`() {
        ToastNotifier.show(context(), "第一条")
        ToastNotifier.show(context(), "第二条")

        assertTrue(ShadowToast.showedToast("第二条"))
    }
}
