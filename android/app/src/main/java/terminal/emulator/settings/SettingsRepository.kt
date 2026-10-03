package terminal.emulator.settings

import androidx.datastore.preferences.core.Preferences
import androidx.datastore.preferences.core.edit
import androidx.datastore.preferences.core.floatPreferencesKey
import androidx.datastore.preferences.core.stringPreferencesKey
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.MutableSharedFlow
import kotlinx.coroutines.flow.debounce
import kotlinx.coroutines.flow.distinctUntilChanged
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.flow.map
import kotlinx.coroutines.launch
import javax.inject.Inject
import javax.inject.Singleton

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
        const val DEFAULT_FONT_SIZE = 14f
        private const val DEFAULT_THEME = "Dracula Plus"
        const val DEFAULT_DAY_THEME_NAME = "Catppuccin Latte"
        const val DEFAULT_FOLLOW_SYSTEM = "follow_system"
        const val DEFAULT_THEME_MODE = "fixed"

        /** 自由文本落盘的防抖窗口：每次写入都是完整文件重写。 */
        private const val DEBOUNCE_MILLIS = 300L

        /** Shell 启动入口默认空（DESIGN :122 未设置时为空），空即走默认回退链。 */
        const val DEFAULT_SHELL = ""

        /**
         * 首次启动的按设备自适应字号（sp）：全新安装得到的尺寸能显示约
         * [DEFAULT_FONT_COLUMNS_TARGET] 个可见列（等宽字形约 0.6em 宽：sp = widthDp / (0.6 * target)），
         * 并钳位到 [MIN_FONT_SP, MAX_FONT_SP]。下限为 14sp，使小屏手机绝不会低于可读范围。
         * 已在同一模拟器上与真 termux 0.118.3 标定（1080x2400@420dpi）：
         * termux 字形带 27px / 字距 ~21.2px / ~51 列，本应用 27px / 21.8px / ~49 列
         * ——在 ±10% 容差内，无需再改。
         */
        fun defaultFontSizeFor(screenWidthDp: Float): Float = (
            screenWidthDp / DEFAULT_FONT_COLUMNS_TARGET /
                MONOSPACE_CHAR_ASPECT
            ).coerceIn(
            MIN_FONT_SP,
            MAX_FONT_SP,
        )

        private const val DEFAULT_FONT_COLUMNS_TARGET = 52f
        private const val MONOSPACE_CHAR_ASPECT = 0.6f

        /** termux default_font_size parity: never launch below 14sp. */
        const val MIN_FONT_SP = 14f
        private const val MAX_FONT_SP = 24f
    }

    val appThemeMode: Flow<String> =
        provider.dataStore.data.map { it[Keys.APP_THEME_MODE] ?: DEFAULT_FOLLOW_SYSTEM }
    private val deviceDefaultFontSize: Float
        get() = defaultFontSizeFor(provider.screenWidthDp)

    val fontSize: Flow<Float> =
        provider.dataStore.data.map { it[Keys.FONT_SIZE] ?: deviceDefaultFontSize }

    /** True once the user has explicitly picked a font size; false on a fresh install. */
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

    /** Clears an invalid font family setting (DESIGN 字体选择节: 设置错误重置应用数据). */
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

    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.IO)

    private val bootstrapUrlEdits = MutableSharedFlow<String>(replay = 1, extraBufferCapacity = 1)

    init {
        scope.launch {
            @OptIn(kotlinx.coroutines.FlowPreview::class)
            bootstrapUrlEdits
                .debounce(DEBOUNCE_MILLIS)
                .distinctUntilChanged()
                .collect { value -> put(Keys.BOOTSTRAP_URL, value) }
        }
    }

    private suspend fun <T> put(key: Preferences.Key<T>, value: T) {
        provider.dataStore.edit { it[key] = value }
    }
}
