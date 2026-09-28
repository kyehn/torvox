package terminal.emulator.settings

import terminal.emulator.runtime.LogUtil

private const val TAG = "SystemFonts"

/**
 * 完整字体列表，只在设置页打开字体列表时获取。
 *
 * 取自渲染侧字体库：库的内容即 `/system/etc/fonts.xml` 声明的文件集加上用户投放
 * 目录 `~/.termux/fonts`（DESIGN 字体选择节），因此列表与渲染侧可选择的字体必然一致
 * —— 列表里选中的字体一定能被 `setFontFamily` 应用。
 *
 * 平台 `SystemFonts.getAvailableFonts()` 不可用于此：它返回 `Set<Font>`，而 SDK 没有
 * 公开的族名访问器（`Typeface` 无 `familyName`，`createFromFile` 无 ttc 下标重载），
 * 且未加载进渲染库的文件本就无法被选择。
 */
internal fun availableFontFamilies(rustFamilies: List<String>): List<String> {
    if (rustFamilies.isEmpty()) {
        LogUtil.e(TAG, "font database is empty")
        throw IllegalStateException("No available font families")
    }
    return rustFamilies
}
