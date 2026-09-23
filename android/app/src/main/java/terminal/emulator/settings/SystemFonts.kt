package terminal.emulator.settings

private const val SYSTEM_FONTS_XML_PATH = "/system/etc/fonts.xml"

/**
 * 纯 `fonts.xml` 解析：按文档顺序返回 `<family name>`，不改写、不去重、不排序。
 * 无名条目无法展示与选择，只收直接 `name` 属性。
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
 * 系统字体列表，解析平台 `fonts.xml`（DESIGN 字体选择节）。
 * 缺失或不可解析即致命：记录日志并崩溃，永不静默回退。
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
    val families =
        try {
            parseFontsXmlFamilies(xml)
        } catch (exception: Exception) {
            android.util.Log.e("SystemFonts", "Unparseable $SYSTEM_FONTS_XML_PATH", exception)
            throw IllegalStateException("Unparseable system fonts.xml", exception)
        }
    if (families.isEmpty()) {
        android.util.Log.e("SystemFonts", "No families in $SYSTEM_FONTS_XML_PATH")
        throw IllegalStateException("No families in system fonts.xml")
    }
    return families
}
