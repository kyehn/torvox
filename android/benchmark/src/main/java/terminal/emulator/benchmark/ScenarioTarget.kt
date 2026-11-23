package terminal.emulator.benchmark

import androidx.test.platform.app.InstrumentationRegistry
import androidx.test.uiautomator.UiDevice

/** 被测目标：包名 + 启动组件，由仪器参数解析。 */
internal data class ScenarioTarget(val packageName: String, val launcherActivity: String)

/** 仪器参数 `targetPackage`，默认 `com.termux`。 */
internal fun readTargetPackage(): String =
    parseTargetPackage(InstrumentationRegistry.getArguments().getString("targetPackage"))

/** 仪器参数 `targetPackage` 的取值：空值与空白都退回默认包名。 */
internal fun parseTargetPackage(raw: String?): String = raw?.trim()?.takeIf { it.isNotEmpty() } ?: TARGET_PACKAGE

/** 仪器参数 `targetPackages`（逗号分隔），默认 `com.termux`。 */
internal fun readTargetPackages(): List<String> =
    parseTargetPackages(InstrumentationRegistry.getArguments().getString("targetPackages"))

/** 仪器参数 `targetPackages` 的取值：逐项去空白，空项丢弃，整体为空时退回默认包名。 */
internal fun parseTargetPackages(raw: String?): List<String> {
    val parsed = raw?.split(',')?.map { it.trim() }?.filter { it.isNotEmpty() }
    return if (parsed.isNullOrEmpty()) listOf(TARGET_PACKAGE) else parsed
}

/** 从设备上解析被测目标的启动组件。 */
internal fun readScenarioTarget(device: UiDevice): ScenarioTarget {
    val packageName = readTargetPackage()
    val launcherActivity = readLauncherActivity(device, packageName)
    return ScenarioTarget(packageName, launcherActivity)
}
