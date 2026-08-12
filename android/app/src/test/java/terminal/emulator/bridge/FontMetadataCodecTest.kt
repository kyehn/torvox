package terminal.emulator.bridge

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertNull
import org.junit.Test

class FontMetadataCodecTest {

    @Test
    fun `fromJson parses valid metadata`() {
        val json =
            """
            {
                "activeFamily": "Liberation Mono",
                "filePath": "/system/fonts/LiberationMono-Regular.ttf",
                "fontSize": 14.0,
                "cjkFallback": "Noto Sans CJK SC",
                "nerdFallback": "JetBrainsMono Nerd Font",
                "emojiFallback": "Noto Color Emoji",
                "primaryIsCjk": false
            }
            """.trimIndent()

        val metadata = FontMetadataCodec.fromJson(json)
        assertNotNull(metadata)
        assertEquals("Liberation Mono", metadata!!.activeFamily)
        assertEquals("/system/fonts/LiberationMono-Regular.ttf", metadata.filePath)
        assertEquals(14.0f, metadata.fontSize, 0.001f)
        assertEquals("Noto Sans CJK SC", metadata.cjkFallback)
        assertEquals("JetBrainsMono Nerd Font", metadata.nerdFallback)
        assertEquals("Noto Color Emoji", metadata.emojiFallback)
        assertEquals(false, metadata.primaryIsCjk)
    }

    @Test
    fun `fromJson returns null on malformed input`() {
        assertNull(FontMetadataCodec.fromJson("not json"))
        assertNull(FontMetadataCodec.fromJson(""))
        assertNull(FontMetadataCodec.fromJson("{broken"))
    }

    @Test
    fun `fromJson handles missing fields gracefully`() {
        val json = """{"activeFamily": "monospace"}"""
        val metadata = FontMetadataCodec.fromJson(json)
        assertNotNull(metadata)
        assertEquals("monospace", metadata!!.activeFamily)
        assertEquals("", metadata.filePath)
        assertEquals(0f, metadata.fontSize, 0.001f)
        assertEquals(false, metadata.primaryIsCjk)
    }

    @Test
    fun `toJson produces valid JSON roundtrip`() {
        val original =
            FontMetadata(
                activeFamily = "Droid Sans Mono",
                filePath = "/system/fonts/DroidSansMono.ttf",
                fontSize = 12.5f,
                cjkFallback = "",
                nerdFallback = "",
                emojiFallback = "",
                primaryIsCjk = false,
            )

        val json = FontMetadataCodec.toJson(original)
        val parsed = FontMetadataCodec.fromJson(json)

        assertNotNull(parsed)
        assertEquals(original.activeFamily, parsed!!.activeFamily)
        assertEquals(original.filePath, parsed.filePath)
        assertEquals(original.fontSize, parsed.fontSize, 0.001f)
        assertEquals(original.cjkFallback, parsed.cjkFallback)
        assertEquals(original.primaryIsCjk, parsed.primaryIsCjk)
    }
}
