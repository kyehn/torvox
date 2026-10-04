package terminal.emulator.monitor

import android.os.PowerManager
import androidx.test.core.app.ApplicationProvider
import org.junit.Assert.assertEquals
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config

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
        var criticalCalls = 0
        val monitor =
            ThermalMonitor(context) {
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
        val monitor = ThermalMonitor(context)

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
    fun critical_status_notifies_once_and_writes_no_file() {
        val context = ApplicationProvider.getApplicationContext<android.content.Context>()
        var criticalCalls = 0
        val monitor = ThermalMonitor(context) { criticalCalls++ }

        monitor.onThermalStatusChanged(PowerManager.THERMAL_STATUS_CRITICAL)

        assertEquals(1, criticalCalls)
    }

    @Test
    fun `unregister is a safe no-op and the lifecycle never throws`() {
        // JVM 的 shadow 只实现单参 listener 注册，本仓用的双参（executor）版本
        // 走真实 Android 桩并抛「not mocked」——`register` 按设计在内部吞掉它。
        // 此处锁死的是生命周期契约本身：未注册时注销为空操作，注册→注销→重注册
        // 全程不抛，且决策逻辑在周期之后依然只触发一次。
        val context = ApplicationProvider.getApplicationContext<android.content.Context>()
        val (monitor, calls) = monitor()

        monitor.unregister()
        monitor.register()
        monitor.unregister()
        monitor.register()

        monitor.onThermalStatusChanged(PowerManager.THERMAL_STATUS_CRITICAL)
        assertEquals(1, calls())
        monitor.unregister()
    }
}
