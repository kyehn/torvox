package terminal.emulator.settings

import android.content.Context
import dagger.hilt.android.qualifiers.ApplicationContext
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.withContext
import terminal.emulator.runtime.LogUtil
import terminal.emulator.util.TerminalDispatchers
import java.io.File
import java.io.IOException
import javax.inject.Inject
import javax.inject.Singleton

private const val TAG = "TermuxProperties"

/** 一次加载的完整结果：属性与来源，供设置页展示与诊断。 */
data class TermuxConfiguration(
    val properties: TermuxProperties = TermuxProperties(),
    val propertiesPath: String? = null,
    val errors: List<String> = emptyList(),
)

/**
 * `~/.termux/termux.properties` 的读取与缓存。
 *
 * 读取时机是显式的 [reloadNow]：首帧前（`TerminalRuntime.start`）与每次设置重下发
 * （`TerminalRuntime.applySettings`）各一次。不用 `FileObserver`——它只覆盖 inotify
 * 可达的路径，用户用 SAF 或 `adb push` 改写时事件未必到达，而重读对所有写入方式都
 * 成立，代价是一次 stat。
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

    /** 最近一次成功读取的配置：读失败时继续用它，避免界面闪回默认值。 */
    @Volatile private var lastGood: TermuxConfiguration = TermuxConfiguration()

    /** 同步重读；调用方已在线程池上时使用。 */
    suspend fun reloadNow() {
        withContext(TerminalDispatchers.inputOutput) {
            val errors = mutableListOf<String>()
            val home = File(context.filesDir, "home")
            val propertiesFile = locate(home, "termux.properties")

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

            for (error in errors) LogUtil.warn(TAG, error)
            val configuration =
                TermuxConfiguration(
                    properties = properties,
                    propertiesPath = propertiesFile?.absolutePath,
                    errors = errors,
                )
            lastGood = configuration
            _configuration.value = configuration
        }
    }

    /**
     * 主路径优先、次路径兜底（与 termux 相同）：`~/.termux/termux.properties`
     * 存在即用，否则 `~/.config/termux/termux.properties`。
     *
     * 只认**常规文件**且不跟随符号链接：这份文件由外部写入（`adb push`、SAF），
     * 链接可以让读取跑到应用家目录之外，也可能指向一个设备；把它们当配置读进来只会
     * 得到无法解释的行为。非常规文件（管道、目录）同样跳过。
     */
    private fun locate(home: File, name: String): File? {
        val primary = File(File(home, ".termux"), name)
        if (isReadableRegularFile(primary)) return primary
        val secondary = File(File(home, ".config/termux"), name)
        return secondary.takeIf { isReadableRegularFile(it) }
    }

    private fun isReadableRegularFile(file: File): Boolean {
        if (!file.exists() || !file.canRead()) return false
        return try {
            java.nio.file.Files.isRegularFile(
                file.toPath(),
                java.nio.file.LinkOption.NOFOLLOW_LINKS,
            )
        } catch (error: Exception) {
            false
        }
    }

    /**
     * 读不出时记错误并返回 null；`null` 的语义是「保留上次结果」，不是「空配置」。
     *
     * 超过 [MAX_BYTES] 的文件按读失败处理：配置是用户手写的小文件，一个几百兆的
     * 文件要么是放错了地方，要么是别的东西——两种情况都不该被读进内存。
     */
    private fun readFile(file: File, errors: MutableList<String>): String? {
        if (file.length() > MAX_BYTES) {
            errors += "${file.name}: larger than $MAX_BYTES bytes; skipped"
            return null
        }
        return try {
            file.readText(Charsets.UTF_8)
        } catch (error: IOException) {
            errors += "${file.name}: read failed (${error.message})"
            null
        } catch (error: SecurityException) {
            errors += "${file.name}: read denied (${error.message})"
            null
        }
    }
}

/** 单个配置文件的体积上限；超过即视为无效（`config.md` 的读取规则）。 */
private const val MAX_BYTES = 64 * 1024
