package terminal.emulator.leakcanary

import leakcanary.AppWatcher
import org.junit.Assert.assertTrue
import org.junit.Test
import terminal.emulator.TerminalLogcatTest

class LeakCanaryIntegrationTest : TerminalLogcatTest() {
    @Test
    fun appWatcherIsInstalled() {
        assertTrue("AppWatcher should be installed in debug build", AppWatcher.isInstalled)
    }
}
