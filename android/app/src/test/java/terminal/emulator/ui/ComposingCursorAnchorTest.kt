package terminal.emulator.ui

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test

/**
 * 组字光标锚点的准入与落格（[composingCursorMarker]）。
 *
 * 锚点 MUST 落在终端光标格：输入法据此把候选窗口贴到真实光标。准入两条，
 * 缺一即不回报——无组字时给一个「光标位置」会让候选窗口贴到不属于它的格子，
 * 未请求监视时回报是一条无人读取的 IPC。
 */
class ComposingCursorAnchorTest {

    @Test
    fun `marker lands in the terminal cursor cell`() {
        val marker =
            composingCursorMarker(
                monitoring = true,
                composing = true,
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
    fun `no monitoring request reports nothing`() {
        assertNull(
            composingCursorMarker(
                monitoring = false,
                composing = true,
                cursorRow = 3,
                cursorCol = 4,
                rows = 24,
                cellWidth = 10f,
                cellHeight = 20f,
            ),
        )
    }

    @Test
    fun `no composition reports nothing`() {
        assertNull(
            composingCursorMarker(
                monitoring = true,
                composing = false,
                cursorRow = 3,
                cursorCol = 4,
                rows = 24,
                cellWidth = 10f,
                cellHeight = 20f,
            ),
        )
    }

    @Test
    fun `cursor outside the viewport reports nothing`() {
        // 光标滚出视口时候选窗口会落在屏幕外，回报没有意义。
        assertNull(
            composingCursorMarker(
                true,
                true,
                cursorRow = -1,
                cursorCol = 4,
                rows = 24,
                cellWidth = 10f,
                cellHeight = 20f,
            ),
        )
        assertNull(
            composingCursorMarker(
                true,
                true,
                cursorRow = 24,
                cursorCol = 4,
                rows = 24,
                cellWidth = 10f,
                cellHeight = 20f,
            ),
        )
        assertNull(
            composingCursorMarker(
                true,
                true,
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
                true,
                true,
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
