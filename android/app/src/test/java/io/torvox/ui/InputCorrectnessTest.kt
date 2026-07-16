package io.torvox.ui

import android.view.inputmethod.EditorInfo
import android.view.inputmethod.InputConnection
import androidx.test.core.app.ApplicationProvider
import androidx.test.ext.junit.runners.AndroidJUnit4
import io.mockk.every
import io.mockk.mockk
import io.torvox.TerminalState
import io.torvox.TerminalViewModel
import kotlinx.coroutines.flow.MutableStateFlow
import org.junit.Assert.assertArrayEquals
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.Shadows.shadowOf

/**
 * Comprehensive input correctness tests for the IME input path.
 *
 * Covers [InputCoalescer], [TerminalInputEncoder], and the composing/commit
 * reconciliation inside [TerminalSurface]'s [android.view.inputmethod.BaseInputConnection].
 *
 * Categories:
 *   1. InputCoalescer — buffering, dedup, bypass, reset
 *   2. TerminalInputEncoder — text encoding with modifiers, bracketed paste
 *   3. Composition flow — deltas (grow/shrink/diverge), commit after compose
 *   4. Focus transitions — pause/suppress on focus loss/gain
 *   5. Edge cases — isPaused gating, empty commitText
 */
@RunWith(AndroidJUnit4::class)
class InputCorrectnessTest {
    private val recorded = mutableListOf<Byte>()

    @Before
    fun setUp() {
        recorded.clear()
    }

    // ---------------------------------------------------------------
    // Helpers
    // ---------------------------------------------------------------

    /** Build a [TerminalSurface] + [InputConnection] backed by a mock view model. */
    private fun buildSurface(): Pair<TerminalSurface, InputConnection> {
        val context = ApplicationProvider.getApplicationContext<android.content.Context>()
        val view = TerminalSurface(context)
        val viewModel = mockk<TerminalViewModel>(relaxed = true)
        every { viewModel.state } returns MutableStateFlow(TerminalState())
        every { viewModel.writeToPty(any()) } answers {
            recorded.addAll((it.invocation.args[0] as ByteArray).toList())
        }
        view.initialize(viewModel)
        val connection = view.onCreateInputConnection(EditorInfo())
        return Pair(view, connection)
    }

    /** Drain the main looper so Choreographer-flushed batched input arrives. */
    private fun drainLooper() {
        shadowOf(android.os.Looper.getMainLooper()).idle()
    }

    // ---------------------------------------------------------------
    // 1. InputCoalescer
    // ---------------------------------------------------------------

    @Test
    fun coalescer_singleByteSends() {
        val sink = mutableListOf<Byte>()
        val coalescer = InputCoalescer { data -> sink.addAll(data.toList()) }
        coalescer.send(byteArrayOf('a'.code.toByte()))
        assertArrayEquals(byteArrayOf('a'.code.toByte()), sink.toByteArray())
    }

    @Test
    fun coalescer_multiByteBypassesCoalescing() {
        val sink = mutableListOf<Byte>()
        val coalescer = InputCoalescer { data -> sink.addAll(data.toList()) }
        val cjk = "你好".toByteArray(Charsets.UTF_8)
        coalescer.send(cjk)
        assertArrayEquals(cjk, sink.toByteArray())
    }

    @Test
    fun coalescer_identicalDoubleFireBothSent() {
        val sink = mutableListOf<Byte>()
        val coalescer = InputCoalescer { data -> sink.addAll(data.toList()) }
        coalescer.send(byteArrayOf('a'.code.toByte()))
        assertArrayEquals(byteArrayOf('a'.code.toByte()), sink.toByteArray())
        sink.clear()
        coalescer.send(byteArrayOf('a'.code.toByte()))
        val expected = byteArrayOf('a'.code.toByte())
        assertArrayEquals(expected, sink.toByteArray())
    }

    @Test
    fun coalescer_threePlusBytesNoDedup() {
        val sink = mutableListOf<Byte>()
        val coalescer = InputCoalescer { data -> sink.addAll(data.toList()) }
        coalescer.send(byteArrayOf('a'.code.toByte()))
        coalescer.send(byteArrayOf('a'.code.toByte()))
        coalescer.send(byteArrayOf('a'.code.toByte()))
        assertArrayEquals(
            "three identical bytes all forwarded individually",
            byteArrayOf('a'.code.toByte(), 'a'.code.toByte(), 'a'.code.toByte()),
            sink.toByteArray(),
        )
    }

    @Test
    fun coalescer_emptyFlushDoesNothing() {
        val sink = mutableListOf<Byte>()
        val coalescer = InputCoalescer { data -> sink.addAll(data.toList()) }
        coalescer.flush()
        assertTrue("flush on empty buffer adds nothing", sink.isEmpty())
    }

    @Test
    fun coalescer_reset() {
        val sink = mutableListOf<Byte>()
        val coalescer = InputCoalescer { data -> sink.addAll(data.toList()) }
        coalescer.updateComposingText("hello")
        assertTrue(coalescer.isComposing())
        assertEquals("hello", coalescer.getComposingText())
        coalescer.reset()
        assertFalse(coalescer.isComposing())
        assertNull(coalescer.getComposingText())
        coalescer.flush()
        assertTrue("flush after reset adds nothing", sink.isEmpty())
    }

    // ---------------------------------------------------------------
    // 2. TerminalInputEncoder
    // ---------------------------------------------------------------

    @Test
    fun encoder_normalAscii() {
        val result = TerminalInputEncoder.encodeCommittedText("hello", ctrlActive = false, altActive = false)
        assertArrayEquals("hello".toByteArray(Charsets.UTF_8), result)
    }

    @Test
    fun encoder_utf8MultiByte() {
        val result = TerminalInputEncoder.encodeCommittedText("你好", ctrlActive = false, altActive = false)
        assertArrayEquals("你好".toByteArray(Charsets.UTF_8), result)
    }

    @Test
    fun encoder_ctrlLetter() {
        val result = TerminalInputEncoder.encodeCommittedText("a", ctrlActive = true, altActive = false)
        assertArrayEquals(byteArrayOf(0x01), result)
    }

    @Test
    fun encoder_altLetter() {
        val result = TerminalInputEncoder.encodeCommittedText("a", ctrlActive = false, altActive = true)
        assertArrayEquals(byteArrayOf(0x1B, 'a'.code.toByte()), result)
    }

    @Test
    fun encoder_ctrlAltLetter() {
        val result = TerminalInputEncoder.encodeCommittedText("a", ctrlActive = true, altActive = true)
        assertArrayEquals("ctrl takes priority over alt", byteArrayOf(0x01), result)
    }

    @Test
    fun encoder_bracketedPaste() {
        val result = TerminalInputEncoder.encodeCommittedText("hello", ctrlActive = false, altActive = false, bracketedPaste = true)
        assertArrayEquals("\u001b[200~hello\u001b[201~".toByteArray(Charsets.UTF_8), result)
    }

    @Test
    fun encoder_emptyString() {
        val result = TerminalInputEncoder.encodeCommittedText("", ctrlActive = false, altActive = false)
        assertTrue("empty string produces empty bytes", result.isEmpty())
    }

    // ---------------------------------------------------------------
    // 3. Composition flow
    // ---------------------------------------------------------------

    @Test
    fun setComposingText_emptyBufferSendsAll() {
        val (_, connection) = buildSurface()
        assertTrue(connection.setComposingText("ab", 1))
        drainLooper()
        assertArrayEquals(byteArrayOf('a'.code.toByte(), 'b'.code.toByte()), recorded.toByteArray())
    }

    @Test
    fun setComposingText_growsSendsAppended() {
        val (_, connection) = buildSurface()
        connection.setComposingText("ab", 1)
        drainLooper()
        connection.setComposingText("abc", 1)
        drainLooper()
        assertArrayEquals(
            "growth forwards only the appended character",
            byteArrayOf('a'.code.toByte(), 'b'.code.toByte(), 'c'.code.toByte()),
            recorded.toByteArray(),
        )
    }

    @Test
    fun setComposingText_shrinksSendsBackspaces() {
        val (_, connection) = buildSurface()
        connection.setComposingText("abc", 1)
        drainLooper()
        connection.setComposingText("a", 1)
        assertArrayEquals(
            "shrink sends backspaces for removed characters",
            byteArrayOf('a'.code.toByte(), 'b'.code.toByte(), 'c'.code.toByte(), 0x08.toByte(), 0x08.toByte()),
            recorded.toByteArray(),
        )
    }

    @Test
    fun setComposingText_divergesReplacesAll() {
        val (_, connection) = buildSurface()
        connection.setComposingText("abc", 1)
        drainLooper()
        recorded.clear()
        connection.setComposingText("xyz", 1)
        assertArrayEquals(
            "diverged sends backspaces for entire old composition",
            byteArrayOf(0x08.toByte(), 0x08.toByte(), 0x08.toByte()),
            recorded.toByteArray(),
        )
        drainLooper()
        assertArrayEquals(
            "diverged then sends new composition text",
            byteArrayOf(0x08.toByte(), 0x08.toByte(), 0x08.toByte(), 'x'.code.toByte(), 'y'.code.toByte(), 'z'.code.toByte()),
            recorded.toByteArray(),
        )
    }

    @Test
    fun commitText_afterComposingMatchSkips() {
        val (_, connection) = buildSurface()
        connection.setComposingText("hello", 1)
        drainLooper()
        recorded.clear()
        assertTrue(connection.commitText("hello", 1))
        drainLooper()
        assertTrue(
            "matching commitText after composing should send nothing",
            recorded.isEmpty(),
        )
    }

    @Test
    fun commitText_afterComposingDiffersBackspaceAndSend() {
        val (_, connection) = buildSurface()
        connection.setComposingText("hello", 1)
        drainLooper()
        recorded.clear()
        assertTrue(connection.commitText("world", 1))
        assertArrayEquals(
            "mismatched commitText first sends backspaces",
            byteArrayOf(0x08.toByte(), 0x08.toByte(), 0x08.toByte(), 0x08.toByte(), 0x08.toByte()),
            recorded.toByteArray(),
        )
        drainLooper()
        assertArrayEquals(
            "mismatched commitText then sends replacement text",
            byteArrayOf(
                0x08.toByte(),
                0x08.toByte(),
                0x08.toByte(),
                0x08.toByte(),
                0x08.toByte(),
                'w'.code.toByte(),
                'o'.code.toByte(),
                'r'.code.toByte(),
                'l'.code.toByte(),
                'd'.code.toByte(),
            ),
            recorded.toByteArray(),
        )
    }

    @Test
    fun commitText_withoutComposingSendsDirectly() {
        val (_, connection) = buildSurface()
        assertTrue(connection.commitText("hello", 1))
        drainLooper()
        assertArrayEquals(
            byteArrayOf('h'.code.toByte(), 'e'.code.toByte(), 'l'.code.toByte(), 'l'.code.toByte(), 'o'.code.toByte()),
            recorded.toByteArray(),
        )
    }

    // ---------------------------------------------------------------
    // 4. Focus transitions
    // ---------------------------------------------------------------

    @Test
    fun onWindowFocusChangedFalseFinishesComposing() {
        val (view, connection) = buildSurface()
        connection.setComposingText("hello", 1)
        drainLooper()
        recorded.clear()
        view.onWindowFocusChanged(false)
        assertTrue("onWindowFocusChanged(false) sets isPaused", view.isPaused)
        connection.setComposingText("x", 1)
        drainLooper()
        assertTrue(
            "input blocked after focus loss",
            recorded.isEmpty(),
        )
    }

    @Test
    fun onWindowFocusChangedTrueDoesNotFinishComposing() {
        val (view, connection) = buildSurface()
        connection.setComposingText("hello", 1)
        drainLooper()
        recorded.clear()
        view.onWindowFocusChanged(true)
        assertFalse("onWindowFocusChanged(true) clears isPaused", view.isPaused)
        Thread.sleep(60)
        connection.setComposingText("hellox", 1)
        drainLooper()
        assertArrayEquals(
            "composing buffer preserved; only appended char forwarded",
            byteArrayOf('x'.code.toByte()),
            recorded.toByteArray(),
        )
    }

    @Test
    fun suppressTimerBlocksThenAllows() {
        val (view, connection) = buildSurface()
        view.onWindowFocusChanged(true)
        assertTrue(
            "setComposingText blocked by suppress timer",
            connection.setComposingText("hello", 1),
        )
        drainLooper()
        assertTrue(
            "no bytes sent while suppress timer active",
            recorded.isEmpty(),
        )
        Thread.sleep(60)
        assertTrue(connection.setComposingText("hello", 1))
        drainLooper()
        assertArrayEquals(
            "bytes sent after suppress timer expires",
            byteArrayOf('h'.code.toByte(), 'e'.code.toByte(), 'l'.code.toByte(), 'l'.code.toByte(), 'o'.code.toByte()),
            recorded.toByteArray(),
        )
    }

    // ---------------------------------------------------------------
    // 5. Edge cases
    // ---------------------------------------------------------------

    @Test
    fun isPausedTrueBlocksComposing() {
        val (view, connection) = buildSurface()
        view.isPaused = true
        assertTrue(connection.setComposingText("hello", 1))
        drainLooper()
        assertTrue("composing blocked when isPaused=true", recorded.isEmpty())
    }

    @Test
    fun isPausedTrueBlocksCommit() {
        val (view, connection) = buildSurface()
        view.isPaused = true
        assertTrue(connection.commitText("hello", 1))
        drainLooper()
        assertTrue("commitText blocked when isPaused=true", recorded.isEmpty())
    }

    @Test
    fun isPausedFalseAllowsComposing() {
        val (view, connection) = buildSurface()
        view.isPaused = false
        assertTrue(connection.setComposingText("hello", 1))
        drainLooper()
        assertArrayEquals(
            byteArrayOf('h'.code.toByte(), 'e'.code.toByte(), 'l'.code.toByte(), 'l'.code.toByte(), 'o'.code.toByte()),
            recorded.toByteArray(),
        )
    }

    @Test
    fun commitText_nullReturnsFalse() {
        val (_, connection) = buildSurface()
        assertFalse("commitText(null) returns false", connection.commitText(null, 1))
    }

    @Test
    fun commitText_emptyReturnsTrue() {
        val (_, connection) = buildSurface()
        assertTrue("commitText(\"\") returns true", connection.commitText("", 1))
    }
}
