package io.torvox.ui

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

@RunWith(RobolectricTestRunner::class)
@Config(application = Application::class)
class InputCoalescerTest {
    private lateinit var coalescer: InputCoalescer
    private val sink = mutableListOf<Byte>()
    private var pendingFlush: Runnable? = null

    @Before
    fun setUp() {
        sink.clear()
        pendingFlush = null
        // Capture the flush runnable instead of posting it, so the test controls
        // the message boundary explicitly (simulates the batch accumulating
        // within a single input message before the looper turn runs it).
        coalescer =
            InputCoalescer(
                sink = { data -> sink.addAll(data.toList()) },
                scheduler = { runnable -> pendingFlush = runnable },
            )
    }

    private fun byte(value: Char) = value.code.toByte()

    private fun runFlush() {
        pendingFlush?.run()
        pendingFlush = null
    }

    @Test
    fun singleByteIsBufferedUntilFlush() {
        coalescer.send(byteArrayOf(byte('a')))
        assertTrue("not sent before flush", sink.isEmpty())
        runFlush()
        assertArrayEquals(byteArrayOf(byte('a')), sink.toByteArray())
    }

    @Test
    fun identicalDoubleFireInOneBatchIsDeduped() {
        coalescer.send(byteArrayOf(byte('a')))
        coalescer.send(byteArrayOf(byte('a')))
        runFlush()
        assertArrayEquals("IME double-fire collapses to one byte", byteArrayOf(byte('a')), sink.toByteArray())
    }

    @Test
    fun tripleIdenticalIsPreserved() {
        coalescer.send(byteArrayOf(byte('a')))
        coalescer.send(byteArrayOf(byte('a')))
        coalescer.send(byteArrayOf(byte('a')))
        runFlush()
        assertArrayEquals("three identical bytes preserved", byteArrayOf(byte('a'), byte('a'), byte('a')), sink.toByteArray())
    }

    @Test
    fun differentBytesInOneBatchArePreserved() {
        coalescer.send(byteArrayOf(byte('a')))
        coalescer.send(byteArrayOf(byte('b')))
        runFlush()
        assertArrayEquals(byteArrayOf(byte('a'), byte('b')), sink.toByteArray())
    }

    @Test
    fun twoDifferentBatchesDoNotDedup() {
        coalescer.send(byteArrayOf(byte('a')))
        runFlush()
        coalescer.send(byteArrayOf(byte('a')))
        runFlush()
        assertArrayEquals("separate batches keep both", byteArrayOf(byte('a'), byte('a')), sink.toByteArray())
    }

    @Test
    fun legitFastTypingTwoIdenticalCharsInSeparateBatchesIsPreserved() {
        // I3: genuine IME double-fire is exactly TWO identical bytes within ONE
        // input message (one batch). Two identical characters typed quickly by a
        // real user arrive as separate input messages (separate batches), so they
        // must both be kept. This is the precise distinction the new batch model
        // makes: dedup only happens inside a single batch.
        coalescer.send(byteArrayOf(byte('x')))
        runFlush()
        coalescer.send(byteArrayOf(byte('x')))
        runFlush()
        assertArrayEquals(
            "fast-typed identical chars in distinct batches are preserved",
            byteArrayOf(byte('x'), byte('x')),
            sink.toByteArray(),
        )
    }

    @Test
    fun multiByteInputBypassesCoalescingAndSendsImmediately() {
        val cjk = "你好".toByteArray(Charsets.UTF_8)
        coalescer.send(cjk)
        // Sent immediately, before any flush.
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

        coalescer.reset()

        assertFalse(coalescer.isComposing())
        assertNull(coalescer.getComposingText())
        runFlush()
        assertTrue("no bytes flushed after reset", sink.isEmpty())
    }
}
