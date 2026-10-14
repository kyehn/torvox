package terminal.emulator.ui

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test

/**
 * 组字光标锚点的落格（[composingCursorMarker]）。
 *
 * 锚点 MUST 落在终端光标格：输入法据此把候选窗口贴到真实光标。视口外的光标
 * MUST NOT 回报——候选窗口会落在屏幕外。另两条准入（输入法正在请求监视、当前
 * 有组字）由调用方在查询前判定，不在本函数的参数里。
 */
class ComposingCursorAnchorTest {

    @Test
    fun `marker lands in the terminal cursor cell`() {
        val marker =
            composingCursorMarker(
                cursorRow = 3,
                cursorCol = 4,
                rows = 24,
                cellWidth = 10f,
                cellHeight = 20f,
            )
        requireNotNull(marker)
        assertEquals("插入标记落在第 4 列左沿", 40f, marker.horizontal, 0.01f)
        assertEquals("插入标记落在第 3 行顶部", 60f, marker.top, 0.01f)
        assertEquals("标记下沿即单元格下沿", 80f, marker.bottom, 0.01f)
        assertEquals(marker.bottom, marker.baseline, 0f)
    }

    @Test
    fun `cursor outside the viewport reports nothing`() {
        // 光标滚出视口时候选窗口会落在屏幕外，回报没有意义。
        assertNull(
            composingCursorMarker(
                cursorRow = -1,
                cursorCol = 4,
                rows = 24,
                cellWidth = 10f,
                cellHeight = 20f,
            ),
        )
        assertNull(
            composingCursorMarker(
                cursorRow = 24,
                cursorCol = 4,
                rows = 24,
                cellWidth = 10f,
                cellHeight = 20f,
            ),
        )
        assertNull(
            composingCursorMarker(
                cursorRow = 3,
                cursorCol = -1,
                rows = 24,
                cellWidth = 10f,
                cellHeight = 20f,
            ),
        )
    }

    @Test
    fun `last visible row still reports`() {
        val marker =
            composingCursorMarker(
                cursorRow = 23,
                cursorCol = 0,
                rows = 24,
                cellWidth = 10f,
                cellHeight = 20f,
            )
        requireNotNull(marker)
        assertEquals(460f, marker.top, 0.01f)
    }
}
