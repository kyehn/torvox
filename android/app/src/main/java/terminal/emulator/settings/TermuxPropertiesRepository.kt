package terminal.emulator.settings

import android.content.Context
import dagger.hilt.android.qualifiers.ApplicationContext
import java.io.File
import java.io.IOException
import javax.inject.Inject
import javax.inject.Singleton
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import terminal.emulator.runtime.LogUtil
import terminal.emulator.util.TerminalDispatchers

private const val TAG = "TermuxProperties"

/** 一次加载的完整结果：属性、颜色覆盖与来源，供设置页展示与诊断。 */
data class TermuxConfiguration(
    val properties: TermuxProperties = TermuxProperties(),
    val colors: ColorsProperties? = null,
    val propertiesPath: String? = null,
    val colorsPath: String? = null,
    val errors: List<String> = emptyList(),
)

/**
 * `~/.termux/{termux.properties,colors.properties}` 的读取与缓存。
 *
 * 读取时机是显式的 [reload]：应用回到前台、退出设置页、安装完成后各一次。不用
 * `FileObserver`——它只覆盖 inotify 可达的路径，用户用 SAF 或 `adb push` 改写时
 * 事件未必到达，而「回到前台重读」对所有写入方式都成立，代价是两次 stat。
 *
 * 文件不存在是正常状态（等价于全部默认值），不是错误；只有「存在但读不出」才记错误，
 * 且保留上一次成功的结果，不把用户的配置因为一次 IO 失败清空。
 */
@Singleton
class TermuxPropertiesRepository
@Inject
constructor(@ApplicationContext private val context: Context) {
    private val _configuration = MutableStateFlow(TermuxConfiguration())
    val configuration: StateFlow<TermuxConfiguration> = _configuration.asStateFlow()

    private val scope = CoroutineScope(SupervisorJob() + TerminalDispatchers.inputOutput)

    /** 最近一次成功读取的配置：读失败时继续用它，避免界面闪回默认值。 */
    @Volatile private var lastGood: TermuxConfiguration = TermuxConfiguration()

    /** 非阻塞重读；同一时刻只保留最后一次请求（重读是幂等的）。 */
    fun reload() {
        scope.launch { reloadNow() }
    }

    /** 同步重读；调用方已在线程池上时使用。 */
    suspend fun reloadNow() {
        withContext(TerminalDispatchers.inputOutput) {
            val errors = mutableListOf<String>()
            val home = File(context.filesDir, "home")
            val propertiesFile = locate(home, "termux.properties")
            val colorsFile = locate(home, "colors.properties")

            val properties =
                if (propertiesFile == null) {
                    TermuxProperties()
                } else {
                    readFile(propertiesFile, errors)?.let { text ->
                        val loaded = TermuxPropertiesParser.parse(text)
                        errors += loaded.errors.map { "${propertiesFile.name}: $it" }
                        loaded.properties
                    } ?: lastGood.properties
                }

            val colors =
                if (colorsFile == null) {
                    null
                } else {
                    readFile(colorsFile, errors)?.let { text ->
                        val (parsed, colorErrors) = ColorsPropertiesParser.parse(text)
                        errors += colorErrors.map { "colors.properties: $it" }
                        parsed.takeUnless { it.isEmpty }
                    }
                }

            for (error in errors) LogUtil.warn(TAG, error)
            val configuration =
                TermuxConfiguration(
                    properties = properties,
                    colors = colors,
                    propertiesPath = propertiesFile?.absolutePath,
                    colorsPath = colorsFile?.absolutePath,
                    errors = errors,
                )
            lastGood = configuration
            _configuration.value = configuration
        }
    }

    /**
     * 主路径优先、次路径兜底（与 termux 相同）：`~/.termux/termux.properties`
     * 存在即用，否则 `~/.config/termux/termux.properties`。
     */
    private fun locate(home: File, name: String): File? {
        val primary = File(File(home, ".termux"), name)
        if (primary.isFile) return primary
        val secondary = File(File(home, ".config/termux"), name)
        return secondary.takeIf { it.isFile }
    }

    /** 读不出时记错误并返回 null；`null` 的语义是「保留上次结果」，不是「空配置」。 */
    private fun readFile(file: File, errors: MutableList<String>): String? =
        try {
            file.readText(Charsets.UTF_8)
        } catch (error: IOException) {
            errors += "${file.name}: read failed (${error.message})"
            null
        } catch (error: SecurityException) {
            errors += "${file.name}: read denied (${error.message})"
            null
        }
}
