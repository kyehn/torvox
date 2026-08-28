package terminal.emulator.bridge

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

class FontInfoDtoCodecTest {

    @Test
    fun `fromJson parses full structured info`() {
        val json =
            """
            {
                "active": {"name": "Fira Code", "monospaced": true},
                "cjk_state": "fallback",
                "cjk_families": ["Noto Sans CJK SC", "Noto Sans CJK JP"],
                "cell_width_px": 7.0,
                "cell_height_px": 14.0,
                "font_size": 14.0
            }
            """.trimIndent()

        val dto = FontInfoDto.fromJson(json)
        assertNotNull(dto)
        val active = requireNotNull(requireNotNull(dto).active)
        assertEquals("Fira Code", active.name)
        assertTrue(active.monospaced)
        assertEquals("fallback", dto.cjkState)
        assertEquals(2, dto.cjkFamilies.size)
        assertEquals("Noto Sans CJK SC, Noto Sans CJK JP", dto.cjkFallbackText())
        assertTrue(dto.hasRealCjkFallback)
    }

    @Test
    fun `none state has no fallback`() {
        val json =
            """
            {"active": {"name": "Liberation Mono", "monospaced": true},
             "cjk_state": "none", "cjk_families": [],
             "cell_width_px": 7.0, "cell_height_px": 14.0, "font_size": 14.0}
            """.trimIndent()

        val dto = FontInfoDto.fromJson(json)
        assertNotNull(dto)
        assertFalse(requireNotNull(dto).hasRealCjkFallback)
        assertNull(dto.cjkFallbackText())
    }

    @Test
    fun `malformed json returns null`() {
        assertNull(FontInfoDto.fromJson("not json"))
    }

    @Test
    fun `placeholder json round-trips`() {
        val json = FontInfoDto.placeholderJson("monospace")
        val dto = FontInfoDto.fromJson(json)
        assertNotNull(dto)
        assertEquals("monospace", requireNotNull(requireNotNull(dto).active).name)
        assertEquals("none", dto.cjkState)
    }
}
