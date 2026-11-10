package terminal.emulator

import android.app.Activity
import android.content.Intent
import androidx.compose.ui.test.onAllNodesWithTag
import androidx.test.platform.app.InstrumentationRegistry
import androidx.test.runner.lifecycle.ActivityLifecycleMonitorRegistry
import androidx.test.runner.lifecycle.Stage
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertSame
import org.junit.Test

/**
 * 主界面必须单实例。
 *
 * 终端的原生渲染器全局唯一（一份交换链、一份会话表），第二个主界面实例会与
 * 第一个争抢同一套进程级状态：先退到后台的实例领着「切后台暂停」，后启动的
 * 实例一恢复就把那次暂停还掉，渲染线程随即在已被系统回收的 BufferQueue 上
 * 取帧（`ERROR_SURFACE_LOST_KHR`），返回后终端永久黑屏——表现为「新会话概率
 * 无响应，切换会话后刷新」。
 *
 * 用例走真实投递路径：再次启动主界面（带 `FLAG_ACTIVITY_NEW_TASK`），断言系统
 * 复用既有实例并把新 Intent 投递给它，而不是新建一个实例。
 */
class MainActivitySingleInstanceTest : ComposeTerminalLogcatTest() {

    @Test
    fun relaunchingReusesTheRunningInstance() {
        val first = composeTestRule.activity
        val context = InstrumentationRegistry.getInstrumentation().targetContext
        context.startActivity(
            Intent(context, MainActivity::class.java)
                .addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
                .putExtra(MainActivity.EXTRA_OPEN_SETTINGS, true),
        )
        // 设置覆盖层打开即证明新 Intent 被投递到了既有实例（onNewIntent）。
        val delivered =
            UxTestUtils.pollUntilTrue(timeoutMs = 20_000, intervalMs = 200) {
                composeTestRule
                    .onAllNodesWithTag("SettingsScreen", useUnmergedTree = true)
                    .fetchSemanticsNodes()
                    .isNotEmpty()
            }
        assertNotNull("第二次启动的 Intent 必须投递到既有实例", delivered)

        // 生命周期监视器只允许在主线程上查询。
        val resumed = mutableListOf<Activity>()
        InstrumentationRegistry.getInstrumentation().runOnMainSync {
            resumed.addAll(
                ActivityLifecycleMonitorRegistry.getInstance().getActivitiesInStage(Stage.RESUMED),
            )
        }
        assertEquals("再次启动不得产生第二个主界面实例", 1, resumed.size)
        assertSame("再次启动必须复用既有实例", first, resumed.single())
    }
}
