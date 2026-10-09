package terminal.emulator.settings

import androidx.datastore.preferences.core.Preferences
import androidx.datastore.preferences.core.edit
import androidx.datastore.preferences.core.floatPreferencesKey
import androidx.datastore.preferences.core.stringPreferencesKey
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.MutableSharedFlow
import kotlinx.coroutines.flow.debounce
import kotlinx.coroutines.flow.distinctUntilChanged
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.flow.map
import kotlinx.coroutines.launch
import kotlinx.coroutines.sync.Mutex
import kotlinx.coroutines.sync.withLock
import terminal.emulator.util.TerminalDispatchers
import javax.inject.Inject
import javax.inject.Singleton
import kotlin.math.floor
import kotlin.math.roundToInt

@Singleton
class SettingsRepository
@Inject
constructor(private val provider: SettingsDataStoreProvider) {
    private object Keys {
        val FONT_SIZE = floatPreferencesKey("font_size")
        val FONT_FAMILY = stringPreferencesKey("font_family")
        val THEME_NAME = stringPreferencesKey("theme_name")
        val DAY_THEME_NAME = stringPreferencesKey("day_theme_name")
        val NIGHT_THEME_NAME = stringPreferencesKey("night_theme_name")
        val THEME_MODE = stringPreferencesKey("theme_mode")
        val SHELL = stringPreferencesKey("shell")
        val APP_THEME_MODE = stringPreferencesKey("app_theme_mode")
        val BOOTSTRAP_URL = stringPreferencesKey("bootstrap_url")
    }

    companion object {
        private const val DEFAULT_THEME = "Dracula Plus"
        const val DEFAULT_DAY_THEME_NAME = "Catppuccin Latte"
        const val DEFAULT_FOLLOW_SYSTEM = "follow_system"
        const val DEFAULT_THEME_MODE = "fixed"

        /** 自由文本落盘的防抖窗口：每次写入都是完整文件重写。 */
        private const val DEBOUNCE_MILLIS = 300L

        /** Shell 启动入口默认空（DESIGN :113 未设置时为空），空即走默认回退链。 */
        const val DEFAULT_SHELL = ""

        /**
         * 首次启动的按设备自适应字号（sp）：全新安装得到的尺寸能显示约
         * [DEFAULT_FONT_COLUMNS_TARGET] 个可见列（等宽字形约 0.6em 宽：sp = widthDp / (0.6 * target)），
         * 并钳位到 [ADAPTIVE_DEFAULT_MIN_SP, ADAPTIVE_DEFAULT_MAX_SP]。
         * 该区间约束的是「自适应算出的默认值落在哪」，与用户可选的
         * [FONT_SIZE_MIN_SP]..[FONT_SIZE_MAX_PX] 是两件事：此处的下限使小屏手机
         * 的初始字号不低于可读范围，上限使超大屏不会一启动就只有几列字。
         * 已在同一模拟器上与真 termux 0.118.3 标定（1080x2400@420dpi）：
         * termux 字形带 27px / 字距 ~21.2px / ~51 列，本应用 27px / 21.8px / ~49 列
         * ——在 ±10% 容差内，无需再改。
         */
        fun defaultFontSizeFor(screenWidthDp: Float): Float = (
            screenWidthDp / DEFAULT_FONT_COLUMNS_TARGET /
                MONOSPACE_CHAR_ASPECT
            ).coerceIn(
            ADAPTIVE_DEFAULT_MIN_SP,
            ADAPTIVE_DEFAULT_MAX_SP,
        )

        private const val DEFAULT_FONT_COLUMNS_TARGET = 52f
        private const val MONOSPACE_CHAR_ASPECT = 0.6f

        /** 自适应默认值的下限/上限，与用户可选范围无关（见 [defaultFontSizeFor]）。 */
        const val ADAPTIVE_DEFAULT_MIN_SP = 14f
        const val ADAPTIVE_DEFAULT_MAX_SP = 24f

        /**
         * 用户可选的字号范围与精度，取自 Termux
         * `TermuxAppSharedPreferences.getDefaultFontSizes`（DESIGN.md:89
         * 「默认大小与可选范围/精度须参考 Termux」）：下限 4dip；默认值 12dip 且取偶，
         * 故最小调整步长为 2；上限写作 256**像素**而非 sp，故换算需除以 sp→px 系数。
         *
         * 这是用户可选区间的**唯一**定义处。此前另有一份与原生
         * `setFontSizeInPlace` 守卫（`4.0..=100.0`）重复的 `NATIVE_FONT_SIZE_MAX_SP`，
         * 两份常量一旦漂移，调节条上界就与原生实际接受的区间脱节，而原生对超限值
         * 是静默丢弃——用户看到的正是「设置条范围和实际可设置范围不一致」。
         * 原生侧的合法区间改由图集边长推导（见 `FontPipeline`），不再有第二份魔数。
         */
        const val FONT_SIZE_MIN_SP = 4f
        const val FONT_SIZE_MAX_PX = 256f
        const val FONT_SIZE_STEP_SP = 2f

        /**
         * 调节条上界（sp）：把 Termux 的像素上限换算到 sp 后按步长向下取整，
         * 使 Material 调节条分出的每一档恰好相差 [FONT_SIZE_STEP_SP]，且不越过
         * Termux 的像素上限。
         *
         * @param spToPxScale sp→像素的完整系数（显示密度 × 系统字体缩放，见
         *   [TerminalRuntime.spToPxScale]）。必须用完整系数而非仅密度：字形实际
         *   光栅尺度即 `sp * spToPxScale`，只用密度会在系统「字体大小」大于 1 时
         *   放行超出 Termux 像素上限的字号（实测 fontScale=1.3 时 96sp 实际
         *   327px > 256px）。
         */
        fun fontSizeMaxSp(spToPxScale: Float): Float = floor(FONT_SIZE_MAX_PX / spToPxScale / FONT_SIZE_STEP_SP)
            .times(FONT_SIZE_STEP_SP)
            .coerceAtLeast(FONT_SIZE_MIN_SP + FONT_SIZE_STEP_SP)

        /**
         * 调节条档数（Material `steps` 语义：两端点之间的中间档数）。
         * 跨度恒被步长整除，故不会出现半档。
         */
        fun fontSizeRangeSteps(spToPxScale: Float): Int =
            ((fontSizeMaxSp(spToPxScale) - FONT_SIZE_MIN_SP) / FONT_SIZE_STEP_SP).roundToInt() - 1
    }

    val appThemeMode: Flow<String> =
        provider.dataStore.data.map { it[Keys.APP_THEME_MODE] ?: DEFAULT_FOLLOW_SYSTEM }
    private val deviceDefaultFontSize: Float
        get() = defaultFontSizeFor(provider.screenWidthDp)

    val fontSize: Flow<Float> =
        provider.dataStore.data.map { it[Keys.FONT_SIZE] ?: deviceDefaultFontSize }

    /** 用户是否已显式选定字号；全新安装时为 false。 */
    val fontSizeExplicitlySet: Flow<Boolean> =
        provider.dataStore.data.map { it[Keys.FONT_SIZE] != null }
    val fontFamily: Flow<String> = provider.dataStore.data.map { it[Keys.FONT_FAMILY] ?: "" }
    val themeName: Flow<String> = provider.dataStore.data.map { it[Keys.THEME_NAME] ?: DEFAULT_THEME }
    val dayThemeName: Flow<String> =
        provider.dataStore.data.map { it[Keys.DAY_THEME_NAME] ?: DEFAULT_DAY_THEME_NAME }
    val nightThemeName: Flow<String> =
        provider.dataStore.data.map { it[Keys.NIGHT_THEME_NAME] ?: DEFAULT_THEME }
    val themeMode: Flow<String> =
        provider.dataStore.data.map { it[Keys.THEME_MODE] ?: DEFAULT_THEME_MODE }
    val shell: Flow<String> = provider.dataStore.data.map { it[Keys.SHELL] ?: DEFAULT_SHELL }

    val bootstrapUrl: Flow<String> = provider.dataStore.data.map { it[Keys.BOOTSTRAP_URL] ?: "" }

    /**
     * 全部持久化设置的单一合并快照，由一次 DataStore 读取派生。UI 订阅这一条流，
     * 而非 13 条并行的按字段管线。字段默认值与上方的按字段流保持一致；
     * 新增设置时两者都要同步。
     */
    data class SettingsState(
        val appThemeMode: String = DEFAULT_FOLLOW_SYSTEM,
        /**
         * 无默认值：真实缺省值随屏幕宽度自适应（`deviceDefaultFontSize`），任何常量都会
         * 与 [settings] 流的实际行为不符。此前这里是 `DEFAULT_FONT_SIZE`（14sp），
         * 于是 `SettingsState()` 造出的快照在窄屏设备上谎报字号——调用方无从察觉，
         * 因为它与「用户已设为 14sp」不可区分。改为必须显式传入：
         * 与真实缺省行为一致的唯一写法就是从 [settings] 流取。
         */
        val fontSize: Float,
        val fontFamily: String = "",
        val themeName: String = DEFAULT_THEME,
        val dayThemeName: String = DEFAULT_DAY_THEME_NAME,
        val nightThemeName: String = DEFAULT_THEME,
        val themeMode: String = DEFAULT_THEME_MODE,
        val shell: String = DEFAULT_SHELL,
        val bootstrapUrl: String = "",
    )

    val settings: Flow<SettingsState> =
        provider.dataStore.data.map { prefs ->
            SettingsState(
                appThemeMode = prefs[Keys.APP_THEME_MODE] ?: DEFAULT_FOLLOW_SYSTEM,
                fontSize = prefs[Keys.FONT_SIZE] ?: deviceDefaultFontSize,
                fontFamily = prefs[Keys.FONT_FAMILY] ?: "",
                themeName = prefs[Keys.THEME_NAME] ?: DEFAULT_THEME,
                dayThemeName = prefs[Keys.DAY_THEME_NAME] ?: DEFAULT_DAY_THEME_NAME,
                nightThemeName = prefs[Keys.NIGHT_THEME_NAME] ?: DEFAULT_THEME,
                themeMode = prefs[Keys.THEME_MODE] ?: DEFAULT_THEME_MODE,
                shell = prefs[Keys.SHELL] ?: DEFAULT_SHELL,
                bootstrapUrl = prefs[Keys.BOOTSTRAP_URL] ?: "",
            )
        }

    suspend fun setFontSize(size: Float) = put(Keys.FONT_SIZE, size)

    /**
     * 首次启动时持久化按设备自适应的默认字号，使全新安装在用户触碰字号滑块前
     * 就渲染出可读的网格。用户已显式选过字号后为空操作。
     */
    suspend fun applyFirstLaunchDefaultFontSize(screenWidthDp: Float) {
        // 用户已选过字号则完全跳过写事务。
        if (fontSizeExplicitlySet.first()) return
        provider.dataStore.edit { prefs ->
            if (prefs[Keys.FONT_SIZE] == null) {
                prefs[Keys.FONT_SIZE] = defaultFontSizeFor(screenWidthDp.coerceAtLeast(0f))
            }
        }
    }

    suspend fun setFontFamily(family: String) = put(Keys.FONT_FAMILY, family)

    /** 清除无效的字体族设置（DESIGN 字体选择节：设置数据错误 → 清除设置数据）。 */
    suspend fun clearFontFamily() {
        provider.dataStore.edit { prefs ->
            prefs.remove(Keys.FONT_FAMILY)
        }
    }

    suspend fun setThemeName(name: String) = put(Keys.THEME_NAME, name)

    suspend fun setDayThemeName(name: String) = put(Keys.DAY_THEME_NAME, name)

    suspend fun setNightThemeName(name: String) = put(Keys.NIGHT_THEME_NAME, name)

    /**
     * 清除存有未知主题名的键（DESIGN 主题节：设置错误重置应用数据）。
     * 只删存错值的那个键，其余主题设置保持不变。
     */
    suspend fun clearUnknownThemeNames(unknown: Set<String>) {
        provider.dataStore.edit { prefs ->
            if (unknown.contains(prefs[Keys.THEME_NAME])) prefs.remove(Keys.THEME_NAME)
            if (unknown.contains(prefs[Keys.DAY_THEME_NAME])) prefs.remove(Keys.DAY_THEME_NAME)
            if (unknown.contains(prefs[Keys.NIGHT_THEME_NAME])) prefs.remove(Keys.NIGHT_THEME_NAME)
        }
    }

    suspend fun setThemeMode(mode: String) = put(Keys.THEME_MODE, mode)

    suspend fun setAppThemeMode(mode: String) = put(Keys.APP_THEME_MODE, mode)

    suspend fun setShell(shell: String) = put(Keys.SHELL, shell)

    suspend fun setBootstrapUrl(url: String) = put(Keys.BOOTSTRAP_URL, url)

    /**
     * 记录一次引导 URL 编辑，由本单例的防抖写入落盘。
     *
     * 防抖必须活在本单例而不是 ViewModel 里：每次写入都是完整文件重写，逐击键
     * 落盘不可接受；而放在 ViewModel 的收集器里，收集器随 ViewModel 一起被取消，
     * 防抖窗口内（[DEBOUNCE_MILLIS]）的最后一次编辑会被静默丢弃。单例的收集器与
     * 应用同寿，页面销毁带不走待写值。
     */
    fun recordBootstrapUrlEdit(url: String) {
        bootstrapUrlEdits.tryEmit(url)
    }

    /** 最近一次编辑值（含尚未落盘的），供安装动作直接读取。 */
    fun latestBootstrapUrlEdit(): String? = bootstrapUrlEdits.replayCache.lastOrNull()

    private val scope = CoroutineScope(SupervisorJob() + TerminalDispatchers.inputOutput)

    private val bootstrapUrlEdits = MutableSharedFlow<String>(replay = 1, extraBufferCapacity = 1)

    /**
     * 防抖写入与清除的互斥：已在途的 `put` 要么先完成（其产物随即被删除），
     * 要么看到停用而跳过——防抖窗口内（300ms）的编辑不可能在删除之后落地。
     */
    private val bootstrapUrlEditMutex = Mutex()

    @Volatile private var bootstrapUrlEditsArmed = true

    /**
     * 丢弃尚未落盘的引导 URL 编辑并暂停防抖写入：清除应用数据前调用，
     * 否则防抖写入会在删除之后重建 `preferences_pb`，清除静默不生效。
     * 连带清空 replay 缓存，故其后的 `latestBootstrapUrlEdit()` 不再返回已清除的值。
     */
    suspend fun dropPendingBootstrapUrlEdits() {
        bootstrapUrlEditMutex.withLock {
            bootstrapUrlEditsArmed = false
            @OptIn(kotlinx.coroutines.ExperimentalCoroutinesApi::class)
            bootstrapUrlEdits.resetReplayCache()
        }
    }

    /** 重新接受引导 URL 编辑写入：清除完成后调用，此后用户的新编辑照常落盘。 */
    suspend fun rearmBootstrapUrlEdits() {
        bootstrapUrlEditMutex.withLock { bootstrapUrlEditsArmed = true }
    }

    init {
        scope.launch {
            @OptIn(kotlinx.coroutines.FlowPreview::class)
            bootstrapUrlEdits
                .debounce(DEBOUNCE_MILLIS)
                .distinctUntilChanged()
                .collect { value ->
                    bootstrapUrlEditMutex.withLock {
                        if (bootstrapUrlEditsArmed) put(Keys.BOOTSTRAP_URL, value)
                    }
                }
        }
    }

    private suspend fun <T> put(key: Preferences.Key<T>, value: T) {
        provider.dataStore.edit { it[key] = value }
    }
}
