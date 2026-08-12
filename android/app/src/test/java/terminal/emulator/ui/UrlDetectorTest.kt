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

    @Test
    fun `extracts ftp url`() {
        assertEquals(listOf("ftp://files.example.com/data"), UrlDetector.findUrls("download ftp://files.example.com/data"))
    }

    @Test
    fun `extracts ssh url`() {
        assertEquals(listOf("ssh://git@github.com"), UrlDetector.findUrls("use ssh://git@github.com"))
    }

    @Test
    fun `extracts mailto url`() {
        assertEquals(listOf("mailto:user@example.com"), UrlDetector.findUrls("email mailto:user@example.com"))
    }

    @Test
    fun `extracts git url`() {
        assertEquals(listOf("git://repo.local/project"), UrlDetector.findUrls("clone git://repo.local/project"))
    }

    @Test
    fun `extracts gemini url`() {
        assertEquals(listOf("gemini://capsule.dev"), UrlDetector.findUrls("browse gemini://capsule.dev"))
    }

    @Test
    fun `trims trailing punctuation from ftp`() {
        assertEquals(listOf("ftp://files.example.com"), UrlDetector.findUrls("link ftp://files.example.com."))
    }

    @Test
    fun `trims trailing punctuation and bracket balance`() {
        // Unmatched trailing ) stripped via bracket balancing
        assertEquals(listOf("https://example.com/a"), UrlDetector.findUrls("(https://example.com/a))"))
        // Balanced parens in path are preserved
        assertEquals(listOf("https://example.com/wiki/Foo_(bar)"), UrlDetector.findUrls("see https://example.com/wiki/Foo_(bar) end"))
    }

    @Test
    fun `extracts magnet url`() {
        assertEquals(listOf("magnet:?xt=urn:abc"), UrlDetector.findUrls("open magnet:?xt=urn:abc"))
    }

    @Test
    fun `extracts ipfs and ipns urls`() {
        // zed-android-port URL_REGEX 20-protocol list (research-zed-port.md:675).
        assertEquals(
            listOf("ipfs://QmTzQ1a1b2c3d4e5f6g7h8i9j0k1l2m3n4o5p6q7r8s9t"),
            UrlDetector.findUrls("get ipfs://QmTzQ1a1b2c3d4e5f6g7h8i9j0k1l2m3n4o5p6q7r8s9t now"),
        )
        assertEquals(
            listOf("ipns://docs.ipfs.tech"),
            UrlDetector.findUrls("browse ipns://docs.ipfs.tech"),
        )
    }

    @Test
    fun `keeps query string and port`() {
        // Regression: the body class must keep `?`, `=`, `&`, and `:` so
        // queries and ports survive (must mirror native url_regex.rs).
        assertEquals(
            listOf("https://example.com:8080/a/b?q=1&x=2"),
            UrlDetector.findUrls("go https://example.com:8080/a/b?q=1&x=2 end"),
        )
    }
}
