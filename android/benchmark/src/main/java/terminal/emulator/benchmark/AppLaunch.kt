package terminal.emulator.benchmark

import androidx.test.uiautomator.UiDevice

/**
 * 一次启动的系统侧结果（`am start -W` 口径）。
 *
 * 用系统口径而非各应用自己的启动指标：宏基准指标量的是各目标自身管线的耗时，跨应用
 * 不可比；`am start -W` 的 TotalTime 与被测应用无关，是唯一能直接横向比的口径。
 */
internal data class AppLaunchResult(
    /** 启动状态（`COLD`／`WARM`／`HOT`）。 */
    val launchState: String,
    /** 真正被拉起的界面（可能是别的应用接管了这次启动）。 */
    val activity: String,
    /** 启动总耗时（系统侧毫秒）。 */
    val totalTimeMillis: Long,
    /** 等待首帧的耗时（系统侧毫秒）。 */
    val waitTimeMillis: Long,
)

/** `am start -W` 的状态行。 */
private const val LAUNCH_STATUS_PREFIX = "Status: "

/** `am start -W` 的启动态行。 */
private const val LAUNCH_STATE_PREFIX = "LaunchState: "

/** `am start -W` 的实际界面行。 */
private const val LAUNCH_ACTIVITY_PREFIX = "Activity: "

/** `am start -W` 的总耗时行。 */
private const val LAUNCH_TOTAL_TIME_PREFIX = "TotalTime: "

/** `am start -W` 的等待耗时行。 */
private const val LAUNCH_WAIT_TIME_PREFIX = "WaitTime: "

/**
 * 解析 `am start -W` 的输出。
 *
 * 状态不是 `ok` 即判失败：启动没成功时后面的耗时字段没有意义，读成 0 会把「没启动」
 * 记成「启动很快」。
 */
internal fun parseLaunchResult(output: String): AppLaunchResult {
    val status = output.lineSequence()
        .map { it.trim() }
        .firstOrNull { it.startsWith(LAUNCH_STATUS_PREFIX) }
        ?.substringAfter(LAUNCH_STATUS_PREFIX)
        ?: error("启动输出里没有状态行：$output")
    check(status == "ok") { "启动未成功（状态 $status）：$output" }
    val launchState = output.lineSequence()
        .map { it.trim() }
        .firstOrNull { it.startsWith(LAUNCH_STATE_PREFIX) }
        ?.substringAfter(LAUNCH_STATE_PREFIX)
        ?: error("启动输出里没有启动态行：$output")
    val activity = output.lineSequence()
        .map { it.trim() }
        .firstOrNull { it.startsWith(LAUNCH_ACTIVITY_PREFIX) }
        ?.substringAfter(LAUNCH_ACTIVITY_PREFIX)
        ?: error("启动输出里没有界面行：$output")
    val totalTimeMillis = output.lineSequence()
        .map { it.trim() }
        .firstOrNull { it.startsWith(LAUNCH_TOTAL_TIME_PREFIX) }
        ?.substringAfter(LAUNCH_TOTAL_TIME_PREFIX)
        ?.toLongOrNull()
        ?: error("启动输出里没有总耗时行：$output")
    val waitTimeMillis = output.lineSequence()
        .map { it.trim() }
        .firstOrNull { it.startsWith(LAUNCH_WAIT_TIME_PREFIX) }
        ?.substringAfter(LAUNCH_WAIT_TIME_PREFIX)
        ?.toLongOrNull()
        ?: error("启动输出里没有等待耗时行：$output")
    return AppLaunchResult(launchState, activity, totalTimeMillis, waitTimeMillis)
}

/**
 * 解析 `cmd package resolve-activity --brief` 的输出，取启动组件。
 *
 * 输出首行是匹配信息，第二行才是 `包名/类名`；取形如该格式的行，找不到即判失败——
 * 没有启动入口的应用没法测冷启动。
 */
internal fun parseLauncherActivity(output: String): String = output.lineSequence()
    .map { it.trim() }
    .firstOrNull { row -> LAUNCHER_ACTIVITY_ROW.matchEntire(row) != null }
    ?: error("解析不到启动 Activity：$output")

/** 启动组件的行格式：`包名/类名`。 */
private val LAUNCHER_ACTIVITY_ROW = Regex("""[\w.]+/[\w.$]+""")

/** 读出指定包的启动组件；解析到的组件不属于该包即判失败（量到的不是被测应用的启动）。 */
internal fun readLauncherActivity(device: UiDevice, packageName: String): String {
    val component = parseLauncherActivity(
        device.executeShellCommand("cmd package resolve-activity --brief $packageName"),
    )
    check(component.startsWith("$packageName/")) {
        "解析到的启动 Activity 不属于 $packageName：$component"
    }
    return component
}

/**
 * 强停指定包：冷启动的准备动作。
 *
 * 强停会留下一条 `USER REQUESTED / FORCE STOP` 的退出记录，故它必须发生在测量窗口
 * 之外——那是被测应用被准备掉，不是被测行为。
 */
internal fun forceStop(device: UiDevice, packageName: String) {
    device.executeShellCommand("am force-stop $packageName")
    device.waitForIdle()
}

/** `dumpsys package` 里的运行时权限段标题。 */
private const val RUNTIME_PERMISSIONS_HEADER = "runtime permissions:"

/** 运行时权限行：`android.permission.XXX: granted=true|false, flags=[...]`。 */
private val RUNTIME_PERMISSION_ROW = Regex("""^(android\.permission\.[A-Z_]+): granted=(true|false)""")

/**
 * 解析包 dump 里的运行时权限及其授权状态。
 *
 * 只认 `runtime permissions:` 段：该段之后的第一行非权限行即段落结束。
 */
internal fun parseRuntimePermissions(output: String): Map<String, Boolean> {
    val permissions = linkedMapOf<String, Boolean>()
    var insideRuntimeSection = false
    for (row in output.lineSequence()) {
        val trimmed = row.trim()
        if (trimmed == RUNTIME_PERMISSIONS_HEADER) {
            insideRuntimeSection = true
            continue
        }
        if (!insideRuntimeSection) continue
        val match = RUNTIME_PERMISSION_ROW.find(trimmed)
        if (match == null) {
            insideRuntimeSection = false
            continue
        }
        permissions[match.groupValues[1]] = match.groupValues[2] == "true"
    }
    return permissions
}

/**
 * 预授运行时权限：权限弹窗会抢走被测应用的启动。
 *
 * 系统侧 `am start -W` 报的是弹窗自己的 Activity（实测：被测应用首次启动时系统弹出
 * 通知权限窗，报出的 Activity 是权限控制器的），量到的就不是被测应用的启动耗时。
 *
 * 通知权限单独显式授一次：目标 SDK 低于 33 的应用在安装时已被系统自动授予该权限，
 * 但首次建通知渠道时系统仍会弹窗，显式授予才会让系统记住用户的选择。
 *
 * 返回授完仍未授权的权限：这些授不了（受限权限等），由调用方记进测量行；若它们
 * 真的挡住了启动，跨应用基准会当场判红并报出抢走启动的 Activity。
 */
internal fun grantRuntimePermissions(device: UiDevice, packageName: String): List<String> {
    val runtimePermissions = parseRuntimePermissions(device.executeShellCommand("dumpsys package $packageName"))
    for (permission in runtimePermissions.filterValues { !it }.keys) {
        device.executeShellCommand("pm grant $packageName $permission")
    }
    device.executeShellCommand("pm grant $packageName $NOTIFICATION_PERMISSION")
    return parseRuntimePermissions(device.executeShellCommand("dumpsys package $packageName"))
        .filterValues { !it }
        .keys
        .toList()
}

/** 按启动组件拉起并读系统侧口径的启动结果。 */
internal fun startAndWait(device: UiDevice, launcherActivity: String): AppLaunchResult =
    parseLaunchResult(device.executeShellCommand("am start -W -n $launcherActivity"))

/**
 * 分位数（最近秩法）：第 `ceil(比例 × 样本数)` 小的样本。
 *
 * 样本数少时 p95 即最大值，这是小样本下的实情，不做插值假装精度。
 */
internal fun percentile(values: List<Long>, ratio: Double): Long {
    require(values.isNotEmpty()) { "分位数没有样本" }
    val sorted = values.sorted()
    val rank = Math.ceil(ratio * sorted.size).toInt().coerceIn(1, sorted.size)
    return sorted[rank - 1]
}
