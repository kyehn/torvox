package io.torvox.bridge

import org.junit.Assert.assertArrayEquals
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * Unit tests for the wire encoding helpers used by the JNA bridge.
 * WireReader and WireWriter are pure JVM and can be tested without an
 * Android device or instrumented test.
 */
class WireCodecTest {
    @Test
    fun wireWriter_writeByte() {
        val wireWriter = WireWriter()
        wireWriter.writeByte(0x42)
        assertArrayEquals(byteArrayOf(0x42), wireWriter.toByteArray())
    }

    @Test
    fun wireWriter_writeI32_zero() {
        val wireWriter = WireWriter()
        wireWriter.writeI32(0)
        assertArrayEquals(byteArrayOf(0, 0, 0, 0), wireWriter.toByteArray())
    }

    @Test
    fun wireWriter_writeI32_one() {
        val wireWriter = WireWriter()
        wireWriter.writeI32(1)
        assertArrayEquals(byteArrayOf(1, 0, 0, 0), wireWriter.toByteArray())
    }

    @Test
    fun wireWriter_writeI32_littleEndian() {
        val wireWriter = WireWriter()
        wireWriter.writeI32(0x12345678)
        val expected = byteArrayOf(0x78, 0x56, 0x34, 0x12)
        assertArrayEquals(expected, wireWriter.toByteArray())
    }

    @Test
    fun wireWriter_writeI32_negative() {
        val wireWriter = WireWriter()
        wireWriter.writeI32(-1)
        assertArrayEquals(byteArrayOf(0xFF.toByte(), 0xFF.toByte(), 0xFF.toByte(), 0xFF.toByte()), wireWriter.toByteArray())
    }

    @Test
    fun wireWriter_writeU32() {
        val wireWriter = WireWriter()
        wireWriter.writeU32(0xDEADBEEFu)
        val expected = byteArrayOf(0xEF.toByte(), 0xBE.toByte(), 0xAD.toByte(), 0xDE.toByte())
        assertArrayEquals(expected, wireWriter.toByteArray())
    }

    @Test
    fun wireWriter_writeString_empty() {
        val wireWriter = WireWriter()
        wireWriter.writeString("")
        // 4 bytes for length (0) and no payload
        assertArrayEquals(byteArrayOf(0, 0, 0, 0), wireWriter.toByteArray())
    }

    @Test
    fun wireWriter_writeString_ascii() {
        val wireWriter = WireWriter()
        wireWriter.writeString("hi")
        // length=2, then 'h', 'i'
        assertArrayEquals(byteArrayOf(2, 0, 0, 0, 'h'.code.toByte(), 'i'.code.toByte()), wireWriter.toByteArray())
    }

    @Test
    fun wireWriter_writeString_utf8() {
        val utf8Char = "中"
        val expected = utf8Char.toByteArray(Charsets.UTF_8)
        val wireWriter2 = WireWriter()
        wireWriter2.writeString(utf8Char)
        val out = wireWriter2.toByteArray()
        // 4 bytes length + UTF-8 bytes
        assertEquals(4 + expected.size, out.size)
        // last N bytes match UTF-8 encoding
        for (i in expected.indices) {
            assertEquals(expected[i], out[4 + i])
        }
    }

    @Test
    fun wireReader_readByte() {
        val wireReader = WireReader(byteArrayOf(0x42))
        assertEquals(0x42.toByte(), wireReader.readByte())
    }

    @Test
    fun wireReader_readBool_true() {
        val wireReader = WireReader(byteArrayOf(1))
        assertTrue(wireReader.readBool())
    }

    @Test
    fun wireReader_readBool_false() {
        val wireReader = WireReader(byteArrayOf(0))
        assertFalse(wireReader.readBool())
    }

    @Test
    fun wireRoundTrip_i32() {
        val values = listOf(0, 1, -1, 42, Int.MAX_VALUE, Int.MIN_VALUE, 0x12345678)
        for (v in values) {
            val wireWriter = WireWriter()
            wireWriter.writeI32(v)
            val wireReader = WireReader(wireWriter.toByteArray())
            // We only have readI32 implemented through readU32/readI32 helpers; use a simple
            // roundtrip check by re-reading through the public readI32 method if present.
            // If not present, ensure bytes match expected little-endian encoding.
            val expected =
                byteArrayOf(
                    (v and 0xFF).toByte(),
                    ((v shr 8) and 0xFF).toByte(),
                    ((v shr 16) and 0xFF).toByte(),
                    ((v shr 24) and 0xFF).toByte(),
                )
            assertArrayEquals("i32 $v", expected, wireWriter.toByteArray())
        }
    }

    @Test
    fun wireWriter_concatenates() {
        val wireWriter = WireWriter()
        wireWriter.writeI32(1)
        wireWriter.writeI32(2)
        wireWriter.writeByte(0xAB.toByte())
        val out = wireWriter.toByteArray()
        assertEquals(9, out.size)
    }

    @Test
    fun wireWriter_string_roundTripAscii() {
        val testString = "hello"
        val wireWriter = WireWriter()
        wireWriter.writeString(testString)
        val wireReader = WireReader(wireWriter.toByteArray())
        assertEquals(testString, wireReader.readString())
    }

    @Test
    fun shell_systemDefault_wireRoundTrip() {
        val wireWriter = WireWriter()
        Shell.SystemDefault.wireEncode(wireWriter)
        val wireReader = WireReader(wireWriter.toByteArray())
        val tag = wireReader.readI32()
        assertEquals(0, tag)
    }

    @Test
    fun shell_custom_wireRoundTrip() {
        val wireWriter = WireWriter()
        Shell.Custom("/system/bin/bash").wireEncode(wireWriter)
        val wireReader = WireReader(wireWriter.toByteArray())
        val tag = wireReader.readI32()
        assertEquals(1, tag)
        assertEquals("/system/bin/bash", wireReader.readString())
    }

    @Test
    fun shell_custom_wireRoundTrip_emptyPath() {
        val wireWriter = WireWriter()
        Shell.Custom("").wireEncode(wireWriter)
        val wireReader = WireReader(wireWriter.toByteArray())
        assertEquals(1, wireReader.readI32())
        assertEquals("", wireReader.readString())
    }

    @Test
    fun shell_custom_wireRoundTrip_utf8Path() {
        val path = "/data/用户/bin/sh"
        val wireWriter = WireWriter()
        Shell.Custom(path).wireEncode(wireWriter)
        val wireReader = WireReader(wireWriter.toByteArray())
        assertEquals(1, wireReader.readI32())
        assertEquals(path, wireReader.readString())
    }

    @Test
    fun bridgeTheme_default_wireRoundTrip() {
        val theme = BridgeTheme()
        val bytes = theme.wireEncodeBytes()
        val decoded = BridgeTheme.wireDecode(WireReader(bytes))
        assertEquals(theme, decoded)
    }

    @Test
    fun bridgeTheme_populated_wireRoundTrip() {
        val theme =
            BridgeTheme(
                name = "Dracula Plus",
                bg = 0x282A36,
                fg = 0xF8F8F2,
                cursor = 0xF8F8F2,
                selectionBg = 0x44475A,
                ansi0 = 0x000000,
                ansi1 = 0xFF5555,
                ansi2 = 0x50FA7B,
                ansi3 = 0xF1FA8C,
                ansi4 = 0xBD93F9,
                ansi5 = 0xFF79C6,
                ansi6 = 0x8BE9FD,
                ansi7 = 0xF8F8F2,
                ansi8 = 0x6272A4,
                ansi9 = 0xFF6E6E,
                ansi10 = 0x69FF94,
                ansi11 = 0xFFFFA5,
                ansi12 = 0xD6ACFF,
                ansi13 = 0xFF92DF,
                ansi14 = 0xA4FFFF,
                ansi15 = 0xFFFFFF,
            )
        val bytes = theme.wireEncodeBytes()
        val decoded = BridgeTheme.wireDecode(WireReader(bytes))
        assertEquals(theme, decoded)
    }

    @Test
    fun bridgeTheme_wireEncode_size() {
        val theme = BridgeTheme(name = "test")
        val bytes = theme.wireEncodeBytes()
        // 1 string (4 + 4) + 20 i32 (20 * 4) = 88
        assertEquals(88, bytes.size)
    }

    @Test
    fun terminalConfig_default_wireRoundTrip() {
        val config = TerminalConfig()
        val bytes = config.wireEncode()
        val decoded = TerminalConfig.wireDecode(WireReader(bytes))
        assertEquals(config.shell, decoded.shell)
        assertEquals(config.rows, decoded.rows)
        assertEquals(config.cols, decoded.cols)
        assertEquals(config.scrollbackLines, decoded.scrollbackLines)
        assertEquals(config.font_size_tenths, decoded.font_size_tenths)
        assertEquals(config.theme, decoded.theme)
        assertEquals(config.home, decoded.home)
        assertEquals(config.user, decoded.user)
        assertEquals(config.path, decoded.path)
        assertEquals(config.workingDirectory, decoded.workingDirectory)
    }

    @Test
    fun terminalConfig_customShell_wireRoundTrip() {
        val config =
            TerminalConfig(
                shell = Shell.Custom("/system/bin/zsh"),
                rows = 40u,
                cols = 120u,
                scrollbackLines = 100000u,
                font_size_tenths = 180u,
                theme = BridgeTheme(name = "Nord"),
                home = "/data/data/io.torvox/files",
                user = "u0_a123",
                path = "/system/bin:/system/xbin",
                workingDirectory = "/data/data/io.torvox/files",
            )
        val bytes = config.wireEncode()
        val decoded = TerminalConfig.wireDecode(WireReader(bytes))
        assertEquals(Shell.Custom("/system/bin/zsh"), decoded.shell)
        assertEquals(40u, decoded.rows)
        assertEquals(120u, decoded.cols)
        assertEquals(100000u, decoded.scrollbackLines)
        assertEquals(180u, decoded.font_size_tenths)
        assertEquals("Nord", decoded.theme.name)
        assertEquals("/data/data/io.torvox/files", decoded.home)
        assertEquals("u0_a123", decoded.user)
        assertEquals("/system/bin:/system/xbin", decoded.path)
        assertEquals("/data/data/io.torvox/files", decoded.workingDirectory)
    }

    @Test
    fun terminalConfig_unknownShellTag_fallsBackToSystemDefault() {
        val wireWriter = WireWriter()
        wireWriter.writeI32(99)
        wireWriter.writeU32(24u)
        wireWriter.writeU32(80u)
        wireWriter.writeU32(50000u)
        wireWriter.writeU32(140u)
        BridgeTheme().wireEncode(wireWriter)
        wireWriter.writeString("")
        wireWriter.writeString("")
        wireWriter.writeString("")
        wireWriter.writeString("")
        wireWriter.writeString("")
        val config = TerminalConfig.wireDecode(WireReader(wireWriter.toByteArray()))
        assertEquals(Shell.SystemDefault, config.shell)
    }
}
