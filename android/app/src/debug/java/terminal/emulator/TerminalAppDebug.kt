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
        // DebugOverlay — Hidden by default: FullMetrics polls /proc/stat,
        // Debug.getPss() and a Choreographer doFrame callback on the MAIN
        // thread every second, which competes with the render pipeline and
        // measurably degrades frame pacing on the software-rendered
        // emulator (frame-time spikes 70-106ms correlated with overlay
        // ticks). Opt back in per-session with:
        //   adb shell setprop debug.torvox.overlay full
        val overlayFull = try {
            val propValue = Class
                .forName("android.os.SystemProperties")
                .getMethod("get", String::class.java)
                .invoke(null, "debug.torvox.overlay") as? String
            propValue == "full"
        } catch (_: Throwable) {
            false
        }
        DebugOverlay.configure {
            overlayMode = if (overlayFull) {
                OverlayMode.FullMetrics(
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
            } else {
                OverlayMode.Hidden(
                    customTabs = listOf(
                        DebugTab(title = "Build Info") {
                            androidx.compose.material3.Text(
                                text = "version=${BuildConfig.VERSION_NAME}\n" +
                                    "code=${BuildConfig.VERSION_CODE}\n" +
                                    "debug=${BuildConfig.DEBUG}",
                            )
                        },
                    ),
                )
            }
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
