package io.torvox.bridge

import org.junit.Assert.assertEquals
import org.junit.Assert.assertThrows
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * K4 — WireReader bounds validation.
 *
 * `readI32` / `readString` must throw a clear [IllegalArgumentException] on a short
 * or corrupt buffer instead of an `IndexOutOfBoundsException` or silently
 * returning garbage. `WireReader` is pure JVM and needs no Android runtime.
 */
class WireReaderValidationTest {
    @Test
    fun readI32_throwsOnShortBuffer() {
        val reader = WireReader(byteArrayOf(1, 2))
        val exception =
            assertThrows(IllegalArgumentException::class.java) {
                reader.readI32()
            }
        assertTrue(
            "message should explain the missing bytes, was: ${exception.message}",
            exception.message?.contains("need 4 bytes") == true,
        )
    }

    @Test
    fun readI32_throwsWhenExactlyOneByteShort() {
        val reader = WireReader(byteArrayOf(1, 2, 3))
        val exception =
            assertThrows(IllegalArgumentException::class.java) {
                reader.readI32()
            }
        assertTrue(exception.message?.contains("but only 3 remain") == true)
    }

    @Test
    fun readString_throwsWhenLengthExceedsRemaining() {
        // length prefix says 10, but only 3 payload bytes follow.
        val writer = WireWriter()
        writer.writeI32(10)
        writer.writeByte(0x61)
        writer.writeByte(0x62)
        writer.writeByte(0x63)
        val reader = WireReader(writer.toByteArray())
        val exception =
            assertThrows(IllegalArgumentException::class.java) {
                reader.readString()
            }
        assertTrue(
            "message should explain the over-long length, was: ${exception.message}",
            exception.message?.contains("need 10 bytes") == true,
        )
    }

    @Test
    fun readString_throwsOnNegativeLength() {
        // A corrupt length prefix of -1 must be rejected, not decoded as garbage.
        val reader = WireReader(byteArrayOf(0xFF.toByte(), 0xFF.toByte(), 0xFF.toByte(), 0xFF.toByte()))
        val exception =
            assertThrows(IllegalArgumentException::class.java) {
                reader.readString()
            }
        assertTrue(exception.message?.contains("negative length") == true)
    }

    @Test
    fun readI32_succeedsOnExactlyEnoughBytes() {
        val reader = WireReader(byteArrayOf(0x78, 0x56, 0x34, 0x12))
        assertEquals(0x12345678, reader.readI32())
    }

    @Test
    fun readString_succeedsOnWellFormedBuffer() {
        val writer = WireWriter()
        writer.writeString("hi")
        val reader = WireReader(writer.toByteArray())
        assertEquals("hi", reader.readString())
    }
}
