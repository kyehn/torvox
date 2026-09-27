package terminal.emulator.util

/**
 * 判断无空白串是否为完整 URL（带协议或 www. 前缀 + 含点主机 + 可选路径）。
 * 刻意用纯 Kotlin 实现：Patterns.WEB_URL 在纯 JVM 单元测试中不可用。
 */
object UrlToken {
    private val RE =
        Regex(
            "^(?:https?://|www\\.)[\\w-]+(?:\\.[\\w-]+)+(?:[/:?#@!\$&'()*+,;=._~%\\[\\]-]\\S*)?$",
            RegexOption.IGNORE_CASE,
        )

    fun looksLikeFullUrl(s: String): Boolean = RE.matches(s)
}
