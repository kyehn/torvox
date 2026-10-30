package terminal.emulator.ui

import android.util.Log
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Test
import terminal.emulator.ComposeTerminalLogcatTest
import terminal.emulator.UxTestUtils
import terminal.emulator.awaitBridge
import terminal.emulator.awaitShellPrompt
import terminal.emulator.getBridge
import terminal.emulator.waitForSession
import java.io.File

/**
 * 会话标题（OSC 0/2）必须由 shell 侧上报，否则抽屉里永远只有序号。
 *
 * ## 曾经的现象
 *
 * 自带标题上报的 shell 在抽屉里有标题，而 `/system/bin/sh`（Android 的
 * mksh）、`bash` 等会话没有——用户据此以为「标题只支持部分 shell」。真实原因
 * 不在终端侧：标题来自 shell 发出的 OSC 0/2，而 mksh 与 bash **自己都不发**
 * （bash 只有用户自己设了 `PROMPT_COMMAND` 才发）。
 *
 * 终端侧能做的只有两件事：给 mksh 的提示符挂上标题序列（应用写的 rc 文件），
 * 以及给 bash 注入 `PROMPT_COMMAND`（唯一不碰用户 rc 文件的做法）。
 *
 * ## 量具
 *
 * 会话的**真实标题**（VT 收到的 OSC 0/2），经桥接查询读回——不是界面上的
 * 占位文案，也不是内部状态。
 */
class SessionTitleInstrumentedTest : ComposeTerminalLogcatTest() {

    private companion object {
        /** 标题上报的上限（毫秒）：标题由提示符画出时发出，落在首个提示符之后。 */
        const val TITLE_TIMEOUT_MS = 20_000L

        /** 轮询间隔（毫秒）。 */
        const val POLL_INTERVAL_MS = 250L
    }

    @Before
    fun setUp() {
        composeTestRule.waitForSession()
    }

    @Test
    fun mksh_session_reports_a_title_from_the_prompt() {
        val bridge = composeTestRule.awaitBridge()
        awaitShellPrompt(bridge)

        // 标题由提示符发出：先等到提示符，再等标题落地。
        var title = ""
        val reported =
            UxTestUtils.pollUntilTrue(timeoutMs = TITLE_TIMEOUT_MS, intervalMs = POLL_INTERVAL_MS) {
                title = bridge.getActiveSessionTitle()
                title.isNotEmpty()
            }
        assertNotNull(
            "mksh 会话在 ${TITLE_TIMEOUT_MS}ms 内没有上报任何标题（抽屉里只有序号）",
            reported,
        )
        // 标题内容是当前目录名（提示符里的 ${PWD##*/}）。
        val home = File(composeTestRule.activity.filesDir, "home")
        assertTrue(
            "标题应为当前目录名（${home.name}），实际：$title",
            title == home.name,
        )
        Log.i("UX_METRIC", "session_title=$title")
    }

    /**
     * bash 会话也必须上报标题。
     *
     * bash 自己不发 OSC 0/2：只有用户自己设了 `PROMPT_COMMAND` 才发。终端侧
     * 唯一不碰用户 rc 文件的做法是**经环境变量**注入 `PROMPT_COMMAND`（bash 启动
     * 时若 shell 内未定义该变量，就用环境里的值）——见 `pty::title_report_hook`。
     *
     * 本用例覆盖那条注入路径：装好 prefix 后新建的会话跑的就是 prefix 里的 bash
     * （会话 shell 由 prefix 决定，见 `TerminalRuntime.buildConfig`），标题必须
     * 来自注入的 `PROMPT_COMMAND`。
     */
    @Test
    fun bash_session_reports_a_title_from_the_prompt() {
        val activity = composeTestRule.activity
        val bash = File(activity.filesDir, "usr/bin/bash")
        assertTrue(
            "本用例的前提：prefix 里的 bash 必须存在（waitForSession 已装 prefix）",
            bash.isFile,
        )
        val oldBridge = composeTestRule.awaitBridge()
        activity.terminalViewModel.createSession()
        // 新会话建好后成为活动会话：活动桥的对象随之更换。
        val switched =
            UxTestUtils.pollUntilTrue(timeoutMs = TITLE_TIMEOUT_MS, intervalMs = POLL_INTERVAL_MS) {
                composeTestRule.getBridge()?.let { it !== oldBridge } == true
            }
        assertNotNull("新建会话后活动会话必须切换过去", switched)
        val bridge = checkNotNull(composeTestRule.getBridge())
        // 标题由提示符发出：等 bash 的首个提示符把 OSC 0 送进 VT。
        var title = ""
        val reported =
            UxTestUtils.pollUntilTrue(timeoutMs = TITLE_TIMEOUT_MS, intervalMs = POLL_INTERVAL_MS) {
                title = bridge.getActiveSessionTitle()
                title.isNotEmpty()
            }
        assertNotNull(
            "bash 会话在 ${TITLE_TIMEOUT_MS}ms 内没有上报任何标题（抽屉里只有序号）",
            reported,
        )
        val home = File(activity.filesDir, "home")
        assertTrue(
            "标题应为当前目录名（${home.name}），实际：$title",
            title == home.name,
        )
        Log.i("UX_METRIC", "bash_session_title=$title")
    }
}
