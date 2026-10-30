package terminal.emulator.ui.theme

import kotlinx.collections.immutable.ImmutableList
import kotlinx.collections.immutable.toImmutableList
import terminal.emulator.bridge.NativeBridge

/**
 * 内置终端主题仓库：单源在 Rust 侧（构建期嵌入的 alacritty 主题），
 * 经原生直接量一次取回整个目录，不在 Kotlin 侧另存名字与颜色副本，不经 JSON。
 */
object ThemeRepository {
    /**
     * 背景、前景与十六色，即设置侧真正消费的全部颜色。
     *
     * 直接取 [TerminalTheme.PACKED_COLOR_BYTES] 而非在此重算：两处各自写一遍
     * `(2 + 16) * 3`，任一处改了另一处不改，解析偏移就整体错位，而错位表现为
     * 主题颜色串位（不报错、只是颜色不对）。打包侧是唯一真源。
     */
    private const val ENTRY_COLOR_BYTES = TerminalTheme.PACKED_COLOR_BYTES

    /** 目录长度（`u16` 小端）。 */
    private const val CATALOG_COUNT_BYTES = 2

    /** 单条的固定开销：标志 1 + 名称长度 1。 */
    private const val ENTRY_FIXED_BYTES = 2

    internal const val FLAG_DEFAULT_DARK = 1
    internal const val FLAG_DEFAULT_LIGHT = 2

    /** 一次查询取回的目录：主题清单（深底在前、日间在后）与两个默认主题标识。 */
    internal class ThemeCatalog(
        val themes: List<TerminalTheme>,
        val defaultDarkName: String,
        val defaultLightName: String,
    )

    /**
     * 进程内缓存的目录。主题在构建期定进二进制、进程生命周期内不变，而解析要经
     * JNI 取回整个目录；调用方若用 `remember` 持有，列表项滚出屏幕再滚回就会重取
     * 一轮。解析失败即抛：目录取不到或字节不完整都是产品缺陷，不退回空列表。
     */
    private val catalog: ThemeCatalog by lazy { parseCatalog(requireCatalog()) }

    private fun requireCatalog(): ByteArray =
        NativeBridge.getThemeCatalog() ?: error("Theme catalog unavailable from native")

    /**
     * 目录字节 → 主题清单与默认标识。纯函数，不碰 JNI，可直接单测。
     * 长度不符即抛，不截断、不补默认色。
     */
    internal fun parseCatalog(packed: ByteArray): ThemeCatalog {
        fun requireRange(offset: Int, length: Int, what: String) {
            require(offset + length <= packed.size) { "Theme catalog is truncated before $what" }
        }

        requireRange(0, CATALOG_COUNT_BYTES, "catalog count")
        val themeCount = readU16(packed, 0)
        val themes = ArrayList<TerminalTheme>(themeCount)
        var defaultDarkName: String? = null
        var defaultLightName: String? = null
        var offset = CATALOG_COUNT_BYTES
        repeat(themeCount) {
            requireRange(offset, ENTRY_FIXED_BYTES, "theme flags")
            val flags = packed[offset].toInt() and 0xFF
            val nameLength = packed[offset + 1].toInt() and 0xFF
            offset += ENTRY_FIXED_BYTES
            requireRange(offset, nameLength, "theme name")
            val themeName = String(packed, offset, nameLength, Charsets.UTF_8)
            offset += nameLength
            requireRange(offset, ENTRY_COLOR_BYTES, "theme colors")
            val colors = packed.copyOfRange(offset, offset + ENTRY_COLOR_BYTES)
            offset += ENTRY_COLOR_BYTES
            if (flags and FLAG_DEFAULT_DARK != 0) defaultDarkName = themeName
            if (flags and FLAG_DEFAULT_LIGHT != 0) defaultLightName = themeName
            themes += TerminalTheme.fromPackedColors(themeName, colors)
        }
        require(offset == packed.size) { "Theme catalog has ${packed.size - offset} trailing bytes" }
        return ThemeCatalog(
            themes = themes,
            defaultDarkName =
            requireNotNull(defaultDarkName) { "Theme catalog names no default dark theme" },
            defaultLightName =
            requireNotNull(defaultLightName) { "Theme catalog names no default light theme" },
        )
    }

    private fun readU16(packed: ByteArray, offset: Int): Int =
        (packed[offset].toInt() and 0xFF) or ((packed[offset + 1].toInt() and 0xFF) shl 8)

    /** 全部主题，深底在前、日间在后（与原生顺序一致）。 */
    fun all(): List<TerminalTheme> = catalog.themes

    /**
     * 进程内缓存的 [all]。主题在构建期定进二进制，进程生命周期内不变，而取目录要
     * 经 JNI；调用方若用 `remember` 持有，列表项滚出屏幕再滚回就会重取一轮。
     */
    val allCached: ImmutableList<TerminalTheme> by lazy { all().toImmutableList() }

    /**
     * 按名取主题。未知返回 null（设置数据错误，由调用方清除该键），
     * 不做静默替换。
     */
    fun byNameOrNull(themeName: String): TerminalTheme? = catalog.themes.find { it.name == themeName }

    /** 按名取主题，未知抛异常，不静默替换默认主题。 */
    fun byName(themeName: String): TerminalTheme =
        byNameOrNull(themeName) ?: error("unknown terminal theme: $themeName")

    /** 默认夜间主题标识，对应默认终端主题。 */
    fun defaultDarkName(): String = catalog.defaultDarkName

    /** 默认日间主题标识。 */
    fun defaultLightName(): String = catalog.defaultLightName
}
