package terminal.emulator.ui

import java.util.regex.Pattern

object UrlDetector {
    /**
     * URL patterns matching 27 protocols — mirrors Rust url_regex.rs exactly.
     *
     * Uses negated character classes ([^\s<>,...]) like the Rust version instead
     * of whitelists, so it naturally handles @ in ssh://git@github.com, colons
     * in data: URIs, etc. without needing per-char enumeration.
     *
     * Reference: native/src/terminal/url_regex.rs SCHEMES_SLASH + SCHEMES_COLON.
     * SelectionExpander.findUrlStart is scheme-agnostic (looks for `://`);
     * this pattern is used for text content scanning (long-press detection).
     */
    private val URL_PATTERN: Pattern = buildUrlPattern()

    private fun buildUrlPattern(): Pattern {
        val schemesSlash =
            "https?|ftp|ftps|file|data|ssh|git|svn|hg|sftp|scp|irc|ircs|gemini|gopher|news|nntp|ed2k|steam|skype|xmpp"
        // ipfs:/ipns: per zed-android-port URL_REGEX (research-zed-port.md:675).
        val schemesColon = "mailto|tel|sms|callto|magnet|ipfs|ipns"
        // Characters excluded from URL body: whitespace, angle brackets, comma,
        // semicolon, exclamation, single/double quotes, and backslash.
        // Dot (.) kept for domain names. Colon (:) kept for nested schemes (urn:).
        // Parens () kept for paths like /wiki/Foo_(bar). Question mark (?) and
        // equals (=) kept for query params (?q=1&x=2).
        // trimTrailingPunctuation handles trailing cleanup.
        val body = "[^\\s<>,;!'\"\\\\]*"
        return Pattern.compile("(?:(?:$schemesSlash)://$body)|(?:(?:$schemesColon):$body)")
    }

    fun findUrls(text: String): List<String> {
        val matcher = URL_PATTERN.matcher(text)
        val urls = mutableListOf<String>()
        while (matcher.find()) {
            var url = matcher.group() ?: continue
            url = trimTrailingPunctuation(url)
            url = percentDecode(url)
            if (url.isNotBlank()) {
                urls.add(url)
            }
        }
        return urls
    }

    /**
     * Find URLs with caching by scroll position.
     * Avoids re-scanning the same text region.
     */
    fun findUrlsCached(text: String, scrollOffset: Int): List<String> = UrlCache.getOrCompute(scrollOffset) { findUrls(text) }

    private fun trimTrailingPunctuation(url: String): String {
        var end = url.length
        while (end > 0) {
            val ch = url[end - 1]
            if (ch !in ".,;:!'\"") break
            end--
        }
        // Balance unmatched closing brackets (reference: url_regex.rs clean_url)
        // Count parens/brackets: ( = +1, ) = -1. Negative surplus means
        // unmatched trailing close chars that should be stripped.
        var closeParens = 0
        var closeBrackets = 0
        for (i in 0 until end) {
            when (url[i]) {
                '(' -> closeParens++
                ')' -> closeParens--
                '[' -> closeBrackets++
                ']' -> closeBrackets--
            }
        }
        while (end > 0 && closeParens < 0 && url[end - 1] == ')') {
            end--
            closeParens++
        }
        while (end > 0 && closeBrackets < 0 && url[end - 1] == ']') {
            end--
            closeBrackets++
        }
        return url.substring(0, end)
    }

    private fun percentDecode(url: String): String {
        if (!url.contains('%')) return url
        val bytes = mutableListOf<Byte>()
        val sb = StringBuilder(url.length)
        var i = 0
        while (i < url.length) {
            val ch = url[i]
            if (ch == '%' && i + 2 < url.length) {
                val hex = url.substring(i + 1, i + 3)
                val decoded = hex.toIntOrNull(16)
                if (decoded != null) {
                    bytes.add(decoded.toByte())
                    i += 3
                    continue
                }
            }
            if (bytes.isNotEmpty()) {
                sb.append(String(bytes.toByteArray(), Charsets.UTF_8))
                bytes.clear()
            }
            sb.append(ch)
            i++
        }
        if (bytes.isNotEmpty()) {
            sb.append(String(bytes.toByteArray(), Charsets.UTF_8))
        }
        return sb.toString()
    }
}
