package terminal.emulator.benchmark

import org.junit.Assert.assertEquals
import org.junit.Test

/**
 * 持续输出负载的判据：命令文本与字节记账。
 *
 * 字节记账错一个字节，吞吐与「输出字节上限」的判定就跟着错；命令文本错一个字符，
 * 负载根本没跑起来而呈现帧数仍可能不为零。故两者都按字面量断言。
 */
class SustainedOutputLoadTest {
    @Test
    fun buildsCommandWithMarkerPerBatch() {
        val load = SustainedOutputLoad(batchCount = 3, linesPerBatch = 2, pauseSeconds = "0.1")

        assertEquals(
            "for i in 1 2 3; do seq 1 2; sleep 0.1; log -t s5out __s5b\$i; done",
            load.command(),
        )
    }

    @Test
    fun escapesSpacesForInputText() {
        val load = SustainedOutputLoad(batchCount = 2, linesPerBatch = 1, pauseSeconds = "0.1")

        assertEquals(
            "for%si%sin%s1%s2;%sdo%sseq%s1%s1;%ssleep%s0.1;%slog%s-t%ss5out%s__s5b\$i;%sdone",
            load.inputTextArgument(),
        )
    }

    @Test
    fun countsBytesWithCarriageReturnExpansion() {
        val load = SustainedOutputLoad(batchCount = 1, linesPerBatch = 12, pauseSeconds = "0.1")

        // 1 到 9 各 1 字节，10 到 12 各 2 字节，正文共 15 字节；12 个换行各展开成 CRLF。
        assertEquals(15L + 24L, load.bytesPerBatch())
    }

    @Test
    fun rejectsNonPositiveShape() {
        runCatching { SustainedOutputLoad(batchCount = 0, linesPerBatch = 1, pauseSeconds = "0.1") }
            .onSuccess { throw AssertionError("批次数为零的负载 MUST 被拒绝") }
        runCatching { SustainedOutputLoad(batchCount = 1, linesPerBatch = 0, pauseSeconds = "0.1") }
            .onSuccess { throw AssertionError("每批行数为零的负载 MUST 被拒绝") }
    }
}
