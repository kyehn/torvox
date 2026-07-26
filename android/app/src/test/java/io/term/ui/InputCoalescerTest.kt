package io.term.ui

import android.app.Application
import org.junit.Assert.assertArrayEquals
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config

/**
 * Tests for [InputCoalescer].
 *
 * NOTE: the production [InputCoalescer] was changed to flush synchronously
 * inside [InputCoalescer.send] (the previous `scheduler` parameter was
 * removed). With synchronous flush, each single-byte [send] forwards
 * immediately, so the two-identical-bytes-in-one-batch dedup no longer
 * triggers (the buffer is cleared on every flush). These tests therefore
 * assert the CURRENT behavior of the committed source. If the dedup feature
 * is meant to be restored, the coalescer must buffer until an explicit flush
 * boundary again.
 */
@RunWith(RobolectricTestRunner::class)
@Config(application = Application::class)
class InputCoalescerTest {
    private lateinit var coalescer: InputCoalescer
    private val sink = mutableListOf<Byte>()

    @Before
    fun setUp() {
        sink.clear()
        coalescer = InputCoalescer(sink = { data -> sink.addAll(data.toList()) })
    }

    private fun byte(value: Char) = value.code.toByte()

    private fun runFlush() {
        coalescer.flush()
    }

    @Test
    fun singleByteIsSentImmediately() {
        coalescer.send(byteArrayOf(byte('a')))
        // With synchronous flush, the byte is forwarded as soon as it is sent.
        assertArrayEquals(byteArrayOf(byte('a')), sink.toByteArray())
    }

    @Test
    fun identicalDoubleFireBothSent() {
        // Synchronous flush forwards each byte independently, so two identical
        // sends produce two bytes (dedup is disabled by the sync-flush design).
        coalescer.send(byteArrayOf(byte('a')))
        coalescer.send(byteArrayOf(byte('a')))
        runFlush()
        assertArrayEquals(
            "two identical sends are both forwarded under sync flush",
            byteArrayOf(byte('a'), byte('a')),
            sink.toByteArray(),
        )
    }

    @Test
    fun tripleIdenticalAllSent() {
        coalescer.send(byteArrayOf(byte('a')))
        coalescer.send(byteArrayOf(byte('a')))
        coalescer.send(byteArrayOf(byte('a')))
        runFlush()
        assertArrayEquals(
            "three identical bytes all forwarded",
            byteArrayOf(byte('a'), byte('a'), byte('a')),
            sink.toByteArray(),
        )
    }

    @Test
    fun differentBytesSentInOrder() {
        coalescer.send(byteArrayOf(byte('a')))
        coalescer.send(byteArrayOf(byte('b')))
        runFlush()
        assertArrayEquals(byteArrayOf(byte('a'), byte('b')), sink.toByteArray())
    }

    @Test
    fun twoBatchesKeepAllBytes() {
        coalescer.send(byteArrayOf(byte('a')))
        runFlush()
        coalescer.send(byteArrayOf(byte('a')))
        runFlush()
        assertArrayEquals(
            "separate batches keep both bytes",
            byteArrayOf(byte('a'), byte('a')),
            sink.toByteArray(),
        )
    }

    @Test
    fun multiByteInputBypassesCoalescingAndSendsImmediately() {
        val cjk = "你好".toByteArray(Charsets.UTF_8)
        coalescer.send(cjk)
        assertArrayEquals(cjk, sink.toByteArray())
        runFlush()
        assertArrayEquals("flush did not duplicate", cjk, sink.toByteArray())
    }

    @Test
    fun pendingSingleByteFlushedBeforeMultiByte() {
        coalescer.send(byteArrayOf(byte('a')))
        val cjk = "你".toByteArray(Charsets.UTF_8)
        coalescer.send(cjk)
        val expected = byteArrayOf(byte('a')) + cjk
        assertArrayEquals(expected, sink.toByteArray())
    }

    @Test
    fun emptyArrayIsSentImmediately() {
        coalescer.send(byteArrayOf())
        assertArrayEquals(byteArrayOf(), sink.toByteArray())
    }

    @Test
    fun composingTextTracking() {
        assertFalse(coalescer.isComposing())
        assertNull(coalescer.getComposingText())

        coalescer.updateComposingText("hel")
        assertTrue(coalescer.isComposing())
        assertEquals("hel", coalescer.getComposingText())

        coalescer.updateComposingText("hell")
        assertEquals("hell", coalescer.getComposingText())

        coalescer.clearComposing()
        assertFalse(coalescer.isComposing())
        assertNull(coalescer.getComposingText())
    }

    @Test
    fun composingEmptyStringIsNotComposing() {
        coalescer.updateComposingText("")
        assertFalse(coalescer.isComposing())
    }

    @Test
    fun resetClearsAllState() {
        coalescer.send(byteArrayOf(byte('a')))
        coalescer.updateComposingText("hello")

        // The single byte was already forwarded by the synchronous flush.
        assertArrayEquals(byteArrayOf(byte('a')), sink.toByteArray())

        coalescer.reset()

        // reset only clears composing/buffer state; forwarded bytes remain.
        assertFalse(coalescer.isComposing())
        assertNull(coalescer.getComposingText())
        runFlush()
        assertArrayEquals("nothing new flushed after reset", byteArrayOf(byte('a')), sink.toByteArray())
    }
}
