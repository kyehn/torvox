package terminal.emulator.ui

import org.junit.Assert.assertTrue
import org.junit.Assert.fail
import org.junit.Test
import java.io.File

/**
 * `_fontInfo` 通道的取值契约：只装原生回传的字体信息 JSON，或空串（原生尚未上报）。
 *
 * 曾经往该通道写本地化文案（`R.string.no_font_loaded`）当「没数据」的哨兵。那串文本
 * 下游必过 `FontInfoDto.fromJson`，解析必然失败并落到兜底分支，于是「没数据」在界面上
 * 与「加载失败」同形，而维护者看代码还以为错误被如实呈现了。
 *
 * 这里只检缺陷签名（往该通道塞本地化字符串），不做泛化的代码形状扫描——后者会把
 * 经局部变量转手的合法赋值一并打成违规，测出来的是形状而不是契约。
 */
class FontInfoChannelContractTest {
    @Test
    fun `font info state is never assigned a localized message`() {
        val source = File("../app/src/main/java/terminal/emulator/TerminalViewModel.kt")
        assertTrue("找不到 TerminalViewModel.kt：${source.absolutePath}", source.isFile)

        val offenders =
            source
                .readLines()
                .withIndex()
                .filter { (_, line) -> "_fontInfo.value =" in line }
                .filter { (_, line) -> "getString(" in line || "R.string." in line }
                .map { (number, line) -> "${number + 1}: ${line.trim()}" }

        if (offenders.isNotEmpty()) {
            fail(
                "_fontInfo.value 只接受原生 getFontInfo() 的结果或其空串（orEmpty()），" +
                    "写入本地化文案会被当成 JSON 解析失败、把「没数据」伪装成「加载失败」：\n" +
                    offenders.joinToString("\n"),
            )
        }
    }
}
