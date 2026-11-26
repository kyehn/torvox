package terminal.emulator.bridge

import terminal.emulator.settings.ColorsProperties
import terminal.emulator.settings.TerminalCursorStyle

/**
 * 原生接受的光栅缩放（sp→px 系数）区间：原生为唯一真源（`NativeBridge.getRasterScaleRange`）。
 *
 * 此前此处另存一份 `0.5f..8f` 字面量，与 Rust `setRasterScale` 的守卫只由注释绑定——
 * 与字号上界那次同型的第二处重复：两端一旦漂移，[coerceSpToPxScale] 会把原生会拒收的
 * 系数当作合法值，而字号上界（`SettingsRepository.fontSizeMaxSp` 以系数为分母）
 * 随之用错系数算，重新出现「设置条范围与实际可设置范围不一致」。
 */
internal val rasterScaleRange: ClosedFloatingPointRange<Float> by lazy {
    // 可空签名与 `listFontFamilies` 同款：原生的失败值是 null，让它以 null 到达
    // 调用方并在此处被指名道姓地拒绝，好过在 `bounds.size` 上抛一个无来由的 NPE。
    // 不设兜底默认值：钳到一个猜测值只会让字号上界与原生实际接受区间悄悄脱节，
    // 正是本属性存在的理由所反对的。
    val bounds =
        requireNotNull(NativeBridge.getRasterScaleRange()) {
            "原生未返回光栅缩放区间（JNI 调用失败）"
        }
    require(bounds.size == 2 && bounds.all { it.isFinite() && it > 0f } && bounds[0] <= bounds[1]) {
        "原生未给出合法的光栅缩放区间: ${bounds.contentToString()}"
    }
    bounds[0]..bounds[1]
}

/**
 * 一条会话的完整渲染契约：主题 + 字体族 + 字号 + 光栅尺度。
 *
 * 两条下发路径（建会话后的 `applyRenderSettings`、配置变化后的 `applySettings`）
 * 共用本契约：漏掉 `setRasterScale` 的那一条既不编译失败也不影响字号本身，
 * 症状只是系统「字体大小」变化后 Kotlin 按新系数算、原生仍按旧值光栅——
 * 字形被拉伸，单元格度量与网格整体偏掉一个 fontScale 倍。
 */
internal data class RenderContract(
    val fontSizeTenths: Int,
    val fontFamily: String,
    val themeName: String,
    val rasterScale: Float,
    /** `termux.properties` 的 `terminal-cursor-style`，取 `TerminalCursorStyle.nativeValue`。 */
    val cursorStyleNativeValue: Int = TerminalCursorStyle.BLOCK.nativeValue,
    /**
     * `colors.properties` 的调色板覆盖槽（19 个，未覆盖的槽为负值）。
     *
     * 随主题一起下发：覆盖是**全局配置**，不随主题切换消失，故每次换主题都带上它。
     * 默认值是「全部槽不覆盖」——原生按固定长度解包，槽数不对会被拒收。
     */
    val themeOverrides: IntArray = IntArray(ColorsProperties.PALETTE_OVERRIDE_SLOTS) { ColorsProperties.NO_OVERRIDE },
)

/**
 * 把整组渲染契约下发到一条会话。
 *
 * 契约与下发动作同处一地：漏项的代价是静默的字形错位（见 [RenderContract]），
 * 故「契约有哪些字段」与「契约怎么落到原生」由同一处回答。
 */
internal fun Bridge.applyRenderContract(contract: RenderContract) {
    setTheme(contract.themeName, contract.themeOverrides)
    setCursorStyle(contract.cursorStyleNativeValue)
    applyFontContract(contract.fontFamily, contract.fontSizeTenths, contract.rasterScale)
}

/**
 * 字体契约（字族 + 字号 + 光栅尺度）：三项都是**全局**量，原生侧只落在唯一的
 * 字体管线上，与会话无关。
 *
 * 独立成函数是因为它有两个调用时机，且两处必须给同一组值：
 * 建会话前的 spawn（此时定下的字体度量决定 PTY 的初始 winsize）与建会话后的
 * 契约下发（主题等其余项此时才可下发）。
 */
internal fun Bridge.applyFontContract(fontFamily: String, fontSizeTenths: Int, rasterScale: Float) {
    setFontFamily(fontFamily)
    setFontSizeInPlace(fontSizeTenths)
    setRasterScale(rasterScale)
}

/**
 * sp→设备像素的完整系数 = 显示密度 × 系统字体缩放，钳到原生接受的区间。
 *
 * 顶层纯函数以便测试：字号的每一次 sp↔px 换算都经此系数，钳位区间 MUST 与
 * 原生 `setRasterScale` 一致——区间外的值被原生拒收并记错误日志，会使字号上界
 * 与实际渲染脱节。区间由 [rasterScaleRange] 提供，故纯函数不直接碰 JNI。
 */
internal fun coerceSpToPxScale(density: Float, fontScale: Float, range: ClosedFloatingPointRange<Float>): Float =
    (density * fontScale).coerceIn(range.start, range.endInclusive)
