package terminal.emulator

import androidx.test.platform.app.InstrumentationRegistry
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith
import org.junit.runners.JUnit4
import terminal.emulator.bridge.Bridge
import terminal.emulator.bridge.FontInfo
import terminal.emulator.bridge.NativeBridge
import terminal.emulator.bridge.Shell
import terminal.emulator.bridge.TerminalConfig
import terminal.emulator.util.runCatchingCancellable
import java.io.File

/**
 * DESIGN 字体选择节：`~/.termux/font.ttf` 存在即设为主字体（ttc/otf 同理），
 * 覆盖 TESTING.md「复制字体文件到 font.ttf 后字体被正确设置，检查字形」。
 *
 * 源字体取模拟器镜像内的系统字体文件（运行期复制，不 vendor 进仓）。字形像素
 * 判定需宿主 OCR（`TESTING.md` 环境节的 rapidocr），该自动化尚未实现、登记于
 * `openspec/specs/upstream-alignment` 的禁改区待维护者裁定项台账，因此此处判据为
 * 解析出的族名、等宽标记翻转与单元格度量变化——三者同时成立即证明主字体
 * 确实来自该文件而非回落 notdef。
 */
@RunWith(JUnit4::class)
class FontFileOverrideInstrumentedTest : TerminalLogcatTest() {
    @Test
    fun termux_font_file_becomes_primary_font() {
        val context = InstrumentationRegistry.getInstrumentation().targetContext
        val home = File(context.filesDir, "font-override-home").apply { mkdirs() }
        val fontFile = File(File(home, ".termux").apply { mkdirs() }, "font.ttf")
        File("/system/fonts/Roboto-Regular.ttf").copyTo(fontFile, overwrite = true)

        val bridge =
            Bridge(
                TerminalConfig(
                    shell = Shell.SystemDefault,
                    themeName = "",
                    home = home.absolutePath,
                    workingDirectory = home.absolutePath,
                    prefix = "",
                    mkshrcPath = "",
                    fontSizeTenths = 140,
                ),
            )
        val sessionId = bridge.spawnTerminal(24, 80, "/system/bin/sh")
        assertTrue("会话创建失败", sessionId != 0L)
        try {
            NativeBridge.switchSession(sessionId)
            // 基线 MUST 在 `loadFontFile` **之前**取：该导出当场就把面换上并重建管线
            // （`rebuild_font_pipeline` + `set_font_family`），之后再取拿到的已经是
            // font.ttf 的字体——「等宽翻转为假」「单元格度量变化」两条判据会恒假，
            // 用例永远红（CI 上正是这条先炸的）。
            val default =
                checkNotNull(FontInfo.fromPacked(checkNotNull(NativeBridge.getFontInfo())))
            val family = NativeBridge.loadFontFile(sessionId, fontFile.absolutePath)
            assertNotNull("用户字体文件必须可解析出族名", family)

            // setFontFamily 的入参不重要：font.ttf 存在时探测路径优先应用该文件。
            val applied = bridge.setFontFamily("")
            assertEquals("探测路径必须成功应用 font.ttf", true, applied)

            val info = FontInfo.fromPacked(checkNotNull(NativeBridge.getFontInfo()))
            assertEquals("上报的主字体必须是 font.ttf 的族", family, info.activeName)
            assertTrue(
                "Roboto 为比例字体：主字体切换后等宽标记必须翻转为假（默认=$default，切换后=$info）",
                default.activeMonospaced && !info.activeMonospaced,
            )
            assertTrue(
                "单元格度量必须来自 font.ttf（Roboto）而非默认等宽字体",
                info.cellWidthPx != default.cellWidthPx ||
                    info.cellHeightPx != default.cellHeightPx,
            )

            NativeBridge.feedTerminal(sessionId, "AB\r\n".toByteArray())
            val gridded =
                UxTestUtils.pollUntilTrue(timeoutMs = 15_000, intervalMs = 100) {
                    NativeBridge.getTerminalText(sessionId)?.contains("AB") == true
                }
            assertNotNull("主字体生效后写入的字符必须落格", gridded)
        } finally {
            runCatchingCancellable { bridge.close() }
            fontFile.delete()
        }
    }
}
