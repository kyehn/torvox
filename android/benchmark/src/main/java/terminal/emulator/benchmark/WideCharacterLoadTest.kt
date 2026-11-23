package terminal.emulator.benchmark

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * 宽字符负载的判据：命令文本与字节记账。
 *
 * 宽字符不能直接敲进终端（硬件键盘事件通道按 KeyCharacterMap 映射字符，非 ASCII 映射
 * 不到），故负载把宽字符写成 UTF-8 字节的八进制转义。八进制转义写错一个字节，终端
 * 收到的就不是那个字符；字节记账错一行，「输出字节上限」的判定与落盘的字节数就跟着错。
 */
class WideCharacterLoadTest {
    @Test
    fun escapesWideCharactersAsOctal() {
        val load = WideCharacterLoad(lineCount = 1, canary = "__s8tail")

        val command = load.command()
        assertTrue(command.contains("\\344\\275\\240\\345\\245\\275\\344\\270\\226\\347\\225\\214"))
        assertTrue(command.contains("\\145\\314\\201"))
        assertTrue(command.contains("\\360\\237\\230\\200"))
        assertTrue(command.contains("\\360\\237\\221\\250\\342\\200\\215\\360\\237\\221\\251\\342\\200\\215\\360\\237\\221\\247"))
        assertTrue(command.contains("\\357\\274\\241\\357\\274\\242\\357\\274\\243"))
    }

    @Test
    fun commandContainsOnlyAscii() {
        val load = WideCharacterLoad(lineCount = 1, canary = "__s8tail")

        val command = load.command()
        assertTrue(command.all { it.code < 128 })
    }

    @Test
    fun escapesSpacesForInputText() {
        val load = WideCharacterLoad(lineCount = 2, canary = "__s8tail")

        val argument = load.inputTextArgument()
        assertFalse(argument.contains(' '))
        assertTrue(argument.contains("%s"))
    }

    @Test
    fun countsBytesWithCarriageReturnExpansion() {
        val load = WideCharacterLoad(lineCount = 1, canary = "__s8tail")

        // 五个模板合计 46 字节；五行换行各展开成 CRLF；标题哨兵是 ESC ] 2 ; <哨兵> BEL，
        // 哨兵 8 字节加 5 字节定长部分。
        assertEquals(46L + 10L + 13L, load.bytes())
    }

    @Test
    fun countsBytesAcrossRounds() {
        val load = WideCharacterLoad(lineCount = 1000, canary = "__s8tail")

        assertEquals(46_000L + 10_000L + 13L, load.bytes())
    }

    @Test
    fun rejectsNonPositiveShape() {
        runCatching { WideCharacterLoad(lineCount = 0, canary = "__s8tail") }
            .onSuccess { throw AssertionError("轮数为零的负载 MUST 被拒绝") }
        runCatching { WideCharacterLoad(lineCount = 1, canary = "") }
            .onSuccess { throw AssertionError("空标题哨兵的负载 MUST 被拒绝") }
    }
}
