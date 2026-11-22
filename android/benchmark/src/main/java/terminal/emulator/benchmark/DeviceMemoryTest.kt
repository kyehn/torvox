package terminal.emulator.benchmark

import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * 内存汇总行的解析契约。
 *
 * 期望值取自设备实测的 `dumpsys meminfo` 字面量（2026-11-22 本机 AVD，API 35 模拟器）。
 */
class DeviceMemoryTest {
    @Test
    fun parsesSummaryTotals() {
        val memory = parseDeviceMemory(MEMINFO_DUMP)
        assertEquals(59520, memory.pssKilobytes)
        assertEquals(163660, memory.rssKilobytes)
    }

    @Test
    fun rejectsDumpWithoutSummary() {
        val failure = runCatching { parseDeviceMemory("Applications Memory Usage (in Kilobytes):\n") }
        assertTrue(failure.isFailure)
    }

    private companion object {
        val MEMINFO_DUMP = """
            Applications Memory Usage (in Kilobytes):
            Uptime: 5190413 Realtime: 5190413

            ** MEMINFO in pid 11492 [com.termux] **
                           Pss  Private  Private  SwapPss      Rss     Heap     Heap     Heap
                         Total    Dirty    Clean    Dirty    Total     Size    Alloc     Free
                        ------   ------   ------   ------   ------   ------   ------   ------
              Native Heap    20457    20424        0       79    21388    58940    52386     2224
              Dalvik Heap     8116     3784     4252       37     9916     9495     4748     4747
             Dalvik Other     2920     2704       24        5     3604
                    Stack      688      688        0        0      692
                   Ashmem       19        0        0        0      560
                 Other dev       20        4       16        0      328
                   .so mmap    11638      740     8548       29    41040
                   .jar mmap     1736        0      116        0    36120
                   .apk mmap     6972        0     6872        0     8256
                   .ttf mmap      295        0      244        0      620
                   .dex mmap      299        4      276        0     1120
                   .oat mmap      545        0        0        0    11648
                   .art mmap     3137      576     1772      188    23220
                 Other mmap     1788        4      916        0     4180
                    Unknown      552      544        0        0      968
                      TOTAL    59520    29472    23036      338   163660    68435    57134     6971

             App Summary
                               Pss(KB)                        Rss(KB)
                                ------                         ------
                   Java Heap:     6132                          33136
                 Native Heap:    20424                          21388
                        Code:    16800                          99168
                       Stack:      688                            692
                    Graphics:        0                              0
               Private Other:     8464
                      System:     7012
                     Unknown:                                    9276

                   TOTAL PSS:    59520            TOTAL RSS:   163660       TOTAL SWAP PSS:      338

             Objects
                       Views:       10         ViewRootImpl:        1
        """.trimIndent()
    }
}
