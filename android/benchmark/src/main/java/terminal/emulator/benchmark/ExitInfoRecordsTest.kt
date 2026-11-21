package terminal.emulator.benchmark

import android.app.ApplicationExitInfo
import org.junit.Assert.assertEquals
import org.junit.Assert.assertThrows
import org.junit.Assert.assertTrue
import org.junit.Test
import java.text.SimpleDateFormat
import java.util.Date

/**
 * `dumpsys activity exit-info` 输出的解析判据。
 *
 * 样本取自模拟器上被测应用的真实退出历史（API 35 模拟器，`am force-stop` 与 `am crash`
 * 两条路径），断言用字面量而非重新计算的期望值，格式漂移时即判红。
 */
class ExitInfoRecordsTest {
    @Test
    fun parsesExitRecords() {
        val records = parseExitInfoRecords(EXIT_INFO_OUTPUT_SAMPLE)

        assertEquals(2, records.size)
        val forced = records[0]
        assertEquals(8219, forced.pid)
        assertEquals(10, forced.reasonCode)
        assertEquals("USER REQUESTED", forced.reasonName)
        assertEquals(21, forced.subReasonCode)
        assertEquals("FORCE STOP", forced.subReasonName)
        assertEquals("[FORCE STOP] stop com.termux due to from pid 8260", forced.description)
        assertEquals("0.00", forced.pss)
        assertEquals("0.00", forced.rss)
    }

    @Test
    fun parsesCrashRecord() {
        val crash = parseExitInfoRecords(EXIT_INFO_CRASH_OUTPUT_SAMPLE).single()

        assertEquals(8151, crash.pid)
        assertEquals(4, crash.reasonCode)
        assertEquals("APP CRASH(EXCEPTION)", crash.reasonName)
        assertEquals(0, crash.subReasonCode)
        assertEquals("UNKNOWN", crash.subReasonName)
        assertEquals("crash", crash.description)
    }

    @Test
    fun readsWallClockTimestamp() {
        val records = parseExitInfoRecords(EXIT_INFO_OUTPUT_SAMPLE)

        // 退出时刻是设备本地墙钟：按同一格式回打即原始文本。
        val format = SimpleDateFormat("yyyy-MM-dd HH:mm:ss.SSS")
        assertEquals("2026-11-21 16:08:00.838", format.format(Date(records[0].timestampMillis)))
        assertEquals("2026-11-21 16:07:49.934", format.format(Date(records[1].timestampMillis)))
        assertEquals(10_904L, records[0].timestampMillis - records[1].timestampMillis)
    }

    @Test
    fun classifiesExitCauses() {
        val records = parseExitInfoRecords(EXIT_INFO_OUTPUT_SAMPLE)

        assertEquals(ProcessExitCause.EXPECTED, records[0].cause)
        assertEquals(ProcessExitCause.APPLICATION_FAULT, records[1].cause)
        assertEquals(ProcessExitCause.APPLICATION_FAULT, exitCauseOf(ApplicationExitInfo.REASON_CRASH))
        assertEquals(
            ProcessExitCause.APPLICATION_FAULT,
            exitCauseOf(ApplicationExitInfo.REASON_CRASH_NATIVE),
        )
        assertEquals(ProcessExitCause.APPLICATION_FAULT, exitCauseOf(ApplicationExitInfo.REASON_ANR))
        assertEquals(ProcessExitCause.EXPECTED, exitCauseOf(ApplicationExitInfo.REASON_USER_REQUESTED))
        assertEquals(ProcessExitCause.ENVIRONMENT, exitCauseOf(ApplicationExitInfo.REASON_LOW_MEMORY))
        assertEquals(ProcessExitCause.ENVIRONMENT, exitCauseOf(ApplicationExitInfo.REASON_SIGNALED))
        // 平台新增的原因码不在已知集合内时归未知，判红交人查，不猜语义。
        assertEquals(ProcessExitCause.UNKNOWN, exitCauseOf(17))
    }

    @Test
    fun selectsRecordsWithinScenarioWindow() {
        val records = parseExitInfoRecords(EXIT_INFO_OUTPUT_SAMPLE)
        val newest = records[0].timestampMillis

        assertEquals(listOf(8219), records.withinWindow(newest - 1_000L, newest + 1_000L).map { it.pid })
        assertEquals(2, records.withinWindow(newest - 20_000L, newest).size)
        assertTrue(records.withinWindow(newest + 1_000L, newest + 20_000L).isEmpty())
    }

    @Test
    fun acceptsPackageWithoutExitHistory() {
        assertTrue(parseExitInfoRecords(EXIT_INFO_EMPTY_OUTPUT_SAMPLE).isEmpty())
    }

    @Test
    fun rejectsOutputThatIsNotExitInfoDump() {
        val failure = assertThrows(IllegalStateException::class.java) {
            parseExitInfoRecords("Unknown command: exit-info\n")
        }

        assertTrue(failure.message.orEmpty().contains("dumpsys activity exit-info"))
    }

    @Test
    fun rejectsRecordWithoutReasonRow() {
        val failure = assertThrows(IllegalStateException::class.java) {
            parseExitInfoRecords(
                """
                ACTIVITY MANAGER PROCESS EXIT INFO (dumpsys activity exit-info)
                  package: com.termux
                        ApplicationExitInfo #0:
                          timestamp=2026-11-21 16:08:00.838 pid=8219 realUid=10213 packageUid=10213 definingUid=10213 user=0
                          importance=100 pss=0.00 rss=0.00 description=crash state=empty trace=null
                """.trimIndent(),
            )
        }

        assertTrue(failure.message.orEmpty().contains("缺少原因行"))
    }

    private companion object {
        /** 模拟器上强停与崩溃两条记录的真实 dump。 */
        val EXIT_INFO_OUTPUT_SAMPLE =
            """
            ACTIVITY MANAGER PROCESS EXIT INFO (dumpsys activity exit-info)
            Last Timestamp of Persistence Into Persistent Storage: 2026-11-21 15:37:55.266
              package: com.termux
                Historical Process Exit for uid=10213
                    ApplicationExitInfo #0:
                      timestamp=2026-11-21 16:08:00.838 pid=8219 realUid=10213 packageUid=10213 definingUid=10213 user=0
                      process=com.termux reason=10 (USER REQUESTED) subreason=21 (FORCE STOP) status=0
                      importance=100 pss=0.00 rss=0.00 description=[FORCE STOP] stop com.termux due to from pid 8260 state=empty trace=null
                    ApplicationExitInfo #1:
                      timestamp=2026-11-21 16:07:49.934 pid=8151 realUid=10213 packageUid=10213 definingUid=10213 user=0
                      process=com.termux reason=4 (APP CRASH(EXCEPTION)) subreason=0 (UNKNOWN) status=0
                      importance=100 pss=0.00 rss=0.00 description=crash state=empty trace=null
            """.trimIndent()

        /** 模拟器上崩溃一条记录的真实 dump。 */
        val EXIT_INFO_CRASH_OUTPUT_SAMPLE =
            """
            ACTIVITY MANAGER PROCESS EXIT INFO (dumpsys activity exit-info)
            Last Timestamp of Persistence Into Persistent Storage: 2026-11-21 15:37:55.266
              package: com.termux
                Historical Process Exit for uid=10213
                    ApplicationExitInfo #0:
                      timestamp=2026-11-21 16:07:49.934 pid=8151 realUid=10213 packageUid=10213 definingUid=10213 user=0
                      process=com.termux reason=4 (APP CRASH(EXCEPTION)) subreason=0 (UNKNOWN) status=0
                      importance=100 pss=0.00 rss=0.00 description=crash state=empty trace=null
            """.trimIndent()

        /** 从未启动过的包的真实 dump：只有 dump 头。 */
        val EXIT_INFO_EMPTY_OUTPUT_SAMPLE =
            """
            ACTIVITY MANAGER PROCESS EXIT INFO (dumpsys activity exit-info)
            Last Timestamp of Persistence Into Persistent Storage: 2026-11-21 16:15:07.165
            """.trimIndent()
    }
}
