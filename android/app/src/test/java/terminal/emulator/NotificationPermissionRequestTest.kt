package terminal.emulator

import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * `POST_NOTIFICATIONS` 请求的版本闸。
 *
 * 该权限自 Android 13（API 33）起存在：更早的平台上没有这个权限，
 * `checkSelfPermission` 恒为「未授予」，不带版本闸时每次启动都会派发一次
 * 对不存在权限的请求——平台直接拒发，用户看不到任何东西，只是白跑一趟。
 */
class NotificationPermissionRequestTest {

    @Test
    fun requestsOnAndroid13WhenNotGranted() {
        assertTrue(
            "API 33+ 且未授予时必须请求",
            shouldRequestNotificationPermission(sdkInt = 33, permissionGranted = false),
        )
    }

    @Test
    fun skipsWhenAlreadyGranted() {
        assertFalse(
            "已授予时不得重复请求",
            shouldRequestNotificationPermission(sdkInt = 33, permissionGranted = true),
        )
    }

    @Test
    fun skipsBeforeAndroid13() {
        for (sdk in intArrayOf(31, 32)) {
            assertFalse(
                "API $sdk 上没有该权限，不得发起请求",
                shouldRequestNotificationPermission(sdkInt = sdk, permissionGranted = false),
            )
        }
    }
}
