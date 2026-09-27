package terminal.emulator.benchmark

import androidx.test.uiautomator.UiDevice

/** 被测应用包名，与 `android/app/build.gradle.kts` 的 applicationId 一致。 */
internal const val TARGET_PACKAGE = "com.termux"

/** 首启会弹通知授权对话框，缺授权时启动指标失真。 */
internal const val NOTIFICATION_PERMISSION = "android.permission.POST_NOTIFICATIONS"

/** Compose `testTag` 经 `testTagsAsResourceId` 暴露为 resource-id，可直接用于 UiAutomator 定位。 */
internal const val MODIFIER_BAR_RESOURCE_ID = "ModifierBar"

internal const val CONTROL_KEY_RESOURCE_ID = "Key_CTRL"

internal const val DRAWER_KEY_RESOURCE_ID = "Key_DRAWER"

/** 宏基准会重装被测应用并重置运行时权限，需在测量前重新授权。 */
internal fun grantNotificationPermission(device: UiDevice) {
    device.executeShellCommand("pm grant $TARGET_PACKAGE $NOTIFICATION_PERMISSION")
}
