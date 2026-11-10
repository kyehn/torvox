package terminal.emulator

import android.content.Context
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Job
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.launch
import kotlinx.coroutines.sync.Mutex
import kotlinx.coroutines.sync.withLock
import terminal.emulator.bridge.Bridge
import terminal.emulator.bridge.FontInfo
import terminal.emulator.runtime.LogUtil
import terminal.emulator.runtime.TerminalRuntime
import terminal.emulator.settings.SettingsRepository
import terminal.emulator.settings.availableFontFamilies
import terminal.emulator.util.TerminalDispatchers

/**
 * 字体加载、字号/家族设置与字体文件安装。
 *
 * 从 [TerminalViewModel] 拆出：字体三条状态流（可用列表、默认族名、字体信息）
 * 与枚举缓存闩的归属都在此处，视图层经 [availableFonts] 等只读流订阅。
 */
internal class FontManager(
    private val runtime: TerminalRuntime,
    private val settingsRepository: SettingsRepository,
    private val context: Context,
    private val scope: CoroutineScope,
    private val messages: TransientMessageChannel,
) {
    private val _availableFonts = MutableStateFlow<List<String>>(emptyList())

    /** 系统可用字体族（[refreshFontList] 枚举后发布）。 */
    val availableFonts: StateFlow<List<String>> = _availableFonts.asStateFlow()

    private val _defaultFontName = MutableStateFlow("")

    /** 原生当前默认字体族名。 */
    val defaultFontName: StateFlow<String> = _defaultFontName.asStateFlow()

    private val _fontInfo = MutableStateFlow<FontInfo?>(null)

    /** 原生字体信息；null 表示原生尚未上报。 */
    val fontInfo: StateFlow<FontInfo?> = _fontInfo.asStateFlow()

    /** 字体列表是否已枚举过（进程内一次，见 [refreshFontList]）。 */
    @Volatile private var fontListLoaded = false

    /**
     * 从原生发布字体族名与字体信息，两个字段总是同批写入。
     *
     * null 即「原生尚未上报」：设置页据此只显示族名与字号，不伪造单元格尺寸。
     */
    fun publishFontState(bridge: Bridge?) {
        _defaultFontName.value = bridge?.getDefaultFontName().orEmpty()
        _fontInfo.value = bridge?.getFontInfo()
    }

    /**
     * DESIGN 字体选择节：native 应用返回 false 即设置数据错误，
     * 输出日志并清除该设置使会话回落默认。null 表示无会话可验证，不处理。
     */
    private suspend fun clearUnknownFontFamily(requestedFamily: String, applied: Boolean?) {
        if (applied != false) return
        LogUtil.error("FontManager", "Unknown font family, clearing setting: $requestedFamily")
        settingsRepository.clearFontFamily()
    }

    /**
     * 在途加载的协程句柄。字体列表 UI 可在短时间内多次进入，
     * 若无此门闩每次进入都会再启一个枚举并发跑
     * setExtraFontPaths/listFontFamilies/_fontInfo 写入。
     */
    private var fontLoadJob: Job? = null

    /**
     * 会话启动路径：把已存字族应用到本会话的 bridge（bridge 按会话持有，
     * 每个新会话都要应用一次）。只做应用，不列举——列表是 UI 的事。
     */
    fun applyStoredFontSettings() {
        scope.launch(TerminalDispatchers.inputOutput) {
            val bridge = runtime.bridge()
            if (bridge == null) {
                // 会话已登记但 bridge 尚未就绪（spawn 前的瞬间）：跳过本轮，
                // 后续状态更新会再次触发本函数。
                return@launch
            }
            // 先注册用户投放目录再应用，使其字体在 bridge 上可见
            // （重复注册为空操作）；主/次路径都纳入。
            val fontDirs = termuxFontDirs(context).filter {
                it.isDirectory
            }.map { it.absolutePath }
            if (fontDirs.isNotEmpty()) bridge.setExtraFontPaths(fontDirs)
            val storedFamily = settingsRepository.fontFamily.first()
            clearUnknownFontFamily(
                storedFamily,
                bridge.setFontFamily(
                    resolveEffectiveFontFamily(storedFamily),
                ),
            )
        }
    }

    /**
     * 字体列表 UI 路径：枚举并发布可用列表、默认字体名与字体信息。
     * 只在字体列表实际显示时调用（设置页打开字体选择对话框那一刻），
     * 不由会话状态发射触发。
     *
     * 进程内只跑一轮（DESIGN：字体列表被缓存直到应用关闭）：`fonts.xml` 与用户
     * 投放目录在进程存活期间不会变，重跑只会反复走一遍 JNI 列举与目录注册。
     */
    fun refreshFontList() {
        // 并发闩只管「别并发起多轮」；缓存闩只在**真的枚举完成**后置位——
        // 提前置位会让「会话尚未就绪」这一次早退把整进程都锁成空列表，
        // 之后再点开选择框也不再重试。
        if (fontListLoaded || fontLoadJob?.isActive == true) return
        fontLoadJob = scope.launch(TerminalDispatchers.inputOutput) {
            try {
                val bridge = runtime.bridge()
                if (bridge == null) {
                    // 会话已登记但 bridge 尚未就绪（spawn 前的瞬间）：跳过本轮。
                    // _availableFonts 仍为空，直接交给 availableFontFamilies
                    // 会因空列表抛致命异常，把「还没准备好」变成「进程崩溃」。
                    return@launch
                }
                // 先注册用户投放目录再列举，使其字体家族即使在早于
                // Runtime.start() 完成时也被纳入（重复注册为空操作）；主/次路径都纳入。
                val fontDirs = termuxFontDirs(context).filter {
                    it.isDirectory
                }.map { it.absolutePath }
                if (fontDirs.isNotEmpty()) bridge.setExtraFontPaths(fontDirs)
                val allFonts =
                    availableFontFamilies(
                        bridge.listFontFamilies().orEmpty(),
                    )
                _availableFonts.value = allFonts
                fontListLoaded = true
                // 空即「原生未上报」，不得改用列表首项冒充：DESIGN 要求主字体为空时
                // 取 fonts.xml 的 monospace 家族，列表首项是字体库枚举顺序，与之无关。
                // 显示侧本就对空值有回落（FontFamilySelectors 取列表首项仅作标签）。
                publishFontState(bridge)
            } catch (fatal: IllegalStateException) {
                if (fatal is CancellationException) throw fatal
                // 系统 fonts.xml 缺失或不可解析：按 DESIGN 记录日志并崩溃退出，
                // 不得静默回退为空列表。
                LogUtil.error("FontManager", "Fatal: system fonts.xml unreadable", fatal)
                throw fatal
            } catch (exception: Exception) {
                if (exception is CancellationException) throw exception
                LogUtil.error("FontManager", "Failed to load font list", exception)
                throw exception
            }
        }
    }

    // 串行化完整的字体应用链（DataStore 写入 + bridge 字体重载 + 网格重排）。
    // 没有它，快速拖动滑块会启动并发 IO 协程，
    // 使原生 setFontSizeInPlace 调用乱序交错
    // （实测一次拖动中值从 96 跳到 280）并在序列中途重排网格
    // ——即布局错乱的反馈来源。
    private val fontApplyMutex = Mutex()

    /**
     * 轻量的拖动预览：把字号应用到原生管线并刷新单元格度量，
     * 但不写设置、不重排网格。开销足够低，可每个拖动步都运行；
     * 松手手势经 [setFontSize] 提交。
     */
    fun setFontSizeInPlacePreview(size: Float) {
        runtime.setFontSizePreview(size)
    }

    fun setFontSize(size: Float) {
        scope.launch(TerminalDispatchers.inputOutput) {
            fontApplyMutex.withLock {
                settingsRepository.setFontSize(size)
                runtime.applyFontSettings()
                val bridge = runtime.bridge()
                publishFontState(bridge)
            }
        }
    }

    fun setFontFamily(family: String) {
        scope.launch(TerminalDispatchers.inputOutput) {
            try {
                LogUtil.debug("FontManager", "Setting font family: $family")
                settingsRepository.setFontFamily(family)
                val applied = runtime.applyFontSettings()
                val bridge = runtime.bridge()
                clearUnknownFontFamily(family, applied)
                publishFontState(bridge)
                LogUtil.debug("FontManager", "Font applied: ${_defaultFontName.value}")
                val message =
                    if (applied == false) {
                        context.getString(R.string.font_apply_failed, family)
                    } else {
                        context.getString(R.string.font_applied, _defaultFontName.value)
                    }
                messages.send(message)
            } catch (exception: Exception) {
                if (exception is CancellationException) throw exception
                LogUtil.error("FontManager", "setFontFamily failed for $family", exception)
                messages.send(
                    context.getString(R.string.font_apply_failed, exception.message ?: ""),
                )
            }
        }
    }
}
