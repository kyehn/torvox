package terminal.emulator.ui

/**
 * 搜索查询串规整：上限内原样返回，超长返回 null（调用方判无命中，不送原生）。
 *
 * MUST NOT 在此截断：截断会让用户搜到另一个串的结果（`text-search` 查询上限条）。
 * 字符数按码点计，与原生 `chars().count()` 同口径，而非 UTF-16 单元数。
 */
fun coerceSearchQuery(query: String, maxChars: Int): String? {
    require(maxChars >= 0) { "搜索上限非法：$maxChars" }
    return if (query.codePointCount(0, query.length) > maxChars) null else query
}
