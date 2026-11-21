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
 * 冷启动各轮耗时中位数的上限，以及最差一轮的硬上限。
 *
 * 判定取各轮中位数：预算是按实测中位数校准的，而宿主被并发任务抢占时个别迭代会
 * 明显变慢，拿最差一轮当判据会把宿主抖动算成应用回归；最差一轮另过硬上限，拦的是
 * 「一轮就慢到不可用」。
 *
 * 预算是数量级门禁，不是性能 SLO：模拟器宏基准数据不用于建立物理设备性能结论。
 * 余量按宿主负载实测定：2026-11-21 本机 AVD（API 35 模拟器，软件渲染）十轮冷启动，
 * 宿主轻载（负载均值 0.4–3.2）实测 1903–2411ms（中位 2011ms），宿主被 8 个忙循环
 * 超额占用（4 核宿主负载 11–17）实测 3327–4486ms（中位 3694ms）——宿主被抢占时整
 * 轮都变慢，不是个别迭代，故按中位数判定并留出覆盖该情形的余量。
 *
 * 精确耗时由 `StartupTimingMetric` / `FrameTimingMetric` 输出，真机阈值需另行标定；
 * 这里的断言保证基准失败时能指出「慢到不可用」，而不是只留下一串没人看的数字。
 */
internal const val COLD_START_BUDGET_MILLIS = 5_000L

internal const val COLD_START_HARD_LIMIT_MILLIS = 10_000L

/**
 * 热启动各轮耗时中位数的上限，以及最差一轮的硬上限。
 *
 * 口径与 [COLD_START_BUDGET_MILLIS] 相同。实测（2026-11-21 本机 AVD）：宿主轻载十
 * 一轮 1022–1479ms（中位 1098ms），宿主超额占用时 1234–2199ms（中位 1570ms）。
 */
internal const val WARM_START_BUDGET_MILLIS = 2_400L

internal const val WARM_START_HARD_LIMIT_MILLIS = 5_000L

internal const val INTERACTION_BUDGET_MILLIS = 4_000L

/**
 * 回滚区填满后上滑、下滑各一次（含惯性）的耗时上限。
 *
 * 预算按实测校准：模拟器（SwiftShader）上两次 1.5s 拖动实测 3543/3548/3568ms
 * （三轮，2026-11-04 本机 AVD），2026-11-06 本机 AVD 复核 3551–3559ms，取实测的
 * 约 1.5 倍口径（与 Rust 微基准上限断言同口径）留出宿主抖动余量。
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
 * 滚动窗口内终端内容层至少要占用的刷新槽位比例。
 *
 * 该比例是「内容层的呈现频率」除以「设备刷新率」，与设备刷新率无关（见
 * [SurfaceLayerFrames]）。
 *
 * 丢帧数不适合当阈值：它是保留窗口内的绝对量，会随窗口内帧数一起变大——宿主被
 * 并发任务抢占时帧数与丢帧数同涨（实测空闲宿主丢帧 10/13/14 帧，被抢占时丢 41
 * 帧），按最差实测放宽的阈值会失去拦截力，按空闲实测收紧的阈值又会在正常负载下
 * 误报。槽位比例是相对量：宿主被抢占时帧数与窗口长度一起缩小，比例不会跟着塌。
 *
 * 预算按实测校准：2026-11-21 本机 AVD（API 35 模拟器，软件渲染）滚动宏基准三轮，
 * 宿主轻到中载（负载均值 3.5–4.3）实测槽位比例 0.90/0.90/0.93 与 0.80/0.86/0.96
 * （呈现 48–56fps，丢帧 11–14）；宿主被 8 个忙循环超额占用（4 核宿主负载 11–13）时
 * 实测 0.49/0.51/0.56/0.57/0.58（呈现 29–35fps，丢帧 89–113）。取最差实测的约 1.5
 * 倍余量口径（与本文件其他预算同口径）定在 0.35。
 */
internal const val MIN_SCROLL_LAYER_SLOT_UTILIZATION = 0.35

/**
 * 交互动画至少要出帧。
 *
 * 0 帧说明交互没有产生任何绘制（界面没动，指标测到的只是静止画面）。下限 MUST 取 1，
 * MUST NOT 取 2：一次点击在软件渲染的模拟器上常在同一帧内完成按下、回弹与落定，
 * `dumpsys gfxinfo` 只记到 1 帧（模拟器实测），按 2 判定会把「动画发生得很快」判成失败。
 * 「动画真的发生了」由各用例的状态断言兜底（粘滞修饰态置起、抽屉节点出现、输入法真的
 * 弹出），帧数只用来排除「什么都没画」。
 */
internal const val MIN_ANIMATION_FRAME_COUNT = 1

/** 宏基准会重装被测应用并重置运行时权限，需在测量前重新授权。 */
internal fun grantNotificationPermission(device: UiDevice) {
    device.executeShellCommand("pm grant $TARGET_PACKAGE $NOTIFICATION_PERMISSION")
}
