package io.term

import leakcanary.LeakCanary

class TerminalAppDebug : TerminalApp() {
    override fun onCreate() {
        LeakCanary.config =
            LeakCanary.config.copy(
                retainedVisibleThreshold = 3,
                maxStoredHeapDumps = 5,
                computeRetainedHeapSize = true,
                dumpHeapWhenDebugging = false,
            )
        super.onCreate()
    }
}
