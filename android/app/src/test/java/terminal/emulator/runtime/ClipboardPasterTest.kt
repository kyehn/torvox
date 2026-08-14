package terminal.emulator.runtime

import android.content.Context
import androidx.test.core.app.ApplicationProvider
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner

/**
 * ClipboardPaster end-to-end: clipboard read → PasteChunker → sink.
 */
@RunWith(RobolectricTestRunner::class)
class ClipboardPasterTest {

    private lateinit var access: ClipboardAccess

    @Before
    fun setUp() {
        val context = ApplicationProvider.getApplicationContext<Context>()
        access = ClipboardAccess(context, "ClipboardPasterTest")
    }

    @Test
    fun `paste forwards every chunk and returns the character count`() {
        access.setClipboardText("hello world")
        val paster = ClipboardPaster(access)
        val received = mutableListOf<ByteArray>()
        val count = paster.pasteTo { received.add(it) }
        assertTrue("paste must reach the sink", received.isNotEmpty())
        val joined = received.joinToString("") { String(it) }
        assertEquals("hello world", joined)
        assertEquals("count is the clipboard length", 11, count)
    }

    @Test
    fun `long clipboard is split into multiple chunks`() {
        val longText = "a".repeat(10_000)
        access.setClipboardText(longText)
        val chunker = PasteChunker(chunkChars = 4_000)
        val paster = ClipboardPaster(access, chunker)
        val received = mutableListOf<ByteArray>()
        val count = paster.pasteTo { received.add(it) }
        assertTrue("10k chars with 4k chunks must yield >1 chunk", received.size > 1)
        assertEquals(longText.length, count)
        assertEquals(longText, received.joinToString("") { String(it) })
    }

    @Test
    fun `newlines are normalized to carriage returns`() {
        access.setClipboardText("line1\nline2")
        val paster = ClipboardPaster(access)
        val received = mutableListOf<ByteArray>()
        paster.pasteTo { received.add(it) }
        assertEquals("line1\rline2", received.joinToString("") { String(it) })
    }

    @Test
    fun `blank clipboard yields no sink calls and zero count`() {
        access.setClipboardText("   ")
        val paster = ClipboardPaster(access)
        val received = mutableListOf<ByteArray>()
        val count = paster.pasteTo { received.add(it) }
        assertEquals(0, received.size)
        assertEquals(0, count)
    }
}
