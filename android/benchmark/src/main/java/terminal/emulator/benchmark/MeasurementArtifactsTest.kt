package terminal.emulator.benchmark

import android.os.BatteryManager
import android.os.PowerManager
import org.json.JSONObject
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test
import java.io.File
import java.nio.file.Files

/**
 * 制品落盘的判据：JSONL 行、制品校验和与配额轮转。
 *
 * 校验和用公开的 SHA-256 标准向量（`"abc"` 的摘要）作断言，不拿被测实现算出的值反过来
 * 当期望；轮转与丢弃按字节数断言，不按实现细节断言。
 */
class MeasurementArtifactsTest {
    private val rootDir: File = Files.createTempDirectory("measurement-store").toFile()

    @After
    fun removeTempDir() {
        rootDir.deleteRecursively()
    }

    @Test
    fun appendsRowsAsJsonLines() {
        val store = MeasurementStore("run-1", rootDir, quotaBytes = 1024)

        store.appendRow("{\"run_id\":\"run-1\"}")
        store.appendRow("{\"run_id\":\"run-1\"}")

        val lines = File(rootDir, "rows.jsonl").readLines()
        assertEquals(2, lines.size)
        assertEquals("run-1", JSONObject(lines[0]).getString("run_id"))
    }

    @Test
    fun writesArtifactWithChecksumAndSize() {
        val store = MeasurementStore("run-1", rootDir, quotaBytes = 1024)

        val ref = store.writeArtifact("coldStart/1/logcat.txt", "abc".toByteArray())

        val written = requireNotNull(ref) { "制品应落盘" }
        assertEquals("coldStart/1/logcat.txt", written.name)
        assertEquals(3L, written.byteCount)
        assertEquals(
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad",
            written.sha256,
        )
        assertEquals("abc", File(written.path).readText())
    }

    @Test
    fun rotatesOldestArtifactWhenQuotaExceeded() {
        val store = MeasurementStore("run-1", rootDir, quotaBytes = 100)

        val first = requireNotNull(store.writeArtifact("first.txt", ByteArray(60))) { "首件应落盘" }
        val second = requireNotNull(store.writeArtifact("second.txt", ByteArray(60))) { "次件应落盘" }

        assertFalse("最旧的制品应被轮转清理", File(first.path).exists())
        assertTrue(File(second.path).exists())
        assertTrue(store.artifactBytes() <= 100)
    }

    @Test
    fun discardsArtifactLargerThanQuota() {
        val store = MeasurementStore("run-1", rootDir, quotaBytes = 10)

        val ref = store.writeArtifact("huge.txt", ByteArray(20))

        assertNull("单件就超配额时应丢弃", ref)
        assertEquals(0L, store.artifactBytes())
    }

    @Test
    fun countsIterationsPerScenario() {
        val store = MeasurementStore("run-1", rootDir, quotaBytes = 1024)

        assertEquals(1, store.nextIteration("coldStart"))
        assertEquals(2, store.nextIteration("coldStart"))
        assertEquals(1, store.nextIteration("scroll"))
    }

    @Test
    fun serializesRowWithEnvironmentExitAndVerdict() {
        val store = MeasurementStore("run-1", rootDir, quotaBytes = 1024)
        val exit = ExitInfoRecord(
            timestampMillis = 1_795_279_264_131L,
            pid = 18084,
            reasonCode = 4,
            reasonName = "APP CRASH(EXCEPTION)",
            subReasonCode = 0,
            subReasonName = "UNKNOWN",
            description = "crash",
            pss = "0.00",
            rss = "0.00",
        )
        val row = MeasurementRow(
            runId = store.runId,
            label = "coldStart",
            iteration = 1,
            frameworkIteration = 0,
            measured = true,
            startedAtMillis = 1_795_279_264_000L,
            endedAtMillis = 1_795_279_264_131L,
            environment = DeviceEnvironment(

                model = "sdk_gphone64_x86_64",
                romBuildId = "sdk_gphone64_x86_64-userdebug 15 AE3A.240806.043 12960925 dev-keys",
                apiLevel = 35,
                refreshRateHz = 60.000004,
                batteryLevelPercent = 100,
                batteryStatus = BatteryManager.BATTERY_STATUS_CHARGING,
                batteryTemperatureCelsius = 25.0,
                thermalStatus = PowerManager.THERMAL_STATUS_NONE,
                animationScales = AnimationScales("1.0", "1.0", "1.0"),
                fixedPerformanceMode = FixedPerformanceState.UNVERIFIABLE,
                capturedAtEpochMillis = 1_795_279_264_000L,
            ),
            app = AppIdentity("com.termux", 2000L, "0.1.0"),
            metrics = mapOf("total_time_millis" to 363.0),
            exits = listOf(exit),
            artifacts = emptyList(),
            verdict = "FAIL",
            note = "测量窗口内进程退出",
        )

        val json = JSONObject(row.toJson())
        assertEquals("run-1", json.getString("run_id"))
        assertEquals("coldStart", json.getJSONObject("scenario").getString("label"))
        assertEquals(1, json.getJSONObject("scenario").getInt("iteration"))
        assertTrue(json.getJSONObject("scenario").getBoolean("measured"))
        assertEquals(0, json.getJSONObject("scenario").getInt("framework_iteration"))
        assertEquals(131, json.getJSONObject("scenario").getInt("elapsed_millis"))
        assertEquals("sdk_gphone64_x86_64", json.getJSONObject("device").getString("model"))
        assertEquals(35, json.getJSONObject("device").getInt("api"))
        assertEquals("CHARGING", json.getJSONObject("device").getString("battery_status"))
        assertEquals("NONE", json.getJSONObject("device").getString("thermal_status"))
        assertEquals("1.0", json.getJSONObject("device").getJSONObject("animation_scales").getString("window"))
        assertEquals(
            "UNVERIFIABLE",
            json.getJSONObject("device").getString("fixed_performance_mode"),
        )
        assertEquals("com.termux", json.getJSONObject("app").getString("package"))
        assertEquals(363.0, json.getJSONObject("metrics").getDouble("total_time_millis"), 0.0)
        assertEquals(1, json.getJSONArray("exit").length())
        assertTrue(json.getJSONArray("exit").getString(0).contains("APP CRASH(EXCEPTION)"))
        assertEquals("FAIL", json.getJSONObject("verdict").getString("result"))
    }
}
