package io.torvox.ocr

import android.graphics.Rect
import io.torvox.decodeRgbaToPixels
import java.io.File

/**
 * Result of locating one inverted (text-selected) cell in a GPU frame.
 *
 * @param centerX absolute x pixel of the cell centre
 * @param centerY absolute y pixel of the cell centre
 * @param left    cell left edge in pixels
 * @param top     cell top edge in pixels
 * @param right   cell right edge in pixels
 * @param bottom  cell bottom edge in pixels
 */
data class InvertedCell(
    val centerX: Int,
    val centerY: Int,
    val left: Int,
    val top: Int,
    val right: Int,
    val bottom: Int,
)

/**
 * Visual-analysis helper used by the selection instrumentation tests.
 *
 * The terminal inverts the colours of a selected cell (background becomes the text colour
 * and vice-versa). This scans the raw RGBA GPU frame, estimates the page background from
 * the corners, and returns every connected region whose colour is clearly *not* the
 * background (i.e. the inverted selection highlight). Each region is reduced to its
 * bounding cell so tests can compare it against the long-press coordinate.
 *
 * @param rgbaPath absolute path to a `.rgba` raw frame (width x height x 4 bytes)
 * @param gridCols number of terminal columns (for cell banding); pass 0 to skip banding
 * @param gridRows number of terminal rows; pass 0 to skip banding
 */
fun analyzeInvertedCells(
    rgbaPath: String,
    gridCols: Int = 0,
    gridRows: Int = 0,
): List<InvertedCell> {
    val frame = decodeRgbaToPixels(File(rgbaPath))
    val width = frame.width
    val height = frame.height
    val pixels = frame.pixels

    // Estimate background from the four corners (skip a 4px margin).
    val bgSamples =
        listOf(
            sampleCorner(pixels, width, height, 4, 4),
            sampleCorner(pixels, width, height, width - 5, 4),
            sampleCorner(pixels, width, height, 4, height - 5),
            sampleCorner(pixels, width, height, width - 5, height - 5),
        )
    val bg = averageColor(bgSamples)

    // Mark every pixel that differs strongly from the background.
    val mask = BooleanArray(width * height)
    var invertedCount = 0
    for (i in pixels.indices) {
        val p = pixels[i]
        val dr = kotlin.math.abs(((p shr 16) and 0xFF) - bg.first)
        val dg = kotlin.math.abs(((p shr 8) and 0xFF) - bg.second)
        val db = kotlin.math.abs((p and 0xFF) - bg.third)
        val diff = dr + dg + db
        if (diff > 120) {
            mask[i] = true
            invertedCount++
        }
    }

    // If almost nothing is inverted, there is no selection highlight.
    if (invertedCount < 20) return emptyList()

    return bandIntoCells(mask, width, height, gridCols, gridRows)
}

private fun sampleCorner(
    pixels: IntArray,
    width: Int,
    height: Int,
    x: Int,
    y: Int,
): Int = pixels[y * width + x]

private fun averageColor(colors: List<Int>): Triple<Int, Int, Int> {
    var r = 0
    var g = 0
    var b = 0
    for (c in colors) {
        r += (c shr 16) and 0xFF
        g += (c shr 8) and 0xFF
        b += c and 0xFF
    }
    val n = colors.size.coerceAtLeast(1)
    return Triple(r / n, g / n, b / n)
}

/**
 * Groups the inverted pixel mask into bounding rectangles. When [gridCols]/[gridRows] are
 * provided the rectangles are snapped to the terminal cell grid; otherwise they are the
 * raw connected bounding boxes (cheap single-pass row/col scan).
 */
private fun bandIntoCells(
    mask: BooleanArray,
    width: Int,
    height: Int,
    gridCols: Int,
    gridRows: Int,
): List<InvertedCell> {
    if (gridCols <= 0 || gridRows <= 0) {
        // Raw bounding-box scan.
        var minX = width
        var minY = height
        var maxX = 0
        var maxY = 0
        for (y in 0 until height) {
            for (x in 0 until width) {
                if (mask[y * width + x]) {
                    if (x < minX) minX = x
                    if (x > maxX) maxX = x
                    if (y < minY) minY = y
                    if (y > maxY) maxY = y
                }
            }
        }
        if (maxX < minX) return emptyList()
        return listOf(
            InvertedCell(
                (minX + maxX) / 2,
                (minY + maxY) / 2,
                minX,
                minY,
                maxX,
                maxY,
            ),
        )
    }

    val cellW = width / gridCols
    val cellH = height / gridRows
    val seen = mutableSetOf<Int>()
    val cells = mutableListOf<InvertedCell>()
    for (y in 0 until height) {
        for (x in 0 until width) {
            if (!mask[y * width + x]) continue
            val col = (x / cellW).coerceIn(0, gridCols - 1)
            val row = (y / cellH).coerceIn(0, gridRows - 1)
            val key = row * gridCols + col
            if (!seen.add(key)) continue
            val left = col * cellW
            val top = row * cellH
            val right = left + cellW
            val bottom = top + cellH
            cells.add(
                InvertedCell(
                    (left + right) / 2,
                    (top + bottom) / 2,
                    left,
                    top,
                    right,
                    bottom,
                ),
            )
        }
    }
    return cells
}
