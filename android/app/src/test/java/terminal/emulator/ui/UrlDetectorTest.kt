package terminal.emulator.ui

import org.junit.Assert.assertEquals
import org.junit.Test

class UrlDetectorTest {
    @Test
    fun `extracts plain http url`() {
        assertEquals(listOf("https://example.com"), UrlDetector.findUrls("visit https://example.com now"))
    }

    @Test
    fun `extracts url with path and query`() {
        assertEquals(
            listOf("https://example.com/a/b?q=1&x=2"),
            UrlDetector.findUrls("see https://example.com/a/b?q=1&x=2 end"),
        )
    }

    @Test
    fun `trims trailing punctuation`() {
        assertEquals(listOf("https://example.com"), UrlDetector.findUrls("link https://example.com."))
        assertEquals(listOf("https://example.com/a"), UrlDetector.findUrls("(https://example.com/a)"))
    }

    @Test
    fun `percent decodes url`() {
        assertEquals(listOf("https://example.com/你好"), UrlDetector.findUrls("https://example.com/%E4%BD%A0%E5%A5%BD"))
    }

    @Test
    fun `no urls in plain text`() {
        assertEquals(emptyList<String>(), UrlDetector.findUrls("no links here"))
    }

    @Test
    fun `extracts multiple urls`() {
        assertEquals(
            listOf("http://a.com", "http://b.com"),
            UrlDetector.findUrls("one http://a.com two http://b.com"),
        )
    }
}
