package terminal.emulator.benchmark

/**
 * 全场景矩阵：一个场景的负载、指标、采集面、判据与资源上限。
 *
 * 矩阵是跨应用对比的编排清单：同一场景给各被测应用**完全相同**的字节流与网格尺寸，
 * 负载不等价则对比无效。每项在矩阵里写死，不在执行期临时决定——否则两轮对比的
 * 条件不同，横向关系失效。
 *
 * 资源上限取自「极端场景快速失败」的要求：每个场景都有 wall-clock 上限、输出字节
 * 上限与采样间隔下限，超限即终止该场景并记「超限终止」，不继续等待、不继续追加
 * 输出，也不在失败场景上继续消耗设备。
 */
internal data class Scenario(
    /** 场景编号（S1–S12）。 */
    val id: String,
    /** 场景名。 */
    val label: String,
    /** 统一负载：各被测应用拿到的完全相同的输入。 */
    val load: String,
    /** 采集的指标。 */
    val metrics: List<String>,
    /** 采集面：各指标取自哪个通道。 */
    val surfaces: List<String>,
    /** 判据：什么算通过。 */
    val verdict: String,
    /** wall-clock 上限（毫秒）：超时即终止并记「超限终止」。 */
    val wallClockLimitMillis: Long,
    /** 输出字节上限：超限即终止并记「超限终止」。 */
    val outputByteLimit: Long,
    /** 采样间隔下限（毫秒）：轮询不得比它更密。 */
    val samplingIntervalFloorMillis: Long,
)

/** 文本输出的字节上限：超限即终止该场景。 */
private const val SCENARIO_OUTPUT_BYTE_LIMIT = 8L * 1024 * 1024

/** 采样间隔下限：`process_stats` 的轮询下限是 100 毫秒，再密则采集本身成为噪声。 */
private const val SCENARIO_SAMPLING_INTERVAL_FLOOR_MILLIS = 100L

/**
 * 全场景矩阵。
 *
 * 场景分类与研究报告的矩阵表一一对应：启动三态、稳态空载、持续输出、突发输出、
 * 长滚动、宽字符行、交互延迟、输入洪水、前后台切换与资源极端。
 */
internal val SCENARIO_MATRIX: List<Scenario> = listOf(
    Scenario(
        id = "S1",
        label = "冷启动",
        load = "杀进程后 am start -W 拉起",
        metrics = listOf("启动到首帧", "退出归因"),
        surfaces = listOf("am start -W", "显示级 frametimeline", "exit-info"),
        verdict = "首帧在预算内、无异常退出",
        wallClockLimitMillis = 60_000L,
        outputByteLimit = SCENARIO_OUTPUT_BYTE_LIMIT,
        samplingIntervalFloorMillis = SCENARIO_SAMPLING_INTERVAL_FLOOR_MILLIS,
    ),
    Scenario(
        id = "S2",
        label = "温启动",
        load = "Activity 重启不杀进程",
        metrics = listOf("首帧", "帧数"),
        surfaces = listOf("显示级 frametimeline", "应用内首帧探针"),
        verdict = "首帧在预算内",
        wallClockLimitMillis = 30_000L,
        outputByteLimit = SCENARIO_OUTPUT_BYTE_LIMIT,
        samplingIntervalFloorMillis = SCENARIO_SAMPLING_INTERVAL_FLOOR_MILLIS,
    ),
    Scenario(
        id = "S3",
        label = "热启动",
        load = "进程存活切前台",
        metrics = listOf("首帧", "丢帧数"),
        surfaces = listOf("显示级 frametimeline", "应用内呈现汇总"),
        verdict = "无丢帧、无 Surface 丢失",
        wallClockLimitMillis = 15_000L,
        outputByteLimit = SCENARIO_OUTPUT_BYTE_LIMIT,
        samplingIntervalFloorMillis = SCENARIO_SAMPLING_INTERVAL_FLOOR_MILLIS,
    ),
    Scenario(
        id = "S4",
        label = "稳态空载",
        load = "静置 10 秒",
        metrics = listOf("帧数", "CPU 占用", "内存"),
        surfaces = listOf("显示级 frametimeline", "呈现汇总", "meminfo"),
        verdict = "空载不产生持续绘制",
        wallClockLimitMillis = 15_000L,
        outputByteLimit = SCENARIO_OUTPUT_BYTE_LIMIT,
        samplingIntervalFloorMillis = SCENARIO_SAMPLING_INTERVAL_FLOOR_MILLIS,
    ),
    Scenario(
        id = "S5",
        label = "持续输出",
        load = "连续字节流（逐行）",
        metrics = listOf("吞吐", "呈现间隔 p95", "内存峰值"),
        surfaces = listOf("显示级 frametimeline", "呈现汇总", "meminfo"),
        verdict = "无丢帧、内存有界",
        wallClockLimitMillis = 60_000L,
        outputByteLimit = SCENARIO_OUTPUT_BYTE_LIMIT,
        samplingIntervalFloorMillis = SCENARIO_SAMPLING_INTERVAL_FLOOR_MILLIS,
    ),
    Scenario(
        id = "S6",
        label = "突发输出",
        load = "单次大块字节流",
        metrics = listOf("无损", "内存峰值", "退出归因"),
        surfaces = listOf("meminfo", "exit-info", "logcat"),
        verdict = "输出无损、无异常退出",
        wallClockLimitMillis = 30_000L,
        outputByteLimit = SCENARIO_OUTPUT_BYTE_LIMIT,
        samplingIntervalFloorMillis = SCENARIO_SAMPLING_INTERVAL_FLOOR_MILLIS,
    ),
    Scenario(
        id = "S7",
        label = "长滚动",
        load = "填满回滚区后拖动",
        metrics = listOf("呈现间隔 p95", "丢帧"),
        surfaces = listOf("显示级 frametimeline", "呈现汇总"),
        verdict = "p95 在预算内、无停摆",
        wallClockLimitMillis = 30_000L,
        outputByteLimit = SCENARIO_OUTPUT_BYTE_LIMIT,
        samplingIntervalFloorMillis = SCENARIO_SAMPLING_INTERVAL_FLOOR_MILLIS,
    ),
    Scenario(
        id = "S8",
        label = "宽字符行",
        load = "CJK、组合字符、emoji 行",
        metrics = listOf("排版正确", "呈现间隔 p95"),
        surfaces = listOf("显示级 frametimeline", "呈现汇总", "截图复核"),
        verdict = "无错位、无丢帧",
        wallClockLimitMillis = 30_000L,
        outputByteLimit = SCENARIO_OUTPUT_BYTE_LIMIT,
        samplingIntervalFloorMillis = SCENARIO_SAMPLING_INTERVAL_FLOOR_MILLIS,
    ),
    Scenario(
        id = "S9",
        label = "交互延迟",
        load = "探针命令 + 计时回显",
        metrics = listOf("按键到回显间隔"),
        surfaces = listOf("logcat 时间戳"),
        verdict = "间隔在预算内",
        wallClockLimitMillis = 30_000L,
        outputByteLimit = SCENARIO_OUTPUT_BYTE_LIMIT,
        samplingIntervalFloorMillis = SCENARIO_SAMPLING_INTERVAL_FLOOR_MILLIS,
    ),
    Scenario(
        id = "S10",
        label = "输入洪水",
        load = "连续按键事件",
        metrics = listOf("丢键数", "ANR"),
        surfaces = listOf("exit-info", "logcat"),
        verdict = "无丢键、无 ANR",
        wallClockLimitMillis = 30_000L,
        outputByteLimit = SCENARIO_OUTPUT_BYTE_LIMIT,
        samplingIntervalFloorMillis = SCENARIO_SAMPLING_INTERVAL_FLOOR_MILLIS,
    ),
    Scenario(
        id = "S11",
        label = "后台前台切换",
        load = "切后台再切回 + 旋转",
        metrics = listOf("恢复后首帧", "丢帧"),
        surfaces = listOf("显示级 frametimeline", "呈现汇总"),
        verdict = "恢复即出帧、无 Surface 丢失",
        wallClockLimitMillis = 30_000L,
        outputByteLimit = SCENARIO_OUTPUT_BYTE_LIMIT,
        samplingIntervalFloorMillis = SCENARIO_SAMPLING_INTERVAL_FLOOR_MILLIS,
    ),
    Scenario(
        id = "S12",
        label = "资源极端",
        load = "低内存、强压、灭屏、Doze",
        metrics = listOf("存活", "快速死亡", "归因"),
        surfaces = listOf("exit-info", "batterystats", "thermalservice"),
        verdict = "快速死亡且归因明确",
        wallClockLimitMillis = 60_000L,
        outputByteLimit = SCENARIO_OUTPUT_BYTE_LIMIT,
        samplingIntervalFloorMillis = SCENARIO_SAMPLING_INTERVAL_FLOOR_MILLIS,
    ),
)

/** 按编号取场景；编号不在矩阵里即判失败，编排不得引用不存在的场景。 */
internal fun scenarioById(id: String): Scenario = SCENARIO_MATRIX.firstOrNull { it.id == id } ?: error("场景矩阵里没有 $id")
