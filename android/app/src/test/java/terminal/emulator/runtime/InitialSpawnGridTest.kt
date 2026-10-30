package terminal.emulator.runtime

import org.junit.Assert.assertEquals
import org.junit.Test

/**
 * spawn 之前的网格决策：PTY 的 winsize 决定 shell 怎么排版它的第一个 prompt，故度量就绪时
 * 必须直接建出真实网格，而不是先建 24×80、attach 之后再 resize（那会让 shell 重排已经打印过
 * 的 prompt，首屏留下截断 + 补行的两份 prompt）。
 */
class InitialSpawnGridTest {

    // 1080x2209 px surface（1080x2400 减 191 px 系统栏）、26.4x53.2 px 单元格、132 px 修饰键栏：
    // cols = floor(1080 / 26.4) = 40，rows = floor((2209 - 132) / 53.2) = 39。
    @Test
    fun `real metrics yield the real grid not the default`() {
        assertEquals(
            Pair(39, 40),
            initialSpawnGrid(
                surfaceWidth = 1080,
                surfaceHeight = 2209,
                cellWidth = 26.4f,
                cellHeight = 53.2f,
                modifierBarHeightPx = 132,
                imeReserve = 0,
            ),
        )
    }

    /** 原生字体度量或 surface 尺寸尚未就绪（冷启动竞态）：退回默认网格，与旧路径一致。 */
    @Test
    fun `missing metrics fall back to the default grid`() {
        assertEquals(Pair(DEFAULT_GRID_ROWS, DEFAULT_GRID_COLS), initialSpawnGrid(1080, 2209, 0f, 53.2f, 132, 0))
        assertEquals(Pair(DEFAULT_GRID_ROWS, DEFAULT_GRID_COLS), initialSpawnGrid(1080, 2209, 26.4f, 0f, 132, 0))
        assertEquals(Pair(DEFAULT_GRID_ROWS, DEFAULT_GRID_COLS), initialSpawnGrid(0, 2209, 26.4f, 53.2f, 132, 0))
        assertEquals(Pair(DEFAULT_GRID_ROWS, DEFAULT_GRID_COLS), initialSpawnGrid(1080, 0, 26.4f, 53.2f, 132, 0))
    }

    /**
     * spawn 网格必须与 attach 之后的重算网格逐字相等。
     *
     * 不等即意味着 attach 时会多一次 resize，shell 收到 SIGWINCH 后重排已打印的
     * prompt（首屏多出一行提示符）。两条路径必须共用同一套扣减与取整口径。
     */
    @Test
    fun `spawn grid equals the grid recomputed with the same metrics`() {
        val cellWidth = 26.4f
        val cellHeight = 53.2f
        val modifierBarHeightPx = 132
        val spawn =
            initialSpawnGrid(
                surfaceWidth = 1080,
                surfaceHeight = 2209,
                cellWidth = cellWidth,
                cellHeight = cellHeight,
                modifierBarHeightPx = modifierBarHeightPx,
                imeReserve = 0,
            )
        val recomputed =
            computeGridDimensions(
                surfaceWidth = 1080,
                surfaceHeight =
                computeGridAvailableHeight(
                    surfaceHeight = 2209,
                    modifierBarHeightPx = modifierBarHeightPx,
                    imeReserve = 0,
                ),
                cellWidth = cellWidth,
                cellHeight = cellHeight,
            )
        assertEquals(spawn, recomputed)
    }
}
