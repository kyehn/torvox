package terminal.emulator

private const val DEFAULT_ARCH_FALLBACK = "aarch64"

/** `.termux` dir under the Termux home (DESIGN 用户数据节): user fonts live here. */
internal fun termuxDir(context: android.content.Context): java.io.File =
    java.io.File(java.io.File(context.filesDir, "home"), ".termux")

/** User font drop-in dir (DESIGN 字体选择节): scanned into the font list, never copied. */
internal fun termuxFontDir(context: android.content.Context): java.io.File = java.io.File(termuxDir(context), "font")

/** `font.ttf` (or `.ttc` / `.otf`) override (DESIGN 字体选择节): present means default. */
internal fun termuxDefaultFontFile(homePath: String): java.io.File? = listOf("font.ttf", "font.ttc", "font.otf")
    .map { java.io.File(java.io.File(homePath, ".termux"), it) }
    .firstOrNull { it.isFile }

/** Context-based overload of [termuxDefaultFontFile]. */
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

/** 仅支持 arm64-v8a 与 x86_64（见 docs/specification/BUILD.md），其余一律回退到 aarch64。 */
fun detectArchFromAbi(): String = when (android.os.Build.SUPPORTED_ABIS.firstOrNull()) {
    "x86_64" -> "x86_64"
    else -> DEFAULT_ARCH_FALLBACK
}
