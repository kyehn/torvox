package terminal.emulator.settings

/** Cleans a font file name into a display label: `_`/`-` become spaces and
 *  surrounding whitespace is trimmed. Returns `null` when the result is empty
 *  so callers can skip the entry entirely. */
internal fun cleanFontName(fileNameWithoutExtension: String): String? = fileNameWithoutExtension
    .replace('_', ' ')
    .replace('-', ' ')
    .trim()
    .ifEmpty { null }

internal fun systemFonts(): List<String> {
    val fonts = mutableListOf<String>()
    val seen = mutableSetOf<String>()

    // Scan well-known system font directories
    val fontDirectories =
        listOf(
            "/system/fonts/",
            "/product/fonts/",
            "/vendor/fonts/",
        )
    for (dirPath in fontDirectories) {
        val directory = java.io.File(dirPath)
        if (directory.isDirectory) {
            directory
                .listFiles()
                ?.filter { it.name.endsWith(".ttf", true) || it.name.endsWith(".otf", true) }
                ?.forEach { file ->
                    val name = cleanFontName(file.nameWithoutExtension) ?: return@forEach
                    if (seen.add(name.lowercase())) {
                        fonts.add(name)
                    }
                }
        }
    }

    // If no fonts found via directory scan, use Typeface to check known system families
    if (fonts.isEmpty()) {
        val knownFamilies =
            listOf(
                "sans-serif",
                "serif",
                "monospace",
                "sans-serif-light",
                "sans-serif-medium",
                "sans-serif-condensed",
            )
        for (family in knownFamilies) {
            try {
                android.graphics.Typeface.create(family, android.graphics.Typeface.NORMAL)
                seen.add(family.lowercase())
                fonts.add(family)
            } catch (_: RuntimeException) {
                // Not running on Android (unit test with stubs)
            }
        }
    }

    for (
    known in
    listOf(
        "JetBrainsMono Nerd Font",
        "Droid Sans Mono",
        "Noto Sans Mono",
        "Noto Sans SC",
        "Noto Sans CJK SC",
        "Noto Sans TC",
        "Noto Sans CJK TC",
        "Noto Sans JP",
        "Noto Sans KR",
        "DroidSansFallback",
        "Roboto Mono",
        "Source Code Pro",
        "Fira Code",
        "Ubuntu Mono",
    )
    ) {
        if (seen.add(known.lowercase())) {
            fonts.add(known)
        }
    }
    fonts.sort()
    return fonts
}
