package terminal.emulator.benchmark

import androidx.test.platform.app.InstrumentationRegistry
import androidx.test.uiautomator.UiDevice

/** 被测目标：包名 + 启动组件，由仪器参数解析。 */
internal data class ScenarioTarget(
    val packageName: String,
    val launcherActivity: String,
)

/** 仪器参数 `targetPackage`，默认 `com.termux`。 */
internal fun readTargetPackage(): String =
    InstrumentationRegistry.getArguments().getString("targetPackage")?.trim()?.takeIf { it.isNotEmpty() }
        ?: TARGET_PACKAGE

/** 仪器参数 `targetPackages`（逗号分隔），默认 `com.termux`。 */
internal fun readTargetPackages(): List<String> {
    val raw = InstrumentationRegistry.getArguments().getString("targetPackages")
    val parsed = raw?.split(',')?.map { it.trim() }?.filter { it.isNotEmpty() }
    return if (parsed.isNullOrEmpty()) listOf(TARGET_PACKAGE) else parsed
}

/** 从设备上解析被测目标的启动组件。 */
internal fun readScenarioTarget(device: UiDevice): ScenarioTarget {
    val packageName = readTargetPackage()
    val launcherActivity = readLauncherActivity(device, packageName)
    return ScenarioTarget(packageName, launcherActivity)
}
