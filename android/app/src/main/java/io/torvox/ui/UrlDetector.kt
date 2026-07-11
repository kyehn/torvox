package io.torvox.ui

import java.util.regex.Pattern

object UrlDetector {
    private val URL_PATTERN: Pattern =
        Pattern.compile(
            "(https?://[\\w\\-]+(\\.[\\w\\-]+)+([\\w\\-.,@?^=%&:/~+#]*[\\w\\-@^=%&/~+#])?)",
        )

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

    private fun trimTrailingPunctuation(url: String): String {
        var end = url.length
        while (end > 0) {
            val ch = url[end - 1]
            if (ch !in ".,;:!") break
            end--
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
