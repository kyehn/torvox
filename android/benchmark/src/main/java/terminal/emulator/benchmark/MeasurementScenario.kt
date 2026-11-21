package terminal.emulator.benchmark

import androidx.test.uiautomator.UiDevice

/**
 * 跑一段测量场景：先记环境，再判退出。
 *
 * 一次测量要留下两样东西：环境快照（事后判断两轮数字是不是同一环境下的）与进程
 * 退出归因（进程死了的话测得的数字全部作废，退出原因决定这算应用故障还是环境抖动）。
 */
internal fun measureScenario(device: UiDevice, label: String, scenario: () -> Unit) {
    val environment = readDeviceEnvironment(device)
    emitMeasured("[bench-env] $label ${environment.describe()}")
    val startMillis = System.currentTimeMillis()
    try {
        scenario()
    } catch (failure: Throwable) {
        throw failure.withProcessExitAttribution(device, startMillis)
    }
    val exits = readExitInfoRecords(device).withinWindow(startMillis, System.currentTimeMillis())
    emitMeasured("[bench-measure] $label-process-exits count=${exits.size} causes=${exits.map { it.cause }}")
    check(exits.isEmpty()) {
        "$label 测量窗口内进程退出 ${exits.size} 次，测量结果作废：" +
            exits.joinToString("；") { it.describe() }
    }
}
