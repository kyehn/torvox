package terminal.emulator

import androidx.test.platform.app.InstrumentationRegistry
import org.junit.rules.TestRule
import org.junit.runner.Description
import org.junit.runners.model.Statement

/**
 * 用例失败时把应用自己的日志尾部附在失败信息上。
 *
 * 为什么需要它：仪器化失败在 CI 上只留下一行断言（例如「标记未落格」），
 * 而屏幕无墨迹、shell 无回显、surface 判死这三类根因，日志里各有一行明确
 * 锚点。CI 步骤不导出 logcat（工作流是保护文件），于是失败信息与原因之间
 * 没有任何可追的线索，只能靠人本地复现——每次都要重跑十几分钟的套件。
 * 把日志尾部接在断言上，失败信息本身就是诊断材料。
 *
 * 只抓与终端相关的标签，且限制条数：整份 logcat 有噪声，且会淹没真正的锚点。
 */
class TerminalLogcatRule : TestRule {
    override fun apply(base: Statement, description: Description): Statement = object : Statement() {
        override fun evaluate() {
            try {
                base.evaluate()
            } catch (failure: Throwable) {
                throw AssertionError(
                    "${failure.message}${System.lineSeparator()}" +
                        logcatTail(),
                    failure,
                )
            }
        }
    }

    private fun logcatTail(): String {
        val dump =
            try {
                // 读取必须在 use 块内完成：`use` 在块返回时就关闭描述符，
                // 块外再读同一个 fd 只会拿到 EBADF（模拟器上实测 100% 失败，
                // 失败信息退化成「logcat 抓取失败」，恰好丢掉本规则的全部价值）。
                InstrumentationRegistry.getInstrumentation()
                    .uiAutomation
                    .executeShellCommand("logcat -d -t $DUMP_LINES -v brief")
                    .use { descriptor ->
                        android.os.ParcelFileDescriptor.AutoCloseInputStream(descriptor)
                            .bufferedReader()
                            .readText()
                    }
            } catch (error: Exception) {
                return "（logcat 抓取失败：$error）"
            }
        val relevant =
            dump.lineSequence()
                .filter { line -> WATCHED_TAGS.any { line.contains(it) } }
                .toList()
                .takeLast(LOG_LINES)
        if (relevant.isEmpty()) return "（logcat 中没有终端相关日志行）"
        return buildString {
            appendLine("--- 终端相关日志尾部 ---")
            relevant.forEach(::appendLine)
        }
    }

    private companion object {
        /** 附在失败信息里的行数上限：足够覆盖一次失败前后的关键锚点，又不至于淹没断言信息。 */
        const val LOG_LINES = 120

        /**
         * 过滤之前先取的 logcat 尾部行数：设备日志被系统与应用自身噪声淹没，
         * 只取最后一百多行再按标签过滤，应用自己的锚点会整段落空（CI 实测失败
         * 信息多为「logcat 中没有终端相关日志行」），本规则等于白装。
         */
        const val DUMP_LINES = 4000

        /**
         * 过滤用的标签清单。必须写 **logcat 里的实际标签**：`-v brief` 的格式是
         * `级别/标签 ( PID): 消息`，而 `native::android::ffi` 这类模块标签全小写，
         * 按大写 `FFI` 匹配只能命中消息前缀恰为 `FFI: ` 的那几行——于是
         * `render: frame failed: …`（屏幕无墨迹的唯一锚点）与
         * `surface invalidated after …`（surface 判死的唯一锚点）被整段过滤掉，
         * 失败报告里只剩滚动与帧计数噪声。
         */
        val WATCHED_TAGS =
            listOf(
                "Runtime",
                "TerminalSurface",
                "TerminalViewModel",
                "ClipboardAccess",
                "SessionBridgeCloser",
                "native::android::ffi",
                "native::render::context",
                "native::terminal::ghostty_terminal",
            )
    }
}
