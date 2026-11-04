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
 * 回滚区填满后上滑、下滑各一次（含惯性）的耗时上限。
 *
 * 预算按实测校准：模拟器（SwiftShader）上两次 1.5s 拖动实测 3543/3548/3568ms
 * （三轮，2026-11-04 本机 AVD），取实测的约 1.5 倍口径（与 Rust 微基准上限断言
 * 同口径）留出 CI 机器的抖动余量。
 */
internal const val SCROLL_BUDGET_MILLIS = 6_000L

/**
 * 滚动至少要呈现的帧数下限。
 *
 * 帧数由原生渲染器的呈现汇总给出（终端内容直绘 SurfaceView，`dumpsys gfxinfo`
 * 记不到，见 [PresentedFrames]）。
 *
 * 呈现帧数是**宿主吞吐**的函数，不是被测逻辑的函数：同一台模拟器（SwiftShader）
 * 在空闲宿主上两次拖动实测呈现 209/211/214 帧，在被并发任务抢占的宿主上只呈现
 * 65 帧（同一构建、同一手势、同一 AVD）。故下限按**最差实测**取约一半：
 * 65/2 ≈ 30，低于此即「滚动几乎没有出帧」，正是本用例要拦的回归形态。
 * 宿主速度相关的性能结论由耗时预算与逐窗连续性判据承担，不由此绝对值承担。
 */
internal const val MIN_SCROLL_PRESENTED_FRAMES = 30

/**
 * 滚动期间至少要有帧的呈现窗口数下限。
 *
 * 原生侧按 500ms 窗口汇报呈现帧数，**没有帧的窗口不汇报**（见
 * `native/src/render/pass/frame.rs` 的 `present_log.record`）。故汇报窗口数
 * 即「滚动期间仍在持续出帧」的时长：拖动约 3.5s 覆盖 7-8 个窗口，空闲宿主与
 * 满载宿主实测都是 8 个窗口。
 *
 * 该判据与宿主速度无关：它拦的是「滚了一下就不动了」（窗口数塌到 1-2），
 * 这才是用户可见的卡死形态；绝对帧数另由 [MIN_SCROLL_PRESENTED_FRAMES] 兜底。
 */
internal const val MIN_SCROLL_PRESENTED_WINDOWS = 4

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
