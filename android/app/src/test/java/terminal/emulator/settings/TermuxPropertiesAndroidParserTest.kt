package terminal.emulator.settings

import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config

/**
 * `termux.properties` 语法层在 **Android 运行时** 上的行为。
 *
 * 解析委托给 `java.util.Properties`，而 Android 的 libcore 与宿主 JDK 是两份实现
 * （分隔符的空白集合、注释行的处理都不同）。宿主 JVM 上的用例覆盖不到设备上的那条
 * 路径，故这里用 Robolectric 跑一遍 Android 自己的 `java.util.Properties`。
 *
 * 判别力：重复键留痕依赖 `Properties.load` 逐条调用 `put`——若 Android 侧不调用，
 * 重复键就查不出来，本类第一条用例会失败。
 */
@RunWith(RobolectricTestRunner::class)
@Config(sdk = [34])
class TermuxPropertiesAndroidParserTest {

    private fun parse(text: String): TermuxPropertiesLoad = TermuxPropertiesParser.parse(text)

    @Test
    fun `duplicate keys are still reported on android`() {
        val loaded = parse("fullscreen = false\nfullscreen = true\n")
        assertTrue(loaded.properties.fullscreen)
        assertTrue(loaded.errors.any { it.contains("duplicate") })
    }

    @Test
    fun `a whitespace separated pair is read as key and value`() {
        assertEquals(100, parse("terminal-transcript-rows 100").properties.terminalTranscriptRows)
    }

    @Test
    fun `a comment ending with a backslash does not swallow the next line`() {
        val loaded = parse("# 路径以 \\ 结尾\nfullscreen = true\n")
        assertTrue(loaded.properties.fullscreen)
    }

    @Test
    fun `a malformed unicode escape only affects its own value`() {
        val loaded = parse("default-working-directory = \\u12\nfullscreen = true\n")
        assertTrue(loaded.properties.fullscreen)
        assertEquals("u12", loaded.properties.defaultWorkingDirectory)
    }

    @Test
    fun `a non ascii value survives parsing`() {
        assertEquals(
            "/sdcard/我的目录",
            parse("default-working-directory = /sdcard/我的目录").properties.defaultWorkingDirectory,
        )
    }
}
