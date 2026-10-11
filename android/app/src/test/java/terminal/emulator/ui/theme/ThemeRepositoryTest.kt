package terminal.emulator.ui.theme

import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.toArgb
import org.junit.Assert.assertEquals
import org.junit.Assert.assertThrows
import org.junit.Test

class ThemeRepositoryTest {
    private fun packedWith(
        background: ByteArray = byteArrayOf(0x11, 0x22, 0x33),
        foreground: ByteArray = byteArrayOf(0x44, 0x55, 0x66),
        cursor: ByteArray = byteArrayOf(0x77, 0x88.toByte(), 0x99.toByte()),
        selection: ByteArray = byteArrayOf(0xAA.toByte(), 0xBB.toByte(), 0xCC.toByte()),
        ansiByte: Byte = 0x01,
    ): ByteArray {
        val ansiColors = ByteArray(48) { ansiByte }
        return background + foreground + cursor + selection + ansiColors
    }

    @Test
    fun `themeFromPacked maps 60 bytes to theme colors`() {
        val packedColors = packedWith()
        val theme = ThemeRepository.themeFromPacked("test_theme", true, packedColors)
        assertEquals("test_theme", theme.name)
        assertEquals(Color(0xFF112233), theme.background)
        assertEquals(Color(0xFF445566), theme.foreground)
        assertEquals(Color(0xFF778899), theme.cursor)
        assertEquals(Color(0xFFAABBCC), theme.selectionBackground)
        assertEquals(16, theme.ansi.size)
        assertEquals(Color(0xFF010101), theme.ansi[0])
    }

    @Test
    fun `themeFromPacked preserves dark flag`() {
        val packedColors = packedWith()
        val darkTheme = ThemeRepository.themeFromPacked("test_dark", true, packedColors)
        val lightTheme = ThemeRepository.themeFromPacked("test_light", false, packedColors)
        assertEquals(Color(0xFF112233).toArgb(), darkTheme.background.toArgb())
        assertEquals(Color(0xFF112233).toArgb(), lightTheme.background.toArgb())
    }

    @Test
    fun `themeFromPacked rejects wrong lengths instead of truncating`() {
        assertThrows(IllegalArgumentException::class.java) {
            ThemeRepository.themeFromPacked("test_theme", true, ByteArray(59))
        }
        assertThrows(IllegalArgumentException::class.java) {
            ThemeRepository.themeFromPacked("test_theme", true, ByteArray(61))
        }
        assertThrows(IllegalArgumentException::class.java) {
            ThemeRepository.themeFromPacked("test_theme", true, ByteArray(0))
        }
    }
}
