package terminal.emulator.benchmark

import org.junit.Assert.assertEquals
import org.junit.Test

/**
 * 突发输出负载的判据：命令文本与字节记账。
 *
 * 字节记账错一个字节，「输出字节上限」的判定与落盘的字节数就跟着错；命令文本错一个
 * 字符，负载根本没跑起来而界面照旧在画，帧数不为零也证明不了字节流完整。故两者都
 * 按字面量断言。
 */
class BurstOutputLoadTest {
    @Test
    fun buildsCommandWithCanaryAndMarker() {
        val load = BurstOutputLoad(lineCount = 12, canary = "__s6tail")

        assertEquals(
            "seq 1 12; printf '\\033]2;__s6tail\\007'; log -t s6out __s6done",
            load.command(),
        )
    }

    @Test
    fun escapesSpacesForInputText() {
        val load = BurstOutputLoad(lineCount = 12, canary = "__s6tail")

        assertEquals(
            "seq%s1%s12;%sprintf%s'\\033]2;__s6tail\\007';%slog%s-t%ss6out%s__s6done",
            load.inputTextArgument(),
        )
    }

    @Test
    fun countsBytesWithCarriageReturnExpansion() {
        val load = BurstOutputLoad(lineCount = 12, canary = "__s6tail")

        // 1 到 9 各 1 字节，10 到 12 各 2 字节，正文共 15 字节；12 个换行各展开成
        // CRLF；标题哨兵是 ESC ] 2 ; <哨兵> BEL，哨兵 8 字节加 5 字节定长部分。
        assertEquals(15L + 24L + 13L, load.bytes())
    }

    @Test
    fun countsBytesAcrossDigitWidths() {
        val load = BurstOutputLoad(lineCount = 100, canary = "c")

        // 1 到 9 各 1 字节，10 到 99 各 2 字节，100 一行 3 字节：正文 9 + 180 + 3。
        assertEquals(192L + 200L + 6L, load.bytes())
    }

    @Test
    fun rejectsNonPositiveShape() {
        runCatching { BurstOutputLoad(lineCount = 0, canary = "__s6tail") }
            .onSuccess { throw AssertionError("行数为零的负载 MUST 被拒绝") }
        runCatching { BurstOutputLoad(lineCount = 1, canary = "") }
            .onSuccess { throw AssertionError("空标题哨兵的负载 MUST 被拒绝") }
    }
}
