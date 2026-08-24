package terminal.emulator.util

import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * UrlToken.looksLikeFullUrl — the full-URL regex shared by SmartCopy and
 * SelectionExpander. SelectionExpanderTest/SmartCopyTest cover a few
 * slice samples; this file pins the boundary table so a regex edit cannot
 * silently change what counts as a tappable URL.
 */
class UrlTokenTest {

    // ── positives ────────────────────────────────────────────────────

    @Test
    fun `http scheme accepted`() {
        assertTrue(UrlToken.looksLikeFullUrl("http://example.com"))
        assertTrue(UrlToken.looksLikeFullUrl("http://example.com/path"))
    }

    @Test
    fun `https scheme accepted`() {
        assertTrue(UrlToken.looksLikeFullUrl("https://example.com"))
        assertTrue(UrlToken.looksLikeFullUrl("https://example.com/a/b?q=1#frag"))
    }

    @Test
    fun `www dot prefix accepted without scheme`() {
        assertTrue(UrlToken.looksLikeFullUrl("www.example.com"))
        assertTrue(UrlToken.looksLikeFullUrl("www.example.com/path"))
    }

    @Test
    fun `scheme matching is case insensitive`() {
        assertTrue(UrlToken.looksLikeFullUrl("HTTPS://example.com"))
        assertTrue(UrlToken.looksLikeFullUrl("WWW.example.com"))
    }

    @Test
    fun `port query fragment and path dots accepted`() {
        assertTrue(UrlToken.looksLikeFullUrl("https://example.com:8080/a.b/c"))
        assertTrue(UrlToken.looksLikeFullUrl("https://example.com/?q=a%20b"))
        assertTrue(UrlToken.looksLikeFullUrl("https://example.com/#sec.1"))
    }

    @Test
    fun `short top-level domain accepted`() {
        assertTrue(UrlToken.looksLikeFullUrl("https://example.co"))
    }

    @Test
    fun `subdomains and at-sign in path accepted`() {
        assertTrue(UrlToken.looksLikeFullUrl("https://a.b.c.example.com/x"))
        // '@' is allowed in the path charset but not in the host part.
        assertTrue(UrlToken.looksLikeFullUrl("https://example.com/@user"))
    }

    // ── negatives ────────────────────────────────────────────────────

    @Test
    fun `no scheme and no www prefix rejected`() {
        assertFalse(UrlToken.looksLikeFullUrl("example.com"))
        assertFalse(UrlToken.looksLikeFullUrl("localhost"))
    }

    @Test
    fun `single-label host without dot rejected`() {
        assertFalse(UrlToken.looksLikeFullUrl("https://localhost"))
        assertFalse(UrlToken.looksLikeFullUrl("https://example"))
    }

    @Test
    fun `non-http scheme rejected`() {
        assertFalse(UrlToken.looksLikeFullUrl("ftp://example.com"))
        assertFalse(UrlToken.looksLikeFullUrl("file:///etc/passwd"))
        assertFalse(UrlToken.looksLikeFullUrl("ssh://host/path"))
    }

    @Test
    fun `whitespace inside token rejected`() {
        assertFalse(UrlToken.looksLikeFullUrl("https://exa mple.com"))
        assertFalse(UrlToken.looksLikeFullUrl("www.example .com"))
    }

    @Test
    fun `surrounding text rejected`() {
        assertFalse(UrlToken.looksLikeFullUrl("visit https://example.com"))
        assertFalse(UrlToken.looksLikeFullUrl("https://example.com now"))
    }

    @Test
    fun `empty string rejected`() {
        assertFalse(UrlToken.looksLikeFullUrl(""))
    }

    @Test
    fun `ip literal without scheme rejected`() {
        assertFalse(UrlToken.looksLikeFullUrl("192.168.1.1"))
    }
}
