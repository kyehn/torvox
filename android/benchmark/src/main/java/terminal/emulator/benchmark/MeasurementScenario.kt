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
 * 抛出，失败信息里带上进程退出归因与宿主负载证据。
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
 *
 * 场景矩阵项 [scenario] 给出该轮的 wall-clock 上限：窗口结束后超限即终止场景
 * （强停被测应用）并记「超限终止」，不继续等待。应用内部回归项不属于跨应用矩阵，
 * 传空。
 */
internal fun measureScenario(
    device: UiDevice,
    label: String,
    store: MeasurementStore,
    frameworkIteration: Int?,
    scenario: Scenario? = null,
    packageName: String = TARGET_PACKAGE,
    measured: Boolean = frameworkIteration != null,
    note: String = "",
    expectProcessExit: Boolean = false,
    setup: () -> Unit = {},
    scenarioBody: (iteration: Int, budget: ScenarioBudget?) -> Map<String, Double>,
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
        val budget = scenario?.let { ScenarioBudget(it) }
        startedAtMillis = System.currentTimeMillis()
        try {
            metrics = scenarioBody(iteration, budget)
        } catch (thrown: Throwable) {
            failure = thrown.withProcessExitAttribution(device, preparedAtMillis, packageName)
        } finally {
            endedAtMillis = System.currentTimeMillis()
            try {
                budget?.checkDeadline()
            } catch (limitExceeded: ScenarioLimitExceeded) {
                // 超限即终止场景：强停被测应用，不继续消耗设备。
                forceStop(device, packageName)
                if (failure == null) {
                    failure = limitExceeded
                } else {
                    failure.addSuppressed(limitExceeded)
                }
            }
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
            scenarioId = scenario?.id,
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
            verdict = scenarioVerdict(failure, exits, expectProcessExit),
            note = listOfNotNull(note.takeIf { it.isNotEmpty() }, failure?.message).joinToString("；"),
        ).toJson(),
    )
    // 落盘之后才抛：判定按原始失败类型分类，不受归因包装影响。
    // 窗口内进程退出本身就是测量失败的证据（判定已记 FAIL），一并带上归因抛出。
    val exitFailure = when {
        exits.isNotEmpty() && !expectProcessExit ->
            IllegalStateException(
                "$label 测量窗口内进程退出 ${exits.size} 次，测量结果作废：" +
                    exits.joinToString("；") { it.describe() },
            )

        exits.any { it.cause == ProcessExitCause.UNKNOWN } ->
            IllegalStateException(
                "$label 测量窗口内进程以未知原因退出：" +
                    exits.joinToString("；") { it.describe() },
            )

        else -> null
    }
    val thrown = failure ?: exitFailure
    // 宿主负载证据随失败一起抛出：本轮慢是应用慢还是宿主被抢占，看 steal 占比。
    // 负载是证据不是免责——失败照旧失败，只是消息里多出归因依据。
    thrown?.let { throw it.withHostLoadAttribution(hostLoad) }
}

/**
 * 场景判定：按失败类型与进程退出记录分类。
 *
 * 失败可能被归因包装（进程退出、宿主负载），故「超限终止」沿 cause 链识别原始
 * [ScenarioLimitExceeded]；只认最外层类型会把超限误记成 FAIL。
 */
internal fun scenarioVerdict(failure: Throwable?, exits: List<ExitInfoRecord>, expectProcessExit: Boolean): String =
    when {
        failure?.hasCause<ScenarioLimitExceeded>() == true -> "超限终止"
        failure != null -> "FAIL"
        exits.any { it.cause == ProcessExitCause.UNKNOWN } -> "FAIL"
        exits.isNotEmpty() && !expectProcessExit -> "FAIL"
        else -> "PASS"
    }

/** cause 链上是否有 [T] 类型的失败。 */
private inline fun <reified T : Throwable> Throwable.hasCause(): Boolean =
    generateSequence(this) { it.cause }.any { it is T }
