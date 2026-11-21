package terminal.emulator.benchmark

import android.os.BatteryManager
import android.os.PowerManager
import androidx.test.uiautomator.UiDevice

/**
 * 一次测量所处的设备环境快照。
 *
 * 环境漂移会把测量结果带偏：刷新率改帧预算，电池状态与热状态改调度，ROM 与型号
 * 决定绝对量级。故每次测量都把环境记进实测行——事后拿到两轮数字时，第一件事就是
 * 判断它们是不是同一环境下的数字。
 */
internal data class DeviceEnvironment(
    /** 设备型号（`ro.product.model`）。 */
    val model: String,
    /** ROM 构建标识（`ro.build.display.id`）。 */
    val romBuildId: String,
    /** API 级别（`ro.build.version.sdk`）。 */
    val apiLevel: Int,
    /** 活动显示的渲染帧率（Hz）。 */
    val refreshRateHz: Double,
    /** 电池电量百分比。 */
    val batteryLevelPercent: Int,
    /** 电池状态（`BatteryManager.BATTERY_STATUS_*`）。 */
    val batteryStatus: Int,
    /** 电池温度（摄氏度）。 */
    val batteryTemperatureCelsius: Double,
    /** 热状态（`PowerManager.THERMAL_STATUS_*`）。 */
    val thermalStatus: Int,
    /** 被测应用的版本号（versionCode）。 */
    val appVersionCode: Long,
    /** 被测应用的版本名（versionName）。 */
    val appVersionName: String,
    /** 快照时刻（epoch 毫秒）。 */
    val capturedAtEpochMillis: Long,
) {
    /** 一行式说明：进实测行，事后据此判断两轮测量是否同一环境。 */
    fun describe(): String = "model=$model romBuildId=\"$romBuildId\" api=$apiLevel " +
        "refreshRate=${refreshRateHz}Hz battery=$batteryLevelPercent% " +
        "batteryStatus=${batteryStatusName(batteryStatus)} " +
        "batteryTemperature=${batteryTemperatureCelsius}C thermal=${thermalStatusName(thermalStatus)} " +
        "versionCode=$appVersionCode versionName=$appVersionName"
}

/** 电池状态的可读名。 */
private fun batteryStatusName(status: Int): String = when (status) {
    BatteryManager.BATTERY_STATUS_CHARGING -> "CHARGING"
    BatteryManager.BATTERY_STATUS_DISCHARGING -> "DISCHARGING"
    BatteryManager.BATTERY_STATUS_NOT_CHARGING -> "NOT_CHARGING"
    BatteryManager.BATTERY_STATUS_FULL -> "FULL"
    BatteryManager.BATTERY_STATUS_UNKNOWN -> "UNKNOWN"
    else -> "UNKNOWN($status)"
}

/** 热状态的可读名。 */
private fun thermalStatusName(status: Int): String = when (status) {
    PowerManager.THERMAL_STATUS_NONE -> "NONE"
    PowerManager.THERMAL_STATUS_LIGHT -> "LIGHT"
    PowerManager.THERMAL_STATUS_MODERATE -> "MODERATE"
    PowerManager.THERMAL_STATUS_SEVERE -> "SEVERE"
    PowerManager.THERMAL_STATUS_CRITICAL -> "CRITICAL"
    PowerManager.THERMAL_STATUS_EMERGENCY -> "EMERGENCY"
    PowerManager.THERMAL_STATUS_SHUTDOWN -> "SHUTDOWN"
    else -> "UNKNOWN($status)"
}

/** `getprop` 的行格式：`[ro.product.model]: [sdk_gphone64_x86_64]`。 */
private val SYSTEM_PROPERTY_ROW = Regex("""^\[(.+)]: \[(.*)]$""")

/** 解析 `getprop` 的全部属性。 */
internal fun parseSystemProperties(output: String): Map<String, String> = output.lineSequence().mapNotNull { row ->
    val match = SYSTEM_PROPERTY_ROW.matchEntire(row.trim()) ?: return@mapNotNull null
    match.groupValues[1] to match.groupValues[2]
}.toMap()

/** 电池状态。 */
internal data class BatteryState(
    /** 电量百分比。 */
    val levelPercent: Int,
    /** 状态码（`BatteryManager.BATTERY_STATUS_*`）。 */
    val status: Int,
    /** 温度（摄氏度）。 */
    val temperatureCelsius: Double,
)

/**
 * 解析 `dumpsys battery`。
 *
 * 字段是 `level: 100`、`status: 2`、`temperature: 250`；温度单位是 0.1 摄氏度。
 */
internal fun parseBatteryState(output: String): BatteryState {
    val level = batteryField(output, "level")?.toIntOrNull()
        ?: error("电池 dump 里读不到 level：$output")
    val status = batteryField(output, "status")?.toIntOrNull()
        ?: error("电池 dump 里读不到 status：$output")
    val temperature = batteryField(output, "temperature")?.toIntOrNull()
        ?: error("电池 dump 里读不到 temperature：$output")
    return BatteryState(level, status, temperature / 10.0)
}

/** 取 `dumpsys battery` 的某个字段值。 */
private fun batteryField(output: String, name: String): String? = output.lineSequence()
    .map { it.trim() }
    .firstOrNull { it.startsWith("$name:") }
    ?.substringAfter("$name:")
    ?.trim()

/** 解析 `dumpsys thermalservice` 的热状态。 */
internal fun parseThermalStatus(output: String): Int = output.lineSequence()
    .map { it.trim() }
    .firstOrNull { it.startsWith("Thermal Status:") }
    ?.substringAfter("Thermal Status:")
    ?.trim()
    ?.toIntOrNull()
    ?: error("热状态 dump 里读不到 Thermal Status：$output")

/** 活动显示的渲染帧率（`mActiveRenderFrameRate=60.000004`）。 */
private const val ACTIVE_REFRESH_RATE_PREFIX = "mActiveRenderFrameRate="

/** 显示设备信息里的帧率（`renderFrameRate 60.000004`），活动帧率缺失时兜底。 */
private val DISPLAY_DEVICE_REFRESH_RATE = Regex("""renderFrameRate ([\d.]+)""")

/**
 * 解析 `dumpsys display` 的活动刷新率。
 *
 * 取活动显示的渲染帧率；某些机型没有该字段，退回显示设备信息里的帧率。
 */
internal fun parseRefreshRateHz(output: String): Double {
    val active = output.lineSequence()
        .map { it.trim() }
        .firstOrNull { it.startsWith(ACTIVE_REFRESH_RATE_PREFIX) }
        ?.substringAfter(ACTIVE_REFRESH_RATE_PREFIX)
        ?.toDoubleOrNull()
    if (active != null) return active
    val deviceInfo = output.lineSequence()
        .firstOrNull { it.contains("DisplayDeviceInfo{") }
        ?.let { row -> DISPLAY_DEVICE_REFRESH_RATE.find(row)?.groupValues?.get(1)?.toDoubleOrNull() }
    return deviceInfo ?: error("显示 dump 里读不到刷新率：$output")
}

/** 被测应用的版本标识。 */
internal data class PackageVersion(
    /** 版本号（versionCode）。 */
    val versionCode: Long,
    /** 版本名（versionName）。 */
    val versionName: String,
)

/**
 * 解析 `dumpsys package <包名>` 的版本标识。
 *
 * 取 `versionCode=2000 minSdk=31 targetSdk=28` 与 `versionName=0.1.0` 两行。
 */
internal fun parsePackageVersion(output: String): PackageVersion {
    val versionCode = Regex("""versionCode=(\d+)""").find(output)?.groupValues?.get(1)?.toLongOrNull()
        ?: error("包 dump 里读不到 versionCode：$output")
    val versionName = Regex("""versionName=(\S+)""").findAll(output)
        .map { it.groupValues[1] }
        .firstOrNull { it != "null" }
        ?: error("包 dump 里读不到 versionName：$output")
    return PackageVersion(versionCode, versionName)
}

/**
 * 读出设备与被测应用的环境快照。
 *
 * 读不到任何一项都当场失败：环境缺失时留下的数字没法与别的轮次比较，宁可判红
 * 也不能记一份看着齐全、实则缺项的环境。
 */
internal fun readDeviceEnvironment(device: UiDevice): DeviceEnvironment {
    val properties = parseSystemProperties(device.executeShellCommand("getprop"))
    val model = properties["ro.product.model"]
        ?: error("getprop 里读不到 ro.product.model")
    val romBuildId = properties["ro.build.display.id"]
        ?: error("getprop 里读不到 ro.build.display.id")
    val apiLevel = properties["ro.build.version.sdk"]?.toIntOrNull()
        ?: error("getprop 里读不到 ro.build.version.sdk")
    val battery = parseBatteryState(device.executeShellCommand("dumpsys battery"))
    val thermalStatus = parseThermalStatus(device.executeShellCommand("dumpsys thermalservice"))
    val refreshRateHz = parseRefreshRateHz(device.executeShellCommand("dumpsys display"))
    val version = parsePackageVersion(device.executeShellCommand("dumpsys package $TARGET_PACKAGE"))
    return DeviceEnvironment(
        model = model,
        romBuildId = romBuildId,
        apiLevel = apiLevel,
        refreshRateHz = refreshRateHz,
        batteryLevelPercent = battery.levelPercent,
        batteryStatus = battery.status,
        batteryTemperatureCelsius = battery.temperatureCelsius,
        thermalStatus = thermalStatus,
        appVersionCode = version.versionCode,
        appVersionName = version.versionName,
        capturedAtEpochMillis = System.currentTimeMillis(),
    )
}
