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

internal const val TERMINAL_SCREEN_RESOURCE_ID = "TerminalScreen"

internal const val SESSION_DRAWER_RESOURCE_ID = "SessionDrawer"

/** 等待界面元素出现的上限；超时即失败，不允许把「没出现」当成通过。 */
internal const val UI_TIMEOUT_MILLIS = 10_000L

/**
 * 模拟器（软件渲染）上的粗粒度上限，只拦「卡死级」回归，不是性能 SLO。
 *
 * 精确耗时由 `StartupTimingMetric` / `FrameTimingMetric` 输出，真机阈值需另行标定；
 * 这里的断言保证基准失败时能指出「慢到不可用」，而不是只留下一串没人看的数字。
 */
internal const val COLD_START_BUDGET_MILLIS = 6_000L

internal const val WARM_START_BUDGET_MILLIS = 3_000L

internal const val INTERACTION_BUDGET_MILLIS = 4_000L

/**
 * 回滚区填满后上滑、下滑各一次（含事件队列等待）的粗粒度上限，同样只拦
 * 「卡死级」回归。滚动是连续出帧的重活，软件渲染上单次手势就是数百毫秒级，
 * 初值取得宽松；`printMeasured` 落进 CI 的实测值出来后按基准收紧。
 */
internal const val SCROLL_BUDGET_MILLIS = 10_000L

/**
 * 交互动画至少要出帧。
 *
 * 0 帧说明交互没有产生任何绘制（界面没动，指标测到的只是静止画面）。下限 MUST 取 1，
 * MUST NOT 取 2：一次点击在软件渲染的模拟器上常在同一帧内完成按下、回弹与落定，
 * `dumpsys gfxinfo` 只记到 1 帧（CI 实测），按 2 判定会把「动画发生得很快」判成失败。
 * 「动画真的发生了」由各用例的状态断言兜底（粘滞修饰态置起、抽屉节点出现、输入法真的
 * 弹出），帧数只用来排除「什么都没画」。
 */
internal const val MIN_ANIMATION_FRAME_COUNT = 1

/** 宏基准会重装被测应用并重置运行时权限，需在测量前重新授权。 */
internal fun grantNotificationPermission(device: UiDevice) {
    device.executeShellCommand("pm grant $TARGET_PACKAGE $NOTIFICATION_PERMISSION")
}
