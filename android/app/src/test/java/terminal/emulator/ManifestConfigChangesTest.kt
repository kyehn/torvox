package terminal.emulator

import android.content.Context
import android.content.pm.ActivityInfo
import android.content.pm.PackageManager
import androidx.test.core.app.ApplicationProvider
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config

/**
 * Activity 的 `configChanges` 声明。
 *
 * 少声明一项，该项变化就重建 Activity：会话与 PTY 状态一起丢掉，用户看到的是
 * 「插拔键盘/改显示大小/切导航模式后终端重开」。清单是声明的唯一来源，故此处读
 * 合并后的清单断言位掩码——源码里的字符串改法不构成证据。
 */
@RunWith(RobolectricTestRunner::class)
@Config(sdk = [34])
class ManifestConfigChangesTest {

    @Test
    fun `main activity declares density and shape qualifiers`() {
        val context = ApplicationProvider.getApplicationContext<Context>()
        val packageInfo =
            context.packageManager.getPackageInfo(
                context.packageName,
                PackageManager.GET_ACTIVITIES,
            )
        val activity =
            checkNotNull(
                packageInfo.activities?.firstOrNull { it.name.endsWith(".MainActivity") },
            ) { "清单里必须有 MainActivity" }
        val declared = activity.configChanges

        val required =
            mapOf(
                "density" to ActivityInfo.CONFIG_DENSITY,
                "smallestScreenSize" to ActivityInfo.CONFIG_SMALLEST_SCREEN_SIZE,
                "keyboard" to ActivityInfo.CONFIG_KEYBOARD,
                "navigation" to ActivityInfo.CONFIG_NAVIGATION,
            )
        required.forEach { (qualifier, flag) ->
            assertTrue("configChanges 必须声明 $qualifier", declared and flag != 0)
        }

        val retained =
            mapOf(
                "orientation" to ActivityInfo.CONFIG_ORIENTATION,
                "screenSize" to ActivityInfo.CONFIG_SCREEN_SIZE,
                "screenLayout" to ActivityInfo.CONFIG_SCREEN_LAYOUT,
                "keyboardHidden" to ActivityInfo.CONFIG_KEYBOARD_HIDDEN,
                "uiMode" to ActivityInfo.CONFIG_UI_MODE,
                "fontScale" to ActivityInfo.CONFIG_FONT_SCALE,
            )
        retained.forEach { (qualifier, flag) ->
            assertTrue("configChanges 必须保留 $qualifier", declared and flag != 0)
        }
    }
}
