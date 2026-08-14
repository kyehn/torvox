package terminal.emulator.util

/**
 * Whether a whitespace-free string looks like a *complete* URL token
 * (scheme- or `www.`-prefixed, a dotted host, optional path/query). Pure
 * Kotlin on purpose — `android.util.Patterns.WEB_URL`'s class initializer
 * isn't available in plain JVM unit tests. Shared by SmartCopy and
 * SelectionExpander (previously duplicated in both).
 */
object UrlToken {
    private val RE =
        Regex(
            "^(?:https?://|www\\.)[\\w-]+(?:\\.[\\w-]+)+(?:[/:?#@!\$&'()*+,;=._~%\\[\\]-]\\S*)?$",
            RegexOption.IGNORE_CASE,
        )

    fun looksLikeFullUrl(s: String): Boolean = RE.matches(s)
}
