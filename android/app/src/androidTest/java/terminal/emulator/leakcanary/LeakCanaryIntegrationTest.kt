package terminal.emulator.leakcanary

import android.app.Application
import androidx.test.core.app.ApplicationProvider
import androidx.test.platform.app.InstrumentationRegistry
import leakcanary.AppWatcher
import org.junit.Assert.assertTrue
import org.junit.Test
import terminal.emulator.TerminalLogcatTest

class LeakCanaryIntegrationTest : TerminalLogcatTest() {
    @Test
    fun appWatcherIsInstalled() {
        val application = ApplicationProvider.getApplicationContext<Application>()
        InstrumentationRegistry.getInstrumentation().runOnMainSync {
            AppWatcher.manualInstall(application)
        }
        assertTrue("AppWatcher should be installed in androidTest build", AppWatcher.isInstalled)
    }
}
