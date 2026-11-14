package terminal.emulator.util

import org.junit.Assert.assertEquals
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner

/**
 * 日志用的 URL 来源部分：只留 `scheme://host`，路径与查询参数不得出现。
 */
@RunWith(RobolectricTestRunner::class)
class UrlOriginTest {
    @Test
    fun `origin keeps scheme and host only`() {
        assertEquals(
            "https://example.com",
            urlOrigin("https://example.com/path/to/bootstrap.zip?token=secret"),
        )
    }

    @Test
    fun `origin drops user info and port`() {
        assertEquals("https://example.com", urlOrigin("https://user:pass@example.com:8443/x"))
    }

    @Test
    fun `value without scheme reports no scheme`() {
        assertEquals("<no-scheme>", urlOrigin("example.com/bootstrap.zip"))
    }

    @Test
    fun `value without host reports no host`() {
        assertEquals("<no-host>", urlOrigin("file:///data/local/tmp/bootstrap.zip"))
    }
}
