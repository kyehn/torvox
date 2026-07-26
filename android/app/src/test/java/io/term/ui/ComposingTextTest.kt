package io.term.ui

import android.view.inputmethod.EditorInfo
import android.view.inputmethod.InputConnection
import androidx.test.core.app.ApplicationProvider
import androidx.test.ext.junit.runners.AndroidJUnit4
import io.mockk.every
import io.mockk.mockk
import io.term.TerminalState
import io.term.TerminalViewModel
import kotlinx.coroutines.flow.MutableStateFlow
import org.junit.Assert.assertArrayEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.Shadows.shadowOf

/**
 * I4 — Composing IME reconciliation.
 *
 * The `BaseInputConnection` created by `TerminalSurface.onCreateInputConnection`
 * overrides `setComposingText` / `finishComposingText` to reconcile composition
 * deltas instead of dropping them: a growing composition forwards only the
 * appended characters, a shrinking composition sends backspaces, and a finished
 * composition clears the buffer so the next keystroke does not backspace stale
 * state.
 */
@RunWith(AndroidJUnit4::class)
class ComposingTextTest {
    private val recorded = mutableListOf<Byte>()

    private fun buildSurface(): Pair<TerminalSurface, InputConnection> {
        recorded.clear()
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

    private fun drainLooper() {
        shadowOf(android.os.Looper.getMainLooper()).idle()
    }

    @Test
    fun setComposingText_overrideExistsAndForwardsComposition() {
        val (_, connection) = buildSurface()
        assertTrue(connection is android.view.inputmethod.BaseInputConnection)
        // First composition "ab" is forwarded verbatim to the PTY.
        assertTrue(connection.setComposingText("ab", 1))
        drainLooper()
        assertArrayEquals("first composition sent verbatim", byteArrayOf('a'.code.toByte(), 'b'.code.toByte()), recorded.toByteArray())
    }

    @Test
    fun setComposingText_reconcilesGrowthAsAppendedDelta() {
        val (_, connection) = buildSurface()
        connection.setComposingText("ab", 1)
        drainLooper()
        assertArrayEquals("initial composition", byteArrayOf('a'.code.toByte(), 'b'.code.toByte()), recorded.toByteArray())

        // Composition grows ab -> abc: only the appended 'c' is forwarded.
        connection.setComposingText("abc", 1)
        drainLooper()
        assertArrayEquals(
            "growth forwards only the appended character",
            byteArrayOf('a'.code.toByte(), 'b'.code.toByte(), 'c'.code.toByte()),
            recorded.toByteArray(),
        )
    }

    @Test
    fun setComposingText_reconcilesShrinkAsBackspaces() {
        val (_, connection) = buildSurface()
        connection.setComposingText("abc", 1)
        drainLooper()
        assertArrayEquals(
            "initial composition",
            byteArrayOf('a'.code.toByte(), 'b'.code.toByte(), 'c'.code.toByte()),
            recorded.toByteArray(),
        )

        // Composition shrinks abc -> a: two backspaces (0x08) are sent.
        connection.setComposingText("a", 1)
        drainLooper()
        assertArrayEquals(
            "shrink sends backspaces for removed characters",
            byteArrayOf('a'.code.toByte(), 'b'.code.toByte(), 'c'.code.toByte(), 0x08.toByte(), 0x08.toByte()),
            recorded.toByteArray(),
        )
    }

    @Test
    fun finishComposingText_clearsBufferSoNextKeystrokeDoesNotBackspace() {
        val (_, connection) = buildSurface()
        connection.setComposingText("abc", 1)
        drainLooper()
        assertTrue(connection.finishComposingText())
        drainLooper()
        // After finishing, the composing buffer is empty: a fresh "x" is sent
        // forward (not a backspace of the old buffer).
        connection.setComposingText("x", 1)
        drainLooper()
        assertArrayEquals(
            "fresh composition after finish should not backspace stale state",
            byteArrayOf('a'.code.toByte(), 'b'.code.toByte(), 'c'.code.toByte(), 'x'.code.toByte()),
            recorded.toByteArray(),
        )
    }

    @Test
    fun finishComposingText_returnsTrue() {
        val (_, connection) = buildSurface()
        connection.setComposingText("hello", 1)
        drainLooper()
        assertTrue(connection.finishComposingText())
        assertFalse(recorded.contains(0x08.toByte()))
    }
}
