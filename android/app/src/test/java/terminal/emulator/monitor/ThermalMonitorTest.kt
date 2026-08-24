package terminal.emulator.monitor

import android.os.PowerManager
import androidx.test.core.app.ApplicationProvider
import org.junit.Assert.assertEquals
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config
import java.io.File

/**
 * Behavioural tests for [ThermalMonitor]'s decision logic: transition dedup,
 * the severity ladder (only CRITICAL+ fires the kill callback), and the
 * status label mapping. The monitor itself is constructed via Robolectric;
 * the system listener registration path is not exercised here.
 */
@RunWith(RobolectricTestRunner::class)
@Config(sdk = [34])
class ThermalMonitorTest {

    private fun monitor(): Pair<ThermalMonitor, () -> Int> {
        val context = ApplicationProvider.getApplicationContext<android.content.Context>()
        val logDir = File(kotlin.io.path.createTempDirectory("thermal-logs").toFile(), "logs")
        var criticalCalls = 0
        val monitor =
            ThermalMonitor(context, logDir) {
                criticalCalls++
            }
        return monitor to { criticalCalls }
    }

    @Test
    fun critical_status_fires_callback_once_and_dedups() {
        val (monitor, calls) = monitor()

        monitor.onThermalStatusChanged(PowerManager.THERMAL_STATUS_CRITICAL)
        assertEquals(1, calls())
        // Same status again — dedup must not re-fire.
        monitor.onThermalStatusChanged(PowerManager.THERMAL_STATUS_CRITICAL)
        assertEquals(1, calls())

        // A reset to normal, then a new critical transition fires again.
        monitor.onThermalStatusChanged(PowerManager.THERMAL_STATUS_NONE)
        monitor.onThermalStatusChanged(PowerManager.THERMAL_STATUS_CRITICAL)
        assertEquals(2, calls())
    }

    @Test
    fun emergency_and_shutdown_also_fire_callback() {
        val (monitor, calls) = monitor()

        monitor.onThermalStatusChanged(PowerManager.THERMAL_STATUS_EMERGENCY)
        assertEquals(1, calls())
        monitor.onThermalStatusChanged(PowerManager.THERMAL_STATUS_NONE)
        monitor.onThermalStatusChanged(PowerManager.THERMAL_STATUS_SHUTDOWN)
        assertEquals(2, calls())
    }

    @Test
    fun severe_and_below_never_fire_kill_callback() {
        val (monitor, calls) = monitor()

        // LIGHT, MODERATE and SEVERE are throttling levels — the process
        // must survive them (fixes the regression where SEVERE killed).
        monitor.onThermalStatusChanged(PowerManager.THERMAL_STATUS_LIGHT)
        monitor.onThermalStatusChanged(PowerManager.THERMAL_STATUS_MODERATE)
        monitor.onThermalStatusChanged(PowerManager.THERMAL_STATUS_SEVERE)
        assertEquals(0, calls())
    }

    @Test
    fun status_label_mapping_is_complete() {
        val context = ApplicationProvider.getApplicationContext<android.content.Context>()
        val monitor =
            ThermalMonitor(
                context,
                File(context.cacheDir, "unused"),
            )

        assertEquals("THERMAL_STATUS_NONE", monitor.thermalStatusLabel(PowerManager.THERMAL_STATUS_NONE))
        assertEquals("THERMAL_STATUS_LIGHT", monitor.thermalStatusLabel(PowerManager.THERMAL_STATUS_LIGHT))
        assertEquals("THERMAL_STATUS_MODERATE", monitor.thermalStatusLabel(PowerManager.THERMAL_STATUS_MODERATE))
        assertEquals("THERMAL_STATUS_SEVERE", monitor.thermalStatusLabel(PowerManager.THERMAL_STATUS_SEVERE))
        assertEquals("THERMAL_STATUS_CRITICAL", monitor.thermalStatusLabel(PowerManager.THERMAL_STATUS_CRITICAL))
        assertEquals("THERMAL_STATUS_EMERGENCY", monitor.thermalStatusLabel(PowerManager.THERMAL_STATUS_EMERGENCY))
        assertEquals("THERMAL_STATUS_SHUTDOWN", monitor.thermalStatusLabel(PowerManager.THERMAL_STATUS_SHUTDOWN))
        assertEquals("UNKNOWN(42)", monitor.thermalStatusLabel(42))
    }

    @Test
    fun critical_writes_thermal_log_file() {
        val context = ApplicationProvider.getApplicationContext<android.content.Context>()
        val logDir = File(kotlin.io.path.createTempDirectory("thermal-write").toFile(), "logs")
        var criticalCalls = 0
        val monitor =
            ThermalMonitor(context, logDir) {
                criticalCalls++
            }

        monitor.onThermalStatusChanged(PowerManager.THERMAL_STATUS_CRITICAL)

        assertEquals(1, criticalCalls)
        val logs = logDir.listFiles { file -> file.name.startsWith("thermal_") }
        assertEquals("exactly one thermal log", 1, logs?.size)
    }
}
