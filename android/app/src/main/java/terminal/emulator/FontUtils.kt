package terminal.emulator

private const val DEFAULT_ARCH_FALLBACK = "aarch64"

/** Style slots for independent bold/italic families (ghostty-android 4-slot). */
const val FONT_SLOT_REGULAR = -1
const val FONT_SLOT_BOLD = 0
const val FONT_SLOT_ITALIC = 1

fun resolveEffectiveFontFamily(fontFamily: String): String {
    val normalized = fontFamily.trim()
    if (normalized.isEmpty()) return ""
    return when (normalized.lowercase()) {
        "monospace", "mono", "monospaced" -> "monospace"
        "sans-serif", "sans", "sans serif" -> "sans-serif"
        "serif" -> "serif"
        else -> normalized
    }
}

fun detectArchFromAbi(): String = when (
    android.os.Build.SUPPORTED_ABIS
        .firstOrNull()
) {
    "arm64-v8a" -> "aarch64"
    "armeabi-v7a" -> "arm"
    "x86_64" -> "x86_64"
    "x86" -> "i686"
    else -> DEFAULT_ARCH_FALLBACK
}

/** True when the primary ABI is 64-bit (determines linker32/64). */
fun is64BitAbi(): Boolean = android.os.Build.SUPPORTED_ABIS
    .firstOrNull()
    ?.let { it.contains("64") } ?: false
