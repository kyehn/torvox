package terminal.emulator.benchmark

import androidx.test.platform.io.PlatformTestStorageRegistry
import org.json.JSONArray
import org.json.JSONObject
import java.io.File
import java.security.MessageDigest

/** 一件制品的指针。 */
internal data class ArtifactRef(
    /** 制品名（相对本轮目录的路径）。 */
    val name: String,
    /** 设备上的绝对路径。 */
    val path: String,
    /** 字节数。 */
    val byteCount: Long,
    /** SHA-256 摘要（十六进制小写）。 */
    val sha256: String,
)

/**
 * 一轮测量的落盘处：一份 JSONL 行文件与一个制品目录。
 *
 * 行文件只放摘要与制品指针，原始制品按 `run_id / 场景 / 迭代` 分目录存放并附校验和。
 * 目录落在测试输出目录里，宿主侧随测试结果一并回收。
 */
internal class MeasurementStore(
    /** 本轮标识。 */
    val runId: String,
    /** 本轮目录。 */
    private val rootDir: File,
    /** 制品总配额（字节）。 */
    private val quotaBytes: Long,
) {
    /** 行文件。 */
    private val rowsFile = File(rootDir, "rows.jsonl")

    /** 制品目录。 */
    private val artifactsDir = File(rootDir, "artifacts")

    /** 已落盘的制品，按写入顺序（最旧在前）。 */
    private val artifacts = mutableListOf<ArtifactRef>()

    /** 各场景已用的迭代序号。 */
    private val iterations = mutableMapOf<String, Int>()

    /** 下一个迭代序号（从 1 起）。 */
    fun nextIteration(label: String): Int {
        val next = (iterations[label] ?: 0) + 1
        iterations[label] = next
        return next
    }

    /** 追加一行 JSONL。 */
    fun appendRow(row: String) {
        rowsFile.parentFile?.mkdirs()
        rowsFile.appendText(row + "\n")
    }

    /**
     * 落一件制品。
     *
     * 配额内才落盘：腾不出空间时轮转清理最旧的制品；单件就超配额时丢弃该制品并返回
     * null——宁可不留，也不让设备存储被制品占满。
     */
    fun writeArtifact(name: String, bytes: ByteArray): ArtifactRef? {
        if (bytes.size > quotaBytes) return null
        rotateToFit(bytes.size)
        val file = File(artifactsDir, name)
        file.parentFile?.mkdirs()
        file.writeBytes(bytes)
        val ref = ArtifactRef(name, file.absolutePath, bytes.size.toLong(), sha256(bytes))
        artifacts += ref
        return ref
    }

    /** 制品占用的总字节数。 */
    fun artifactBytes(): Long = artifacts.sumOf { it.byteCount }

    /** 轮转清理：腾出足够空间容纳新制品。 */
    private fun rotateToFit(incomingBytes: Int) {
        while (artifactBytes() + incomingBytes > quotaBytes && artifacts.isNotEmpty()) {
            val oldest = artifacts.removeAt(0)
            File(oldest.path).delete()
        }
    }
}

/** SHA-256 摘要（十六进制小写）。 */
internal fun sha256(bytes: ByteArray): String = MessageDigest.getInstance("SHA-256").digest(bytes)
    .joinToString("") { byte -> "%02x".format(byte.toInt() and 0xFF) }

/**
 * 一次测量的行记录（JSONL 的一行）。
 *
 * 字段与研究报告的「数据落地格式」一致：`run_id`、`device`、`app`、`scenario`、
 * `exit`、`artifacts` 与 `verdict`。指标随场景而异，尚无指标的场次不写该字段，
 * 不落空壳。
 */
internal data class MeasurementRow(
    /** 本轮标识。 */
    val runId: String,
    /** 场景名。 */
    val label: String,
    /** 归到的全场景矩阵编号；应用内部回归项没有对应场景。 */
    val scenarioId: String?,
    /** 迭代序号（本仓测量包装的调用次序，从 1 起）。 */
    val iteration: Int,
    /** 宏基准框架的迭代序号；框架的准备轮次（温／热启动前的那一轮）为 null。 */
    val frameworkIteration: Int?,
    /** 该行是否是一次计入结论的测量。 */
    val measured: Boolean,
    /** 场景起止时刻（epoch 毫秒）。 */
    val startedAtMillis: Long,
    val endedAtMillis: Long,
    /** 环境快照。 */
    val environment: DeviceEnvironment,
    /** 被测应用标识。 */
    val app: AppIdentity,
    /** 指标（数值型；无指标的场次不写该字段）。 */
    val metrics: Map<String, Double>,
    /** 测量窗口旁取到的宿主负载；负载读不出来时为空。 */
    val hostLoad: HostLoad?,
    /** 测量窗口内的进程退出记录。 */
    val exits: List<ExitInfoRecord>,
    /** 该次测量关联的制品。 */
    val artifacts: List<ArtifactRef>,
    /** 判定结果（`PASS`、`FAIL` 或 `超限终止`）。 */
    val verdict: String,
    /** 备注：失败原因或补充说明。 */
    val note: String,
) {
    /** 序列化成一行 JSON。 */
    fun toJson(): String = JSONObject().apply {
        put("run_id", runId)
        put(
            "device",
            JSONObject().apply {
                put("model", environment.model)
                put("rom_build_id", environment.romBuildId)
                put("api", environment.apiLevel)
                put("refresh_rate_hz", environment.refreshRateHz)
                put("battery_level_percent", environment.batteryLevelPercent)
                put("battery_status", batteryStatusName(environment.batteryStatus))
                put("battery_temperature_celsius", environment.batteryTemperatureCelsius)
                put("thermal_status", thermalStatusName(environment.thermalStatus))
                put(
                    "animation_scales",
                    JSONObject().apply {
                        put("window", environment.animationScales.window)
                        put("transition", environment.animationScales.transition)
                        put("animator_duration", environment.animationScales.animatorDuration)
                    },
                )
                put("fixed_performance_mode", environment.fixedPerformanceMode.name)
            },
        )
        put(
            "app",
            JSONObject().apply {
                put("package", app.packageName)
                put("version_code", app.versionCode)
                put("version_name", app.versionName)
            },
        )
        if (metrics.isNotEmpty()) {
            put(
                "metrics",
                JSONObject().apply { metrics.forEach { (name, value) -> put(name, value) } },
            )
        }
        hostLoad?.let { load ->
            put(
                "host_load",
                JSONObject().apply {
                    put("steal_ratio", load.stealRatio)
                    put("busy_ratio", load.busyRatio)
                    put("window_millis", load.windowMillis)
                    put("contended", load.contended)
                },
            )
        }
        put(
            "scenario",
            JSONObject().apply {
                put("label", label)
                if (scenarioId != null) put("matrix_id", scenarioId)
                put("iteration", iteration)
                // 温／热启动前框架会多跑一轮准备轮次（该轮不计入框架的测量迭代），
                // 故记下框架自己的序号：null 即准备轮次。
                put("measured", measured)
                if (frameworkIteration != null) put("framework_iteration", frameworkIteration)
                put("started_at", startedAtMillis)
                put("ended_at", endedAtMillis)
                put("elapsed_millis", endedAtMillis - startedAtMillis)
            },
        )
        put("exit", JSONArray().apply { exits.forEach { put(it.describe()) } })
        put(
            "artifacts",
            JSONArray().apply {
                artifacts.forEach { artifact ->
                    put(
                        JSONObject().apply {
                            put("name", artifact.name)
                            put("path", artifact.path)
                            put("byte_count", artifact.byteCount)
                            put("sha256", artifact.sha256)
                        },
                    )
                }
            },
        )
        put(
            "verdict",
            JSONObject().apply {
                put("result", verdict)
                put("note", note)
            },
        )
    }.toString()
}

/**
 * 打开本轮的落盘处。
 *
 * 目录落在测试输出目录里：宿主侧随测试结果一并回收，设备侧不留散落的副本。
 */
internal fun openMeasurementStore(label: String): MeasurementStore {
    val storage = PlatformTestStorageRegistry.getInstance()
    val anchor = requireNotNull(storage.getOutputFileUri("anchor").path) { "测试存储没有给出输出路径" }
    val outputDir = requireNotNull(File(anchor).parentFile) { "测试存储的输出路径没有父目录：$anchor" }
    val runId = "$label-${System.currentTimeMillis()}"
    return MeasurementStore(runId, File(outputDir, runId), MEASUREMENT_QUOTA_BYTES)
}
