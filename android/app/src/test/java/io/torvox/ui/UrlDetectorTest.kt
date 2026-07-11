package io.torvox.ui

import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

class UrlDetectorTest {
    @Test
    fun findUrls_detectsHttpUrl() {
        val text = "Visit http://example.com for more info"
        val urls = UrlDetector.findUrls(text)
        assertEquals(1, urls.size)
        assertTrue(urls[0].contains("http://example.com"))
    }

    @Test
    fun findUrls_detectsHttpsUrl() {
        val text = "Go to https://github.com/torvox"
        val urls = UrlDetector.findUrls(text)
        assertEquals(1, urls.size)
        assertTrue(urls[0].contains("https://github.com/torvox"))
    }

    @Test
    fun findUrls_detectsMultipleUrls() {
        val text = "See http://a.com and https://b.org/path"
        val urls = UrlDetector.findUrls(text)
        assertEquals(2, urls.size)
    }

    @Test
    fun findUrls_trimsTrailingPunctuation() {
        val text = "Visit http://example.com."
        val urls = UrlDetector.findUrls(text)
        assertEquals(1, urls.size)
        assertEquals("http://example.com", urls[0])
    }

    @Test
    fun findUrls_noUrls() {
        val text = "Hello world, no URLs here!"
        val urls = UrlDetector.findUrls(text)
        assertTrue(urls.isEmpty())
    }

    @Test
    fun findUrls_trimsTrailingCommaAndSemicolon() {
        val text = "url: http://example.com/path,;:!"
        val urls = UrlDetector.findUrls(text)
        assertEquals(1, urls.size)
        assertEquals("http://example.com/path", urls[0])
    }

    @Test
    fun findUrls_preservesPlusInUrl() {
        val text = "Search https://example.com/search?q=a+b"
        val urls = UrlDetector.findUrls(text)
        assertEquals(1, urls.size)
        assertEquals("https://example.com/search?q=a+b", urls[0])
    }

    @Test
    fun findUrls_decodesPercentEncodedChars() {
        val text = "See https://example.com/%E4%B8%AD%E6%96%87"
        val urls = UrlDetector.findUrls(text)
        assertEquals(1, urls.size)
        assertEquals("https://example.com/中文", urls[0])
    }

    @Test
    fun findUrls_preservesPlusAndDecodesPercent() {
        val text = "Visit https://en.wikipedia.org/wiki/C%2B%2B"
        val urls = UrlDetector.findUrls(text)
        assertEquals(1, urls.size)
        assertEquals("https://en.wikipedia.org/wiki/C++", urls[0])
    }

    @Test
    fun findUrls_noPercentNoDecode() {
        val text = "Go to https://example.com/simple"
        val urls = UrlDetector.findUrls(text)
        assertEquals(1, urls.size)
        assertEquals("https://example.com/simple", urls[0])
    }
}
