package io.term.bridge

import org.junit.Assert.assertArrayEquals
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

class BridgeConversionTest {
    @Test
    fun wireReader_readBool_nonZero_isTrue() {
        val reader = WireReader(byteArrayOf(0x01))
        assertTrue(reader.readBool())
    }

    @Test
    fun wireReader_readBool_anyNonZero_isTrue() {
        val reader = WireReader(byteArrayOf(0x7F))
        assertTrue(reader.readBool())
    }

    @Test
    fun wireReader_readBool_negative_isTrue() {
        val reader = WireReader(byteArrayOf(0xFF.toByte()))
        assertTrue(reader.readBool())
    }

    @Test
    fun wireWriter_writeBool_true() {
        val writer = WireWriter()
        writer.writeByte(1)
        assertArrayEquals(byteArrayOf(1), writer.toByteArray())
    }

    @Test
    fun wireWriter_writeBool_false() {
        val writer = WireWriter()
        writer.writeByte(0)
        assertArrayEquals(byteArrayOf(0), writer.toByteArray())
    }

    @Test
    fun wireReader_readByte_negativeValue() {
        val reader = WireReader(byteArrayOf(0x80.toByte()))
        assertEquals(0x80.toByte(), reader.readByte())
    }

    @Test
    fun wireReader_readByte_minValue() {
        val reader = WireReader(byteArrayOf(Byte.MIN_VALUE))
        assertEquals(Byte.MIN_VALUE, reader.readByte())
    }

    @Test
    fun wireReader_readByte_maxValue() {
        val reader = WireReader(byteArrayOf(Byte.MAX_VALUE))
        assertEquals(Byte.MAX_VALUE, reader.readByte())
    }

    @Test
    fun wireWriter_writeByte_allValues() {
        val writer = WireWriter()
        writer.writeByte(0x00)
        writer.writeByte(0x7F)
        writer.writeByte(0x80.toByte())
        writer.writeByte(0xFF.toByte())
        assertArrayEquals(byteArrayOf(0x00, 0x7F, 0x80.toByte(), 0xFF.toByte()), writer.toByteArray())
    }

    @Test
    fun terminalConfig_prefixField_roundTrip() {
        val config =
            TerminalConfig(
                shell = Shell.SystemDefault,
                prefix = "/data/data/io.term/files/bootstrap/usr",
            )
        val bytes = config.wireEncode()
        val decoded = TerminalConfig.wireDecode(WireReader(bytes))
        assertEquals("/data/data/io.term/files/bootstrap/usr", decoded.prefix)
    }

    @Test
    fun terminalConfig_prefixField_empty() {
        val config = TerminalConfig()
        val bytes = config.wireEncode()
        val decoded = TerminalConfig.wireDecode(WireReader(bytes))
        assertEquals("", decoded.prefix)
    }

    @Test
    fun terminalConfig_prefixField_longPath() {
        val longPrefix = "/data/data/io.term/files/bootstrap/usr".repeat(10)
        val config = TerminalConfig(prefix = longPrefix)
        val bytes = config.wireEncode()
        val decoded = TerminalConfig.wireDecode(WireReader(bytes))
        assertEquals(longPrefix, decoded.prefix)
    }

    @Test
    fun shell_systemDefault_wireSize() {
        val writer = WireWriter()
        Shell.SystemDefault.wireEncode(writer)
        assertEquals(4, writer.toByteArray().size)
    }

    @Test
    fun shell_custom_emptyPath_wireSize() {
        val writer = WireWriter()
        Shell.Custom("").wireEncode(writer)
        val bytes = writer.toByteArray()
        assertEquals(8, bytes.size)
    }

    @Test
    fun bridgeTheme_allFields_default() {
        val theme = BridgeTheme()
        assertEquals("", theme.name)
        assertEquals(0, theme.bg)
        assertEquals(0, theme.fg)
        assertEquals(0, theme.cursor)
        assertEquals(0, theme.selectionBg)
        val ansiFields =
            theme.run {
                listOf(
                    ansi0,
                    ansi1,
                    ansi2,
                    ansi3,
                    ansi4,
                    ansi5,
                    ansi6,
                    ansi7,
                    ansi8,
                    ansi9,
                    ansi10,
                    ansi11,
                    ansi12,
                    ansi13,
                    ansi14,
                    ansi15,
                )
            }
        for ((i, value) in ansiFields.withIndex()) {
            assertEquals("ansi$i default", 0, value)
        }
    }

    @Test
    fun bridgeTheme_wireEncode_maxValues() {
        val theme =
            BridgeTheme(
                bg = Int.MAX_VALUE,
                fg = Int.MAX_VALUE,
                cursor = Int.MAX_VALUE,
                selectionBg = Int.MAX_VALUE,
            )
        val bytes = theme.wireEncodeBytes()
        val decoded = BridgeTheme.wireDecode(WireReader(bytes))
        assertEquals(Int.MAX_VALUE, decoded.bg)
        assertEquals(Int.MAX_VALUE, decoded.fg)
    }

    @Test
    fun shell_equality_systemDefault() {
        assertEquals(Shell.SystemDefault, Shell.SystemDefault)
    }

    @Test
    fun shell_equality_custom() {
        assertEquals(Shell.Custom("/bin/bash"), Shell.Custom("/bin/bash"))
    }

    @Test
    fun shell_inequality_custom() {
        assertFalse(Shell.Custom("/bin/bash") == Shell.Custom("/bin/zsh"))
    }

    @Test
    fun shell_custom_specialCharacters() {
        val path = "/data/user/0/com.termux/files/home/.local/bin/sh"
        val writer = WireWriter()
        Shell.Custom(path).wireEncode(writer)
        val reader = WireReader(writer.toByteArray())
        assertEquals(1, reader.readI32())
        assertEquals(path, reader.readString())
    }
}
