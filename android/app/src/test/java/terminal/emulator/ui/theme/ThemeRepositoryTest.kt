package terminal.emulator.ui.theme

import androidx.compose.ui.graphics.Color
import org.junit.Assert.assertEquals
import org.junit.Assert.assertThrows
import org.junit.Test

class ThemeRepositoryTest {
    private fun catalogEntry(flags: Int, name: String, colors: ByteArray): ByteArray {
        val nameBytes = name.toByteArray(Charsets.UTF_8)
        return byteArrayOf(flags.toByte(), nameBytes.size.toByte()) + nameBytes + colors
    }

    /** 十六色在目录里各占三字节且同值，故夹具只需一个字节值。 */
    private fun colors(background: Int, foreground: Int, ansiByte: Int): ByteArray =
        rgb(background) + rgb(foreground) + ByteArray(16 * 3) { ansiByte.toByte() }

    private fun rgb(value: Int): ByteArray = byteArrayOf(
        (value ushr 16 and 0xFF).toByte(),
        (value ushr 8 and 0xFF).toByte(),
        (value and 0xFF).toByte(),
    )

    private fun catalog(vararg entries: ByteArray): ByteArray =
        byteArrayOf(entries.size.toByte(), 0) + entries.reduce { left, right -> left + right }

    @Test
    fun `catalog maps packed bytes to theme colors`() {
        val parsed =
            ThemeRepository.parseCatalog(
                catalog(
                    catalogEntry(
                        ThemeRepository.FLAG_DARK or ThemeRepository.FLAG_DEFAULT_DARK,
                        "test_theme",
                        colors(0x112233, 0x445566, 0x99),
                    ),
                    catalogEntry(ThemeRepository.FLAG_DEFAULT_LIGHT, "test_day", colors(0x010203, 0x040506, 0x07)),
                ),
            )
        val theme = parsed.themes.single { it.name == "test_theme" }
        assertEquals(Color(0xFF112233), theme.background)
        assertEquals(Color(0xFF445566), theme.foreground)
        assertEquals(16, theme.ansi.size)
        assertEquals(Color(0xFF999999), theme.ansi[0])
        assertEquals(Color(0xFF999999), theme.ansi[15])
        assertEquals("test_theme", parsed.defaultDarkName)
        assertEquals("test_day", parsed.defaultLightName)
    }

    @Test
    fun `catalog keeps the native order`() {
        val parsed =
            ThemeRepository.parseCatalog(
                catalog(
                    catalogEntry(
                        ThemeRepository.FLAG_DARK or ThemeRepository.FLAG_DEFAULT_DARK,
                        "night",
                        colors(0x000000, 0x000000, 0x00),
                    ),
                    catalogEntry(ThemeRepository.FLAG_DEFAULT_LIGHT, "day", colors(0x000000, 0x000000, 0x00)),
                ),
            )
        assertEquals(listOf("night", "day"), parsed.themes.map { it.name })
    }

    @Test
    fun `catalog rejects wrong lengths instead of truncating or padding`() {
        val whole =
            catalog(
                catalogEntry(
                    ThemeRepository.FLAG_DARK or ThemeRepository.FLAG_DEFAULT_DARK,
                    "test_theme",
                    colors(0x112233, 0x445566, 0x99),
                ),
            )
        val truncated = whole.copyOfRange(0, whole.size - 1)
        assertThrows(IllegalArgumentException::class.java) { ThemeRepository.parseCatalog(truncated) }
        val padded = whole + byteArrayOf(0)
        assertThrows(IllegalArgumentException::class.java) { ThemeRepository.parseCatalog(padded) }
        assertThrows(IllegalArgumentException::class.java) { ThemeRepository.parseCatalog(ByteArray(0)) }
    }

    @Test
    fun `catalog requires both default markers`() {
        val noLight =
            catalog(
                catalogEntry(
                    ThemeRepository.FLAG_DARK or ThemeRepository.FLAG_DEFAULT_DARK,
                    "test_theme",
                    colors(0x000000, 0x000000, 0x00),
                ),
            )
        assertThrows(IllegalArgumentException::class.java) { ThemeRepository.parseCatalog(noLight) }
        val noDark =
            catalog(catalogEntry(ThemeRepository.FLAG_DEFAULT_LIGHT, "test_theme", colors(0x000000, 0x000000, 0x00)))
        assertThrows(IllegalArgumentException::class.java) { ThemeRepository.parseCatalog(noDark) }
    }
}
