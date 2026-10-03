package terminal.emulator

/** `.termux` dir under the Termux home (DESIGN 用户数据节): user fonts live here. */
internal fun termuxDir(context: android.content.Context): java.io.File =
    java.io.File(java.io.File(context.filesDir, "home"), ".termux")

/** 用户字体目录 `~/.termux/fonts`（DESIGN:99）：存在时并入字体扫描路径，
 *  其中的字体出现在字体列表里。从不复制/移动文件。 */
internal fun termuxFontDir(context: android.content.Context): java.io.File = java.io.File(termuxDir(context), "fonts")

/** 主字体覆盖 `~/.termux/font.ttf`（DESIGN:97，ttc/otf 同理）：存在即设为主字体。 */
internal fun termuxDefaultFontFile(homePath: String): java.io.File? = listOf("font.ttf", "font.ttc", "font.otf")
    .map { java.io.File(java.io.File(homePath, ".termux"), it) }
    .firstOrNull { it.isFile }

/** [termuxDefaultFontFile] 的 Context 重载。 */
internal fun termuxDefaultFontFile(context: android.content.Context): java.io.File? =
    termuxDefaultFontFile(java.io.File(context.filesDir, "home").absolutePath)

fun resolveEffectiveFontFamily(fontFamily: String): String {
    val normalized = fontFamily.trim()
    if (normalized.isEmpty()) return ""
    return when (normalized.lowercase()) {
        "monospace",
        "mono",
        "monospaced",
        -> "monospace"

        "sans-serif",
        "sans",
        "sans serif",
        -> "sans-serif"

        "serif" -> "serif"

        else -> normalized
    }
}

/** 仅支持 arm64-v8a 与 x86_64（见 docs/specification/BUILD.md）：其余 ABI 直接抛，
 * 使引导安装失败返回而非静默下载错误架构的 zip（后者再被安装成功态掩盖）。 */
fun detectArchFromAbi(): String = when (val abi = android.os.Build.SUPPORTED_ABIS.firstOrNull()) {
    "arm64-v8a" -> "aarch64"
    "x86_64" -> "x86_64"
    else -> error("不支持的 ABI（仅 arm64-v8a/x86_64）：$abi")
}
