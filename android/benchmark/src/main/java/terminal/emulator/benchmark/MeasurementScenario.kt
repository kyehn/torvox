package terminal.emulator.benchmark

import androidx.test.uiautomator.UiDevice

/**
 * 跑一段测量场景：先记环境，再判退出，最后落一行记录。
 *
 * 一次测量要留下三样东西：环境快照（事后判断两轮数字是不是同一环境下的）、进程退出
 * 归因（进程死了的话测得的数字全部作废，退出原因决定这算应用故障还是环境抖动）、以及
 * 一行可机读记录（JSONL 行文件里的场景、环境、退出与判定）。
 */
internal fun measureScenario(
    device: UiDevice,
    label: String,
    store: MeasurementStore,
    frameworkIteration: Int?,
    scenario: () -> Unit,
) {
    val environment = readDeviceEnvironment(device)
    emitMeasured("[bench-env] $label ${environment.describe()}")
    val iteration = store.nextIteration(label)
    val startedAtMillis = System.currentTimeMillis()
    var failure: Throwable? = null
    try {
        scenario()
    } catch (thrown: Throwable) {
        failure = thrown.withProcessExitAttribution(device, startedAtMillis)
    }
    val endedAtMillis = System.currentTimeMillis()
    val exits = try {
        readExitInfoRecords(device).withinWindow(startedAtMillis, endedAtMillis)
    } catch (collectionFailure: Throwable) {
        // 归因读不出来也 MUST NOT 吃掉原始失败：读失败挂成附加信息，判定仍记失败。
        if (failure == null) {
            failure = collectionFailure
        } else {
            failure.addSuppressed(collectionFailure)
        }
        emptyList()
    }
    emitMeasured("[bench-measure] $label-process-exits count=${exits.size} causes=${exits.map { it.cause }}")
    store.appendRow(
        MeasurementRow(
            runId = store.runId,
            label = label,
            iteration = iteration,
            frameworkIteration = frameworkIteration,
            startedAtMillis = startedAtMillis,
            endedAtMillis = endedAtMillis,
            environment = environment,
            exits = exits,
            artifacts = emptyList(),
            verdict = if (failure == null && exits.isEmpty()) "PASS" else "FAIL",
            note = failure?.message.orEmpty(),
        ).toJson(),
    )
    failure?.let { throw it }
    check(exits.isEmpty()) {
        "$label 测量窗口内进程退出 ${exits.size} 次，测量结果作废：" +
            exits.joinToString("；") { it.describe() }
    }
}
