package terminal.emulator.benchmark

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertThrows
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * 启动口径解析的判据：系统侧 `am start -W` 与 `resolve-activity` 的输出。
 *
 * 断言用设备实测输出作字面量，不拿被测实现算出的值反过来当期望。
 */
class AppLaunchTest {
    @Test
    fun parsesLaunchOutput() {
        val result = parseLaunchResult(LAUNCH_OUTPUT_SAMPLE)

        assertEquals("COLD", result.launchState)
        assertEquals("com.termux/terminal.emulator.MainActivity", result.activity)
        assertEquals(363L, result.totalTimeMillis)
        assertEquals(370L, result.waitTimeMillis)
    }

    @Test
    fun rejectsLaunchOutputWithoutStatusRow() {
        val failure = assertThrows(IllegalStateException::class.java) {
            parseLaunchResult("Starting: Intent { act=android.intent.action.MAIN }\n")
        }

        assertTrue(failure.message.orEmpty().contains("状态行"))
    }

    @Test
    fun rejectsLaunchOutputWithFailedStatus() {
        val failure = assertThrows(IllegalStateException::class.java) {
            parseLaunchResult("Status: timeout\nLaunchState: COLD\n")
        }

        assertTrue(failure.message.orEmpty().contains("启动未成功"))
    }

    @Test
    fun rejectsLaunchOutputWithoutTotalTimeRow() {
        val failure = assertThrows(IllegalStateException::class.java) {
            parseLaunchResult(
                "Status: ok\nLaunchState: COLD\n" +
                    "Activity: com.termux/terminal.emulator.MainActivity\nWaitTime: 370\n",
            )
        }

        assertTrue(failure.message.orEmpty().contains("总耗时行"))
    }

    @Test
    fun parsesLauncherActivityFromResolveOutput() {
        val component = parseLauncherActivity(RESOLVE_OUTPUT_SAMPLE)

        assertEquals("com.termux/terminal.emulator.MainActivity", component)
    }

    @Test
    fun rejectsResolveOutputWithoutActivity() {
        val failure = assertThrows(IllegalStateException::class.java) {
            parseLauncherActivity("No activity found\n")
        }

        assertTrue(failure.message.orEmpty().contains("解析不到启动 Activity"))
    }

    @Test
    fun parsesRuntimePermissionStates() {
        val permissions = parseRuntimePermissions(PACKAGE_OUTPUT_SAMPLE)

        assertEquals(2, permissions.size)
        assertEquals(false, permissions["android.permission.POST_NOTIFICATIONS"])
        assertEquals(true, permissions["android.permission.ACCESS_FINE_LOCATION"])
    }

    @Test
    fun ignoresPermissionsOutsideRuntimeSection() {
        val permissions = parseRuntimePermissions(PACKAGE_OUTPUT_SAMPLE)

        assertFalse(permissions.containsKey("android.permission.INTERNET"))
    }

    @Test
    fun computesPercentileByNearestRank() {
        val values = listOf(420L, 310L, 360L, 290L, 380L)

        assertEquals(360L, percentile(values, 0.5))
        assertEquals(420L, percentile(values, 0.95))
        assertEquals(290L, percentile(values, 0.0))
    }

    @Test
    fun computesPercentileOfSingleSample() {
        assertEquals(363L, percentile(listOf(363L), 0.95))
    }

    @Test
    fun rejectsPercentileWithoutSamples() {
        val failure = assertThrows(IllegalArgumentException::class.java) {
            percentile(emptyList(), 0.5)
        }

        assertTrue(failure.message.orEmpty().contains("没有样本"))
    }

    private companion object {
        /** 真实 `am start -W` 输出（模拟器实测）。 */
        val LAUNCH_OUTPUT_SAMPLE =
            """
            Starting: Intent { act=android.intent.action.MAIN cat=[android.intent.category.LAUNCHER] }
            Status: ok
            LaunchState: COLD
            Activity: com.termux/terminal.emulator.MainActivity
            TotalTime: 363
            WaitTime: 370
            Complete
            """.trimIndent()

        /** 真实 `cmd package resolve-activity --brief` 输出（首行是匹配信息）。 */
        val RESOLVE_OUTPUT_SAMPLE =
            """
            priority=0 preferredOrder=0 match=0x108000 specificIndex=-1 isDefault=false
            com.termux/terminal.emulator.MainActivity
            """.trimIndent()

        /** 真实 `dumpsys package` 输出（裁剪到权限相关的段）。 */
        val PACKAGE_OUTPUT_SAMPLE =
            """
            requested permissions:
              android.permission.POST_NOTIFICATIONS
              android.permission.INTERNET
              android.permission.ACCESS_FINE_LOCATION
            runtime permissions:
              android.permission.POST_NOTIFICATIONS: granted=false, flags=[ USER_SENSITIVE_WHEN_GRANTED]
              android.permission.ACCESS_FINE_LOCATION: granted=true, flags=[ USER_SENSITIVE_WHEN_GRANTED]

            Queries:
            """.trimIndent()
    }
}
