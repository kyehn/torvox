package terminal.emulator.runtime

import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * 启动尺寸的守卫：只拒绝非正尺寸。
 *
 * 此前按整块显示尺寸推导像素下界（宽度按等宽字形宽高比、高度按行高折半），分屏、
 * 自由窗口与折叠半屏这些合法的小窗口一律被判成「等正确的 surfaceChanged」——
 * 会话建不出来，界面停在空白，而日志里只有一条 warn。
 */
class StartSurfaceSizeTest {

    @Test
    fun `split screen and free window sizes are legal start sizes`() {
        assertTrue("分屏冷启动的窗口必须能建出会话", isStartSizeUsable(540, 1200))
        assertTrue("折叠半屏的窗口必须能建出会话", isStartSizeUsable(320, 240))
        assertTrue("一像素见方的窗口也必须能建出会话", isStartSizeUsable(1, 1))
    }

    @Test
    fun `non positive size is rejected`() {
        assertFalse("零宽必须拒绝", isStartSizeUsable(0, 1200))
        assertFalse("零高必须拒绝", isStartSizeUsable(540, 0))
        assertFalse("负尺寸必须拒绝", isStartSizeUsable(-540, -1200))
    }
}
