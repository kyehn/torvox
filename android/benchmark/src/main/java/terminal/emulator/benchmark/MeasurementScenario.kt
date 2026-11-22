package terminal.emulator.benchmark

import android.os.SystemClock
import androidx.test.uiautomator.UiDevice

/**
 * 跑一段测量场景：先记环境，再判退出，最后落一行记录。
 *
 * 一次测量要留下三样东西：环境快照（事后判断两轮数字是不是同一环境下的）、进程退出
 * 归因（进程死了的话测得的数字全部作废，退出原因决定这算应用故障还是环境抖动）、以及
 * 一行可机读记录（JSONL 行文件里的场景、环境、退出与判定）。
 *
 * 场景返回该轮的指标（没有指标的场次返回空表）；失败时指标作废、判定记失败并把失败
 * 抛出，失败信息里带上进程退出归因。
 *
 * [note] 是本轮不进判定的补充说明（如被测应用的环境准备情况），与失败原因一同落进
 * 测量行。
 *
 * 准备动作（强停等）走 [setup]：它在测量窗口之外执行，故它自身的副作用（强停留下
 * 的退出记录）不会被当成被测行为算进本轮。
 *
 * [measured] 标出该行是否计入结论：宏基准框架的温／热启动准备轮次不是测量，跨应用
 * 对比没有框架轮次，每一行都是测量。
 *
 * 宿主负载在场景前后各取一次 CPU 时间快照，落在场景窗口旁：本轮慢是应用慢还是宿主
 * 被抢占，就看这两次快照之间的 steal 占比。
 */
internal fun measureScenario(
    device: UiDevice,
    label: String,
    store: MeasurementStore,
    frameworkIteration: Int?,
    packageName: String = TARGET_PACKAGE,
    measured: Boolean = frameworkIteration != null,
    note: String = "",
    setup: () -> Unit = {},
    scenario: (iteration: Int) -> Map<String, Double>,
) {
    val environment = readDeviceEnvironment(device)
    val app = readAppIdentity(device, packageName)
    emitMeasured("[bench-env] $label ${environment.describe()} ${app.describe()}")
    val iteration = store.nextIteration(label)
    val preparedAtMillis = System.currentTimeMillis()
    var startedAtMillis = preparedAtMillis
    var endedAtMillis = preparedAtMillis
    var failure: Throwable? = null
    var metrics: Map<String, Double> = emptyMap()
    var hostLoad: HostLoad? = null
    try {
        setup()
        val cpuBefore = readCpuTime(device)
        val hostLoadStartedAt = SystemClock.elapsedRealtime()
        startedAtMillis = System.currentTimeMillis()
        try {
            metrics = scenario(iteration)
        } catch (thrown: Throwable) {
            failure = thrown.withProcessExitAttribution(device, preparedAtMillis, packageName)
        } finally {
            endedAtMillis = System.currentTimeMillis()
            try {
                hostLoad = hostLoadBetween(
                    cpuBefore,
                    readCpuTime(device),
                    SystemClock.elapsedRealtime() - hostLoadStartedAt,
                )
            } catch (collectionFailure: Throwable) {
                // 负载读不出来也 MUST NOT 吃掉原始失败：读失败挂成附加信息，判定仍记失败。
                if (failure == null) {
                    failure = collectionFailure
                } else {
                    failure.addSuppressed(collectionFailure)
                }
            }
        }
    } catch (thrown: Throwable) {
        failure = thrown.withProcessExitAttribution(device, preparedAtMillis, packageName)
    }
    val exits = try {
        readExitInfoRecords(device, packageName).withinWindow(startedAtMillis, endedAtMillis)
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
    hostLoad?.let { emitMeasured("[bench-load] $label iteration=$iteration ${it.describe()}") }
    store.appendRow(
        MeasurementRow(
            runId = store.runId,
            label = label,
            iteration = iteration,
            frameworkIteration = frameworkIteration,
            measured = measured,
            startedAtMillis = startedAtMillis,
            endedAtMillis = endedAtMillis,
            environment = environment,
            app = app,
            metrics = metrics,
            hostLoad = hostLoad,
            exits = exits,
            artifacts = emptyList(),
            verdict = if (failure == null && exits.isEmpty()) "PASS" else "FAIL",
            note = listOfNotNull(note.takeIf { it.isNotEmpty() }, failure?.message).joinToString("；"),
        ).toJson(),
    )
    failure?.let { throw it }
    check(exits.isEmpty()) {
        "$label 测量窗口内进程退出 ${exits.size} 次，测量结果作废：" +
            exits.joinToString("；") { it.describe() }
    }
}
