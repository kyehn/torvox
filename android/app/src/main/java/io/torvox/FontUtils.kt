package io.torvox

private const val DEFAULT_ARCH_FALLBACK = "aarch64"

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
