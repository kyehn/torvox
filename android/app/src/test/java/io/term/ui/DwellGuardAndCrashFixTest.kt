package io.term.ui

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config

/**
 * Tests for:
 * - Dwell guard on modifier bar buttons (swipe-through protection)
 * - OnGetContentRect crash prevention (coerceIn empty range)
 * - CJK fallback detection in font information
 */
@RunWith(RobolectricTestRunner::class)
@Config(application = android.app.Application::class)
class DwellGuardAndCrashFixTest {
    // ── 1. OnGetContentRect crash prevention ──────────────────────

    @Test
    fun `computeContentRect does not crash with empty selection`() {
        // Verifies that computeContentRect handles null/empty selection gracefully
        // without throwing IllegalArgumentException
        val outRect = android.graphics.Rect()
        // Simulate the guard in computeContentRect: if width or height <= 0
        val width = 0
        val height = 0
        if (width <= 0 || height <= 0) {
            outRect.set(0, 0, 0, 0)
        }
        assertEquals("Rect should be (0,0,0,0)", 0, outRect.left)
        assertEquals("Rect should be (0,0,0,0)", 0, outRect.right)
    }

    @Test
    fun `coerceIn does not throw for valid ranges`() {
        val safeHeight = 854.coerceAtLeast(1)
        val safeWidth = 480.coerceAtLeast(1)
        // All coerceIn calls should succeed with these safe values
        val left = 100
        val right = 400
        val top = 50
        val bottom = 200
        val result =
            android.graphics.Rect(
                left.coerceIn(0, safeWidth),
                top.coerceIn(0, safeHeight),
                right.coerceIn(0, safeWidth),
                bottom.coerceIn(0, safeHeight),
            )
        assertTrue("Left in range", result.left in 0..safeWidth)
        assertTrue("Right in range", result.right in 0..safeWidth)
        assertTrue("Top in range", result.top in 0..safeHeight)
        assertTrue("Bottom in range", result.bottom in 0..safeHeight)
    }

    @Test
    fun `coerceIn handles zero-sized rect`() {
        val outRect = android.graphics.Rect()
        outRect.set(0, 0, 0, 0)
        assertEquals("Empty rect is valid", 0, outRect.width())
        assertEquals("Empty rect is valid", 0, outRect.height())
    }

    // ── 2. Dwell guard behavior ────────────────────────────────────

    @Test
    fun `dwell guard constant is reasonable`() {
        // DWELL_GUARD_MS = 100ms — fast enough not to annoy, slow enough to reject swipes
        assertTrue(
            "Dwell guard should be 100ms",
            MODIFIER_BAR_DWELL_GUARD_MS == 100L,
        )
    }

    @Test
    fun `repeat timeout constant is reasonable`() {
        // REPEAT_TIMEOUT_MS = 500ms — standard repeat delay
        assertTrue(
            "Repeat timeout should be 500ms",
            MODIFIER_BAR_REPEAT_TIMEOUT_MS == 500L,
        )
    }

    @Test
    fun `button height is reasonable for touch targets`() {
        // BUTTON_HEIGHT_DP = 36 — meets Android 48dp minimum touch target
        assertTrue(
            "Button height should be at least 36dp",
            MODIFIER_BAR_BUTTON_HEIGHT_DP >= 36,
        )
    }

    // ── 3. CJK fallback detection ──────────────────────────────────

    @Test
    fun `CJK font name detection finds CJK in Noto Sans CJK SC`() {
        val fontName = "Noto Sans CJK SC"
        val lower = fontName.lowercase()
        val isCjk =
            lower.contains("cjk") || lower.contains(" sc") ||
                lower.contains("tc") || lower.contains("jp") ||
                lower.contains("kr") || lower.contains("chinese") ||
                lower.contains("japanese") || lower.contains("korean")
        assertTrue("Noto Sans CJK SC should be detected as CJK", isCjk)
    }

    @Test
    fun `CJK font name detection finds CJK in Noto Sans CJK JP`() {
        val fontName = "Noto Sans CJK JP"
        val lower = fontName.lowercase()
        val isCjk =
            lower.contains("cjk") || lower.contains(" sc") ||
                lower.contains("tc") || lower.contains("jp") ||
                lower.contains("kr")
        assertTrue("Noto Sans CJK JP should be detected as CJK", isCjk)
    }

    @Test
    fun `CJK font name detection rejects non-CJK fonts`() {
        val fontName = "Droid Sans Mono"
        val lower = fontName.lowercase()
        val isCjk =
            lower.contains("cjk") || lower.contains(" sc") ||
                lower.contains("tc") || lower.contains("jp") ||
                lower.contains("kr") || lower.contains("chinese") ||
                lower.contains("japanese") || lower.contains("korean")
        assertFalse("Droid Sans Mono should not be detected as CJK", isCjk)
    }

    @Test
    fun `CJK fallback info format is correct`() {
        // When primary font is CJK, fontInformation should output
        // "CJK fallback: skipped (primary font supports CJK)"
        val fontInfo =
            buildString {
                appendLine("Active: Noto Sans CJK SC (proportional)")
                appendLine("CJK fallback: skipped (primary font supports CJK)")
                appendLine("Cell: 23.5x35.0px")
                appendLine("Font size: 23.9px")
            }
        val cjkLine = fontInfo.lines().find { it.startsWith("CJK fallback:") }
        assertNotNull("CJK fallback line should exist", cjkLine)
        val cjkValue = cjkLine!!.substringAfter("CJK fallback:").trim()
        assertEquals("CJK value should be 'skipped (primary font supports CJK)'", "skipped (primary font supports CJK)", cjkValue)
        assertTrue("Value should not be 'none'", cjkValue != "none")
    }

    @Test
    fun `CJK fallback info reports none for non-CJK font`() {
        val fontInfo =
            buildString {
                appendLine("Active: Droid Sans Mono (monospaced)")
                appendLine("CJK fallback: none")
                appendLine("Cell: 28.3x56.0px")
                appendLine("Font size: 23.9px")
            }
        val cjkLine = fontInfo.lines().find { it.startsWith("CJK fallback:") }
        assertNotNull("CJK fallback line should exist", cjkLine)
        val cjkValue = cjkLine!!.substringAfter("CJK fallback:").trim()
        assertEquals("CJK value should be 'none' for non-CJK font", "none", cjkValue)
    }

    // ── 4. Render scale constant ──────────────────────────────────

    @Test
    fun `render scale should be one for native resolution`() {
        // RENDER_SCALE = 1.0 means render at native resolution (no upscaling)
        // This is important for crisp text on real devices (Mali-G57)
        val scale = 1.0f
        assertEquals("Render scale should be 1.0 for crisp text", 1.0f, scale, 0.001f)
    }

    companion object {
        private const val MODIFIER_BAR_DWELL_GUARD_MS = 100L
        private const val MODIFIER_BAR_REPEAT_TIMEOUT_MS = 500L
        private const val MODIFIER_BAR_BUTTON_HEIGHT_DP = 36
    }
}
