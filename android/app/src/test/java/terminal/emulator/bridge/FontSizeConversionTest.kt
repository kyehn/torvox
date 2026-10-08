package terminal.emulator.bridge

import org.junit.Assert.assertEquals
import org.junit.Test

class FontSizeConversionTest {
    @Test
    fun `sp to px scales with the sp-to-px coefficient`() {
        assertEquals(14f, fontSpToPx(14f, spToPxScale = 1f))
        assertEquals(38.5f, fontSpToPx(14f, spToPxScale = 2.75f))
        assertEquals(0f, fontSpToPx(0f, spToPxScale = 3f))
    }

    @Test
    fun `sp to px includes the system font scaling`() {
        // 系数为「显示密度 × 系统字体缩放」：字形实际光栅尺度就是 sp 乘该系数，
        // 只乘显示密度会报出小于真实渲染的像素值（fontScale=1.3 时即差 30%）。
        val density = 2.625f
        val fontScale = 1.3f
        assertEquals(
            94f * density * fontScale,
            fontSpToPx(94f, spToPxScale = density * fontScale),
            0.001f,
        )
    }
}
