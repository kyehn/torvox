package terminal.emulator.benchmark

import androidx.test.uiautomator.UiDevice
import org.junit.rules.ExternalResource
import kotlin.math.abs

/*
 * 环境固定：把会漂移的量钉住，并把实际生效值记进测量行。
 *
 * 环境漂移会把测量结果带偏：刷新率改帧预算，动画缩放改界面动效时长，固定性能模式改
 * 调度，热状态改节流。故测量前钉住这些量；钉不住（实际生效值与目标不符）即当场失败
 * ——宁可没有数据，也不要一份看着齐全、实则在漂移的环境下测出的数字。
 *
 * 固定性能模式在部分设备上没有核对通道（命令被接受但状态无处可查），此时只记
 * 「不可核对」，不谎称已固定。
 */

/** 目标动画缩放：显式定成 1.0。 */
private const val TARGET_ANIMATION_SCALE = "1.0"

/** 目标热状态：NONE，即不节流。 */
private const val TARGET_THERMAL_STATUS = 0

/** 刷新率核对的容差（Hz）：显示模式记录的帧率有万分位的小数尾差。 */
private const val REFRESH_RATE_TOLERANCE_HZ = 0.01

/** 三类动画缩放的设置键。 */
private val ANIMATION_SCALE_KEYS = listOf(
    "window_animation_scale",
    "transition_animation_scale",
    "animator_duration_scale",
)

/** `dumpsys display` 的显示模式行：`id=1, width=320, height=640, fps=60.000004`。 */
private val DISPLAY_MODE_ROW = Regex("""id=(\d+), width=(\d+), height=(\d+), fps=([\d.]+)""")

/** `dumpsys display` 的活动模式行。 */
private val ACTIVE_MODE_ROW = Regex("""modeId (\d+)""")

/** `dumpsys power` 的固定性能模式字段（各版本命名有出入，按模式匹配）。 */
private val FIXED_PERFORMANCE_ROW = Regex("""(?i)\S*fixed\w*performance\w*=(true|false)""")

/** 设备的一个显示模式。 */
internal data class DisplayMode(
    /** 模式号。 */
    val id: Int,
    /** 宽（像素）。 */
    val width: Int,
    /** 高（像素）。 */
    val height: Int,
    /** 刷新率（Hz）。 */
    val refreshRateHz: Double,
)

/** 用户首选显示模式。 */
internal data class PreferredDisplayMode(
    /** 宽（像素）。 */
    val width: Int,
    /** 高（像素）。 */
    val height: Int,
    /** 刷新率（Hz）。 */
    val refreshRateHz: Double,
)

/** 三类动画缩放的实际值。 */
internal data class AnimationScales(
    /** 窗口动画缩放。 */
    val window: String,
    /** 过渡动画缩放。 */
    val transition: String,
    /** 时长动画缩放（Compose 的动效时长基准也取它）。 */
    val animatorDuration: String,
) {
    /** 一行式说明：进环境快照。 */
    fun describe(): String = "window=$window transition=$transition animatorDuration=$animatorDuration"
}

/** 固定性能模式的核对结论。 */
internal enum class FixedPerformanceState {
    /** 设备明确报告固定性能模式已开。 */
    ENABLED,

    /** 设备明确报告固定性能模式未开。 */
    DISABLED,

    /** 平台不提供核对通道：命令已下发，生效状态无处可查。 */
    UNVERIFIABLE,
}

/** 固定前的环境原值：测量结束后复原。 */
internal data class EnvironmentBaseline(
    /** 窗口动画缩放的原值（null 表示原先未设置）。 */
    val windowAnimationScale: String?,
    /** 过渡动画缩放的原值。 */
    val transitionAnimationScale: String?,
    /** 时长动画缩放的原值。 */
    val animatorDurationScale: String?,
    /** 用户首选显示模式的原值（null 表示原先未设置）。 */
    val preferredDisplayMode: PreferredDisplayMode?,
)

/** 环境固定的结果：每项的实际生效值与固定方式。 */
internal data class EnvironmentPinning(
    /** 实际生效的刷新率（Hz）。 */
    val refreshRateHz: Double,
    /** 刷新率的固定方式：`单一模式`（设备只有一个显示模式）或 `首选模式`（已下发并核对）。 */
    val refreshRatePinning: String,
    /** 三类动画缩放的实际值。 */
    val animationScales: AnimationScales,
    /** 实际生效的热状态码。 */
    val thermalStatus: Int,
    /** 固定性能模式的核对结论。 */
    val fixedPerformanceMode: FixedPerformanceState,
    /** 复原用的原值。 */
    val baseline: EnvironmentBaseline,
) {
    /** 一行式说明：进实测行，事后据此判断两轮数字是不是同一环境下的。 */
    fun describe(): String = "refreshRate=${refreshRateHz}Hz refreshRatePinning=$refreshRatePinning " +
        "animationScales=${animationScales.describe()} thermalStatus=$thermalStatus " +
        "fixedPerformanceMode=$fixedPerformanceMode"
}

/** 解析 `dumpsys display` 给出的活动显示设备的显示模式列表。 */
internal fun parseDisplayModes(output: String): List<DisplayMode> {
    val row = output.lineSequence().firstOrNull { it.contains("DisplayDeviceInfo{") }
        ?: error("显示 dump 里读不到 DisplayDeviceInfo")
    val modes = DISPLAY_MODE_ROW.findAll(row).map { match ->
        DisplayMode(
            id = match.groupValues[1].toInt(),
            width = match.groupValues[2].toInt(),
            height = match.groupValues[3].toInt(),
            refreshRateHz = match.groupValues[4].toDouble(),
        )
    }.toList()
    if (modes.isEmpty()) error("显示 dump 里读不到显示模式：$row")
    return modes
}

/** 解析 `dumpsys display` 给出的活动显示模式。 */
internal fun parseActiveDisplayMode(output: String): DisplayMode {
    val row = output.lineSequence().firstOrNull { it.contains("DisplayDeviceInfo{") }
        ?: error("显示 dump 里读不到 DisplayDeviceInfo")
    val activeId = ACTIVE_MODE_ROW.find(row)?.groupValues?.get(1)?.toIntOrNull()
        ?: error("显示 dump 里读不到活动模式号：$row")
    return parseDisplayModes(output).firstOrNull { it.id == activeId }
        ?: error("显示 dump 里读不到活动显示模式：modeId=$activeId")
}

/** 解析 `cmd display get-user-preferred-display-mode` 的输出；未设置时返回 null。 */
internal fun parsePreferredDisplayMode(output: String): PreferredDisplayMode? {
    val text = output.substringAfter("User preferred display mode:", "").trim()
    if (text.isEmpty() || text == "null") return null
    val parts = text.split(Regex("\\s+"))
    if (parts.size != 3) error("解析不到用户首选显示模式：$output")
    val refreshRateHz = parts[2].toDoubleOrNull()
        ?: error("解析不到用户首选显示模式的刷新率：$output")
    val width = parts[0].toIntOrNull() ?: error("解析不到用户首选显示模式的宽度：$output")
    val height = parts[1].toIntOrNull() ?: error("解析不到用户首选显示模式的高度：$output")
    return PreferredDisplayMode(width, height, refreshRateHz)
}

/** 解析 `dumpsys power` 的固定性能模式状态；设备不提供该字段时不可核对。 */
internal fun parseFixedPerformanceMode(output: String): FixedPerformanceState {
    val row = output.lineSequence()
        .map { it.trim() }
        .firstOrNull { FIXED_PERFORMANCE_ROW.matchEntire(it) != null }
        ?: return FixedPerformanceState.UNVERIFIABLE
    val enabled = FIXED_PERFORMANCE_ROW.matchEntire(row)?.groupValues?.get(1)?.toBooleanStrictOrNull()
        ?: return FixedPerformanceState.UNVERIFIABLE
    return if (enabled) FixedPerformanceState.ENABLED else FixedPerformanceState.DISABLED
}

/** 读一个全局设置项；未设置时返回 null。 */
internal fun readGlobalSetting(device: UiDevice, key: String): String? =
    device.executeShellCommand("settings get global $key").trim()
        .takeIf { it.isNotEmpty() && it != "null" }

/** 读三类动画缩放的实际值。 */
internal fun readAnimationScales(device: UiDevice): AnimationScales = AnimationScales(
    window = readGlobalSetting(device, "window_animation_scale") ?: "1.0",
    transition = readGlobalSetting(device, "transition_animation_scale") ?: "1.0",
    animatorDuration = readGlobalSetting(device, "animator_duration_scale") ?: "1.0",
)

/**
 * 固定测量环境，并读回实际生效值。
 *
 * 任何一项的实际值与目标不符即当场失败：环境还在漂移时测得的数字不可比。
 */
internal fun pinMeasurementEnvironment(device: UiDevice): EnvironmentPinning {
    val baseline = readEnvironmentBaseline(device)
    val refreshRate = pinRefreshRate(device)
    val animationScales = pinAnimationScales(device)
    val thermalStatus = pinThermalStatus(device)
    val fixedPerformanceMode = pinFixedPerformanceMode(device)
    return EnvironmentPinning(
        refreshRateHz = refreshRate.first,
        refreshRatePinning = refreshRate.second,
        animationScales = animationScales,
        thermalStatus = thermalStatus,
        fixedPerformanceMode = fixedPerformanceMode,
        baseline = baseline,
    )
}

/** 读固定前的环境原值。 */
private fun readEnvironmentBaseline(device: UiDevice): EnvironmentBaseline = EnvironmentBaseline(
    windowAnimationScale = readGlobalSetting(device, "window_animation_scale"),
    transitionAnimationScale = readGlobalSetting(device, "transition_animation_scale"),
    animatorDurationScale = readGlobalSetting(device, "animator_duration_scale"),
    preferredDisplayMode = parsePreferredDisplayMode(
        device.executeShellCommand("cmd display get-user-preferred-display-mode"),
    ),
)

/**
 * 固定刷新率：设备只有一个显示模式时，刷新率没有别的取值，天然固定；有多个模式时
 * 下发用户首选模式并核对实际生效值。
 */
private fun pinRefreshRate(device: UiDevice): Pair<Double, String> {
    val displayDump = device.executeShellCommand("dumpsys display")
    val activeMode = parseActiveDisplayMode(displayDump)
    if (parseDisplayModes(displayDump).size <= 1) {
        return activeMode.refreshRateHz to "单一模式"
    }
    val commandOutput = device.executeShellCommand(
        "cmd display set-user-preferred-display-mode " +
            "${activeMode.width} ${activeMode.height} ${activeMode.refreshRateHz}",
    )
    val effectiveRateHz = parseRefreshRateHz(device.executeShellCommand("dumpsys display"))
    check(abs(effectiveRateHz - activeMode.refreshRateHz) <= REFRESH_RATE_TOLERANCE_HZ) {
        "刷新率没有固定住：请求 ${activeMode.refreshRateHz}Hz，实际 ${effectiveRateHz}Hz；命令输出：$commandOutput"
    }
    return effectiveRateHz to "首选模式"
}

/** 固定三类动画缩放，并核对实际生效值。 */
private fun pinAnimationScales(device: UiDevice): AnimationScales {
    for (key in ANIMATION_SCALE_KEYS) {
        device.executeShellCommand("settings put global $key $TARGET_ANIMATION_SCALE")
    }
    val scales = readAnimationScales(device)
    check(
        scales.window == TARGET_ANIMATION_SCALE &&
            scales.transition == TARGET_ANIMATION_SCALE &&
            scales.animatorDuration == TARGET_ANIMATION_SCALE,
    ) {
        "动画缩放没有固定住：实际 ${scales.describe()}"
    }
    return scales
}

/** 固定热状态（不节流），并核对实际生效值。 */
private fun pinThermalStatus(device: UiDevice): Int {
    val commandOutput = device.executeShellCommand("cmd thermalservice override-status $TARGET_THERMAL_STATUS")
    val status = parseThermalStatus(device.executeShellCommand("dumpsys thermalservice"))
    check(status == TARGET_THERMAL_STATUS) {
        "热状态没有固定住：请求 $TARGET_THERMAL_STATUS，实际 $status；命令输出：$commandOutput"
    }
    return status
}

/** 固定性能模式：平台不提供核对通道时只记「不可核对」。 */
private fun pinFixedPerformanceMode(device: UiDevice): FixedPerformanceState {
    val commandOutput = device.executeShellCommand("cmd power set-fixed-performance-mode-enabled true")
    val state = parseFixedPerformanceMode(device.executeShellCommand("dumpsys power"))
    check(state != FixedPerformanceState.DISABLED) {
        "固定性能模式没有生效：设备报告未开启；命令输出：$commandOutput"
    }
    return state
}

/**
 * 复原测量环境：解除热状态覆盖、关掉固定性能模式、还原用户首选显示模式与动画缩放。
 *
 * 复原的是固定前的原值，不是「设备出厂默认」：设备上原有的设置属用户的，不该被测量
 * 顺手改掉。
 */
internal fun restoreMeasurementEnvironment(device: UiDevice, baseline: EnvironmentBaseline) {
    device.executeShellCommand("cmd thermalservice reset")
    device.executeShellCommand("cmd power set-fixed-performance-mode-enabled false")
    val preferredMode = baseline.preferredDisplayMode
    if (preferredMode == null) {
        device.executeShellCommand("cmd display clear-user-preferred-display-mode")
    } else {
        device.executeShellCommand(
            "cmd display set-user-preferred-display-mode " +
                "${preferredMode.width} ${preferredMode.height} ${preferredMode.refreshRateHz}",
        )
    }
    restoreGlobalSetting(device, "window_animation_scale", baseline.windowAnimationScale)
    restoreGlobalSetting(device, "transition_animation_scale", baseline.transitionAnimationScale)
    restoreGlobalSetting(device, "animator_duration_scale", baseline.animatorDurationScale)
}

/** 复原一个全局设置项；原值为 null 表示原先未设置，复原即删掉该项。 */
private fun restoreGlobalSetting(device: UiDevice, key: String, value: String?) {
    if (value == null) {
        device.executeShellCommand("settings delete global $key")
    } else {
        device.executeShellCommand("settings put global $key $value")
    }
}

/**
 * 环境固定规则：每条用例前固定环境，用例后复原。
 *
 * 复原在用例结束时做，即便用例失败也会执行——测量不该把设备留在被改过的状态里。
 */
class EnvironmentPinningRule(private val device: UiDevice) : ExternalResource() {
    private var baseline: EnvironmentBaseline? = null

    override fun before() {
        val pinning = pinMeasurementEnvironment(device)
        baseline = pinning.baseline
        emitMeasured("[bench-pin] ${pinning.describe()}")
    }

    override fun after() {
        baseline?.let { restoreMeasurementEnvironment(device, it) }
    }
}
