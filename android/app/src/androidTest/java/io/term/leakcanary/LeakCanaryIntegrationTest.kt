package io.term.leakcanary

import leakcanary.AppWatcher
import org.junit.Assert.assertTrue
import org.junit.Test

class LeakCanaryIntegrationTest {
    @Test
    fun appWatcherIsInstalled() {
        assertTrue("AppWatcher should be installed in debug build", AppWatcher.isInstalled)
    }
}
