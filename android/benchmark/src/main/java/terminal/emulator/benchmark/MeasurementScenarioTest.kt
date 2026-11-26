package terminal.emulator.benchmark

import android.app.ApplicationExitInfo
import org.junit.Assert.assertEquals
import org.junit.Test

/**
 * 场景判定口径：失败可能被归因包装，分类必须沿 cause 链认原始失败类型。
 *
 * 这里不跑测量，只钉住落盘 `verdict` 的分类规则：超限终止不得因包装改写。
 */
class MeasurementScenarioTest {
    private fun exit(reasonCode: Int): ExitInfoRecord = ExitInfoRecord(
        timestampMillis = 1L,
        pid = 1,
        reasonCode = reasonCode,
        reasonName = "TEST",
        subReasonCode = 0,
        subReasonName = "",
        description = "",
        pss = "0.00",
        rss = "0.00",
    )

    @Test
    fun wrappedLimitExceededKeepsLimitVerdict() {
        val failure =
            IllegalStateException(
                "场景 S5 退出归因包装",
                ScenarioLimitExceeded("场景 S5 已耗用 30001ms，超过上限 30000ms：超限终止"),
            )

        assertEquals("超限终止", scenarioVerdict(failure, emptyList(), expectProcessExit = false))
    }

    @Test
    fun doublyWrappedLimitExceededKeepsLimitVerdict() {
        val failure =
            IllegalStateException(
                "宿主负载包装",
                IllegalStateException(
                    "进程退出包装",
                    ScenarioLimitExceeded("场景 S5 超限终止"),
                ),
            )

        assertEquals("超限终止", scenarioVerdict(failure, emptyList(), expectProcessExit = false))
    }

    @Test
    fun plainFailureIsFail() {
        val failure = IllegalStateException("滚动呈现帧数 22 低于下限 30")

        assertEquals("FAIL", scenarioVerdict(failure, emptyList(), expectProcessExit = false))
    }

    @Test
    fun unknownExitCauseFails() {
        assertEquals(
            "FAIL",
            scenarioVerdict(null, listOf(exit(reasonCode = -1)), expectProcessExit = true),
        )
    }

    @Test
    fun unexpectedExitFails() {
        assertEquals(
            "FAIL",
            scenarioVerdict(
                null,
                listOf(exit(reasonCode = ApplicationExitInfo.REASON_CRASH)),
                expectProcessExit = false,
            ),
        )
    }

    @Test
    fun expectedExitPasses() {
        assertEquals(
            "PASS",
            scenarioVerdict(
                null,
                listOf(exit(reasonCode = ApplicationExitInfo.REASON_EXIT_SELF)),
                expectProcessExit = true,
            ),
        )
    }

    @Test
    fun cleanRunPasses() {
        assertEquals("PASS", scenarioVerdict(null, emptyList(), expectProcessExit = false))
    }
}
