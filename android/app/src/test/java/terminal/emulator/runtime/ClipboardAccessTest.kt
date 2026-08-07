package terminal.emulator.runtime

import android.content.Context
import androidx.test.core.app.ApplicationProvider
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Before
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner

/**
 * Round-225 T3 — CopyAccess smart-copy processor semantics
 * (Haven SmartTerminalClipboard:407-430):
 *   1. a non-null processor transforms the written text;
 *   2. a null/blank processor result falls back to the caller's text
 *      (drift guard — never clobber the clipboard with empty);
 *   3. the default processor is null → verbatim (OSC 52 path).
 */
@RunWith(RobolectricTestRunner::class)
class ClipboardAccessTest {

    private lateinit var access: ClipboardAccess

    @Before
    fun setUp() {
        val context = ApplicationProvider.getApplicationContext<Context>()
        access = ClipboardAccess(context, "Test")
    }

    @Test
    fun `default processor is null - verbatim write`() {
        access.setClipboardText("osc-52-content")
        assertEquals(
            "osc-52-content",
            access.clipboardText(),
        )
    }

    @Test
    fun `processor transforms written text`() {
        access.smartCopyProcessor = { text -> text.uppercase() }
        access.setClipboardText("border panel")
        assertEquals("BORDER PANEL", access.clipboardText())
    }

    @Test
    fun `null processor result falls back to caller text`() {
        access.smartCopyProcessor = { null }
        access.setClipboardText("drifted snapshot")
        assertEquals("drifted snapshot", access.clipboardText())
    }

    @Test
    fun `blank processor result falls back to caller text`() {
        access.smartCopyProcessor = { "" }
        access.setClipboardText("keep me")
        assertEquals("keep me", access.clipboardText())
    }

    @Test
    fun `processor cleared restores verbatim`() {
        access.smartCopyProcessor = { it.replace("sc", "processed") }
        access.setClipboardText("sc")
        assertEquals("processed", access.clipboardText())
        access.smartCopyProcessor = null
        access.setClipboardText("sc")
        assertEquals("sc", access.clipboardText())
    }

    @Test
    fun `clipboard label is preserved`() {
        access.setClipboardText("content", label = "custom label")
        val manager = ApplicationProvider.getApplicationContext<Context>()
            .getSystemService(Context.CLIPBOARD_SERVICE) as android.content.ClipboardManager
        assertEquals("custom label", manager.primaryClip?.description?.label)
        assertEquals("content", manager.primaryClip?.getItemAt(0)?.text?.toString())
    }

    @Test
    fun `empty clipboard reports null`() {
        assertNull(access.clipboardText())
    }
}
