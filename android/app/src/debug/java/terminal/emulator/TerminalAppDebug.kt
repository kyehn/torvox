package terminal.emulator

import android.annotation.SuppressLint
import androidx.compose.material3.Text
import com.ms.square.debugoverlay.DebugOverlay
import com.ms.square.debugoverlay.DebugTab
import com.ms.square.debugoverlay.OverlayMode

class TerminalAppDebug : TerminalApp() {
    override fun onCreate() {
        // DebugOverlay — Hidden by default: FullMetrics polls /proc/stat,
        // Debug.getPss() and a Choreographer doFrame callback on the MAIN
        // thread every second, which competes with the render pipeline and
        // measurably degrades frame pacing on the software-rendered
        // emulator (frame-time spikes 70-106ms correlated with overlay
        // ticks). Opt back in per-session with:
        //   adb shell setprop debug.terminal.overlay full
        val overlayFull =
            try {
                // debug 覆盖开关经系统属性读取（仅 debug 构建）：无公开 API，必须反射；失败即隐藏。
                @SuppressLint("PrivateApi")
                val propValue =
                    Class
                        .forName("android.os.SystemProperties")
                        .getMethod("get", String::class.java)
                        .invoke(null, "debug.terminal.overlay") as? String
                propValue == "full"
            } catch (_: Throwable) {
                false
            }
        val customTabs = listOf(buildInfoTab())
        DebugOverlay.configure {
            overlayMode =
                if (overlayFull) {
                    OverlayMode.FullMetrics(customTabs = customTabs, showThermal = false)
                } else {
                    OverlayMode.Hidden(customTabs = customTabs)
                }
        }
        DebugOverlay.addBugReportContributor(
            object : com.ms.square.debugoverlay.BugReportDataContributor {
                override val filename: String = "terminal_build_info.txt"

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

private fun buildInfoTab(): DebugTab {
    val info =
        buildString {
            appendLine("version=${BuildConfig.VERSION_NAME}")
            appendLine("code=${BuildConfig.VERSION_CODE}")
            append("debug=${BuildConfig.DEBUG}")
        }
    return DebugTab(title = "Build Info") {
        Text(text = info)
    }
}
