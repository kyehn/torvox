package io.torvox

import leakcanary.LeakCanary

class TorvoxAppDebug : TorvoxApp() {
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
