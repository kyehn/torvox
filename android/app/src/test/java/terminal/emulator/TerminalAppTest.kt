package terminal.emulator

import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

class TerminalAppTest {
    @Test
    fun `install process is detected by suffix`() {
        assertTrue(TerminalApp.isInstallProcess("com.termux:install"))
    }

    @Test
    fun `main process is not install process`() {
        assertFalse(TerminalApp.isInstallProcess("com.termux"))
        assertFalse(TerminalApp.isInstallProcess(null))
    }
}
