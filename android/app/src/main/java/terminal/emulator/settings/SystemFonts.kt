package terminal.emulator.settings

private const val SYSTEM_FONTS_XML_PATH = "/system/etc/fonts.xml"

/**
 * Pure `fonts.xml` parser: returns `<family name>` values in document
 * order, without rewriting, deduplicating or sorting. Entries without a
 * name cannot be displayed or selected, so only direct `name` attributes
 * are collected.
 */
internal fun parseFontsXmlFamilies(xml: String): List<String> {
    val families = mutableListOf<String>()
    val factory = javax.xml.parsers.DocumentBuilderFactory.newInstance()
    factory.isNamespaceAware = false
    val document = factory.newDocumentBuilder().parse(xml.byteInputStream())
    val nodes = document.getElementsByTagName("family")
    for (index in 0 until nodes.length) {
        val name = nodes.item(index).attributes?.getNamedItem("name")?.nodeValue
        if (!name.isNullOrEmpty()) {
            families.add(name)
        }
    }
    return families
}

/**
 * System font list, parsed from the platform `fonts.xml` (DESIGN 字体选择节).
 * Missing or unparseable content is fatal: log and crash, never silently
 * fall back.
 */
internal fun systemFonts(): List<String> {
    val xml =
        try {
            java.io.File(SYSTEM_FONTS_XML_PATH).readText()
        } catch (exception: Exception) {
            android.util.Log.e("SystemFonts", "Missing $SYSTEM_FONTS_XML_PATH", exception)
            throw IllegalStateException("Missing system fonts.xml", exception)
        }
    if (xml.isBlank()) {
        android.util.Log.e("SystemFonts", "Empty $SYSTEM_FONTS_XML_PATH")
        throw IllegalStateException("Empty system fonts.xml")
    }
    try {
        val families = parseFontsXmlFamilies(xml)
        if (families.isEmpty()) {
            android.util.Log.e("SystemFonts", "No families in $SYSTEM_FONTS_XML_PATH")
            throw IllegalStateException("No families in system fonts.xml")
        }
        return families
    } catch (illegal: IllegalStateException) {
        throw illegal
    } catch (exception: Exception) {
        android.util.Log.e("SystemFonts", "Unparseable $SYSTEM_FONTS_XML_PATH", exception)
        throw IllegalStateException("Unparseable system fonts.xml", exception)
    }
}
