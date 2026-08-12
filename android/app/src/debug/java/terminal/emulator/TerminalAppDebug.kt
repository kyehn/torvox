package terminal.emulator

import com.ms.square.debugoverlay.DebugOverlay
import com.ms.square.debugoverlay.DebugTab
import com.ms.square.debugoverlay.OverlayMode
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
        // DebugOverlay — full metrics mode (CPU/heap/PSS/FPS) with a custom
        // tab exposing terminal build info inside the debug panel. The
        // overlay itself auto-installs via AndroidX Startup; this only
        // tunes the mode and adds the app-specific tab.
        DebugOverlay.configure {
            overlayMode = OverlayMode.FullMetrics(
                customTabs = listOf(
                    DebugTab(title = "Build Info") {
                        androidx.compose.material3.Text(
                            text = "version=${BuildConfig.VERSION_NAME}\n" +
                                "code=${BuildConfig.VERSION_CODE}\n" +
                                "debug=${BuildConfig.DEBUG}",
                        )
                    },
                ),
                showThermal = false,
            )
        }
        DebugOverlay.addBugReportContributor(
            object : com.ms.square.debugoverlay.BugReportDataContributor {
                override val filename: String = "torvox_build_info.txt"

                override fun writeTo(outputStream: java.io.OutputStream) {
                    outputStream.write("version=${BuildConfig.VERSION_NAME}\n".toByteArray())
                    outputStream.write("code=${BuildConfig.VERSION_CODE}\n".toByteArray())
                    outputStream.write("debug=${BuildConfig.DEBUG}\n".toByteArray())
                }
            },
        )
        super.onCreate()
    }
}
