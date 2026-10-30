package terminal.emulator.config

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test
import terminal.emulator.bridge.CursorStyle
import java.io.File

class TermuxPropertiesTest {
    private fun home(): File = File(System.getProperty("java.io.tmpdir"), "termux-home-${System.nanoTime()}").apply { mkdirs() }

    private fun write(home: File, rel: String, text: String): File {
        val file = File(home, rel).apply { parentFile?.mkdirs() }
        file.writeText(text)
        return file
    }

    @Test
    fun `absent file yields defaults`() {
        val home = home()
        val config = TermuxProperties.parse(home)
        assertNull(config.workingDirectory)
        assertNull(config.scrollbackLines)
        assertNull(config.cursorStyle)
    }

    @Test
    fun `primary path wins over secondary`() {
        val home = home()
        val dir = File(home, "primary").apply { mkdirs() }
        write(home, ".termux/termux.properties", "default-working-directory=${dir.absolutePath}\n")
        write(home, ".config/termux/termux.properties", "default-working-directory=/secondary\n")
        val config = TermuxProperties.parse(home)
        assertEquals(dir.absolutePath, config.workingDirectory)
    }

    @Test
    fun `invalid working directory falls back`() {
        val home = home()
        write(home, ".termux/termux.properties", "default-working-directory=/no/such/dir\n")
        assertNull(TermuxProperties.parse(home).workingDirectory)
    }

    @Test
    fun `scrollback range enforced`() {
        val home = home()
        write(home, ".termux/termux.properties", "terminal-transcript-rows=50\n")
        assertNull(TermuxProperties.parse(home).scrollbackLines)
        write(home, ".termux/termux.properties", "terminal-transcript-rows=200\n")
        assertEquals(200, TermuxProperties.parse(home).scrollbackLines)
    }

    @Test
    fun `cursor style mapping`() {
        val home = home()
        write(home, ".termux/termux.properties", "terminal-cursor-style=beam\n")
        assertEquals(CursorStyle.Bar, TermuxProperties.parse(home).cursorStyle)
        write(home, ".termux/termux.properties", "terminal-cursor-style=underline\n")
        assertEquals(CursorStyle.Underline, TermuxProperties.parse(home).cursorStyle)
        write(home, ".termux/termux.properties", "terminal-cursor-style=bogus\n")
        assertNull(TermuxProperties.parse(home).cursorStyle)
    }

    @Test
    fun `unknown keys ignored`() {
        val home = home()
        write(home, ".termux/termux.properties", "foo=bar\nterminal-transcript-rows=300\n")
        assertEquals(300, TermuxProperties.parse(home).scrollbackLines)
    }
}