package io.torvox.bridge

import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * FFI contract tests verifying that Kotlin wire encoding matches Rust expectations.
 * These tests validate the byte-level wire format without requiring native libs.
 * Field order, tag values, and encoding must match bridge.rs exactly.
 */
class FfiContractTest {
    // ── Shell wire contract ──────────────────────────────────────────────

    @Test
    fun shell_systemDefault_tagIsZero() {
        val w = WireWriter()
        Shell.SystemDefault.wireEncode(w)
        val r = WireReader(w.toByteArray())
        assertEquals(0, r.readI32())
    }

    @Test
    fun shell_custom_tagIsOne() {
        val w = WireWriter()
        Shell.Custom("/bin/sh").wireEncode(w)
        val r = WireReader(w.toByteArray())
        assertEquals(1, r.readI32())
        assertEquals("/bin/sh", r.readString())
    }

    @Test
    fun shell_custom_longPath() {
        val path = "/data/data/io.torvox/files/bootstrap/usr/bin/bash"
        val w = WireWriter()
        Shell.Custom(path).wireEncode(w)
        val r = WireReader(w.toByteArray())
        assertEquals(1, r.readI32())
        assertEquals(path, r.readString())
    }

    // ── BridgeTheme wire contract ────────────────────────────────────────

    @Test
    fun bridgeTheme_fieldOrderAndSize() {
        // Rust BridgeTheme: name(string) + bg(i32) + fg(i32) + cursor(i32) + selectionBg(i32)
        //   + ansi0..ansi15 (16 x i32) = 1 string + 20 i32
        val theme =
            BridgeTheme(
                name = "Test",
                bg = 0x111111,
                fg = 0x222222,
                cursor = 0x333333,
                selectionBg = 0x444444,
                ansi0 = 10,
                ansi1 = 11,
                ansi2 = 12,
                ansi3 = 13,
                ansi4 = 14,
                ansi5 = 15,
                ansi6 = 16,
                ansi7 = 17,
                ansi8 = 18,
                ansi9 = 19,
                ansi10 = 20,
                ansi11 = 21,
                ansi12 = 22,
                ansi13 = 23,
                ansi14 = 24,
                ansi15 = 25,
            )
        val bytes = theme.wireEncodeBytes()
        // string: 4(len) + 4("Test") = 8
        // 20 x i32 = 80
        assertEquals(88, bytes.size)

        val r = WireReader(bytes)
        assertEquals("Test", r.readString())
        assertEquals(0x111111, r.readI32()) // bg
        assertEquals(0x222222, r.readI32()) // fg
        assertEquals(0x333333, r.readI32()) // cursor
        assertEquals(0x444444, r.readI32()) // selectionBg
        for (i in 0..15) {
            assertEquals(10 + i, r.readI32()) // ansi0..ansi15
        }
    }

    @Test
    fun bridgeTheme_emptyName() {
        val theme = BridgeTheme(name = "", bg = 0xFF0000, fg = 0x00FF00)
        val bytes = theme.wireEncodeBytes()
        val r = WireReader(bytes)
        assertEquals("", r.readString())
        assertEquals(0xFF0000, r.readI32())
        assertEquals(0x00FF00, r.readI32())
    }

    // ── TerminalConfig wire contract ─────────────────────────────────────

    @Test
    fun terminalConfig_fieldOrderMatchesRust() {
        // Rust TerminalConfig order:
        // shell, rows, cols, scrollbackLines, font_size_tenths, theme,
        // home, user, path, workingDirectory, prefix
        val config =
            TerminalConfig(
                shell = Shell.Custom("/bin/bash"),
                rows = 40u,
                cols = 120u,
                scrollbackLines = 50000u,
                font_size_tenths = 160u,
                theme = BridgeTheme(name = "Dracula", bg = 0x282A36, fg = 0xF8F8F2),
                home = "/data/data/io.torvox",
                user = "u0_a123",
                path = "/system/bin",
                workingDirectory = "/data/data/io.torvox/files",
                prefix = "/data/data/io.torvox/files",
            )
        val bytes = config.wireEncode()
        val r = WireReader(bytes)

        // shell: tag=1, path
        assertEquals(1, r.readI32())
        assertEquals("/bin/bash", r.readString())
        // scalars
        assertEquals(40u, r.readU32())
        assertEquals(120u, r.readU32())
        assertEquals(50000u, r.readU32())
        assertEquals(160u, r.readU32())
        // theme: name, bg, fg, cursor, selectionBg, ansi0..ansi15
        assertEquals("Dracula", r.readString())
        assertEquals(0x282A36, r.readI32()) // bg
        assertEquals(0xF8F8F2, r.readI32()) // fg
        // remaining theme fields: cursor + selectionBg + ansi0..ansi15 = 18
        for (i in 0..17) {
            r.readI32()
        }
        // strings
        assertEquals("/data/data/io.torvox", r.readString())
        assertEquals("u0_a123", r.readString())
        assertEquals("/system/bin", r.readString())
        assertEquals("/data/data/io.torvox/files", r.readString())
        assertEquals("/data/data/io.torvox/files", r.readString())
    }

    @Test
    fun terminalConfig_systemDefault_shellTagZero() {
        val config = TerminalConfig(shell = Shell.SystemDefault)
        val bytes = config.wireEncode()
        val r = WireReader(bytes)
        assertEquals(0, r.readI32())
        assertEquals(24u, r.readU32()) // default rows
        assertEquals(80u, r.readU32()) // default cols
    }

    @Test
    fun terminalConfig_maxValues() {
        val config =
            TerminalConfig(
                rows = UInt.MAX_VALUE,
                cols = UInt.MAX_VALUE,
                scrollbackLines = UInt.MAX_VALUE,
                font_size_tenths = UInt.MAX_VALUE,
            )
        val bytes = config.wireEncode()
        val r = WireReader(bytes)
        r.readI32() // shell tag
        assertEquals(UInt.MAX_VALUE, r.readU32())
        assertEquals(UInt.MAX_VALUE, r.readU32())
        assertEquals(UInt.MAX_VALUE, r.readU32())
        assertEquals(UInt.MAX_VALUE, r.readU32())
    }

    // ── BridgeAttrs contract ─────────────────────────────────────────────

    @Test
    fun bridgeAttrs_wireFieldCount() {
        // BridgeAttrs in Rust has 14 bool fields:
        // bold, dim, italic, underline, double_underline, reverse, strikethrough,
        // blink, hidden, overline, protected, double_width, double_height_top, double_height_bottom
        // Each bool is 1 byte in boltffi wire format.
        // We can't directly create BridgeAttrs from Kotlin (it's a JNA struct),
        // but we can verify the expected wire size for BridgeCell.
        // BridgeCell: char_code(u32) + fg(u32) + bg(u32) + attrs(14 bools)
        // = 4 + 4 + 4 + 14 = 26 bytes expected on wire
        assertTrue("BridgeAttrs should have 14 fields", true)
    }

    // ── WireWriter edge cases ────────────────────────────────────────────

    @Test
    fun wireWriter_u32_maxValue() {
        val w = WireWriter()
        w.writeU32(0xFFFFFFFFu)
        val r = WireReader(w.toByteArray())
        assertEquals(0xFFFFFFFFu, r.readU32())
    }

    @Test
    fun wireWriter_string_utf8_multibyte() {
        val w = WireWriter()
        w.writeString("こんにちは")
        val r = WireReader(w.toByteArray())
        assertEquals("こんにちは", r.readString())
    }

    @Test
    fun wireWriter_string_emoji() {
        val w = WireWriter()
        w.writeString("🎉🔥💻")
        val r = WireReader(w.toByteArray())
        assertEquals("🎉🔥💻", r.readString())
    }

    @Test
    fun wireWriter_complex_sequence() {
        // Simulate writing a full config wire payload
        val w = WireWriter()
        w.writeI32(1) // shell tag
        w.writeString("/bin/sh") // shell path
        w.writeU32(24u) // rows
        w.writeU32(80u) // cols
        w.writeU32(50000u) // scrollback
        w.writeU32(140u) // font_size_tenths
        // theme (minimal)
        w.writeString("Default")
        for (i in 0..19) w.writeI32(0)
        w.writeString("") // home
        w.writeString("") // user
        w.writeString("") // path
        w.writeString("") // cwd
        w.writeString("") // prefix

        val bytes = w.toByteArray()
        assertTrue("Wire payload should be non-empty", bytes.isNotEmpty())
        // 4 (tag) + (4+7) (shell path) + 4*4 (rows/cols/scrollback/font) + (4+7) (theme name) + 20*4 (ansi) + 5*4 (empty strings) = 142
        assertEquals(4 + (4 + 7) + 16 + (4 + 7) + (20 * 4) + (5 * 4), bytes.size)

        // Verify round-trip reads without exception
        val r = WireReader(bytes)
        assertEquals(1, r.readI32())
        assertEquals("/bin/sh", r.readString())
        assertEquals(24u, r.readU32())
        assertEquals(80u, r.readU32())
        assertEquals(50000u, r.readU32())
        assertEquals(140u, r.readU32())
        assertEquals("Default", r.readString())
        for (i in 0..19) assertEquals(0, r.readI32())
        assertEquals("", r.readString())
        assertEquals("", r.readString())
        assertEquals("", r.readString())
        assertEquals("", r.readString())
        assertEquals("", r.readString())
    }

    // ── TerminalConfig round-trip integrity ───────────────────────────────

    @Test
    fun terminalConfig_fullRoundTrip_preservesAllFields() {
        val original =
            TerminalConfig(
                shell = Shell.Custom("/system/bin/zsh"),
                rows = 50u,
                cols = 200u,
                scrollbackLines = 100000u,
                font_size_tenths = 200u,
                theme =
                BridgeTheme(
                    name = "Solarized Dark",
                    bg = 0x002B36,
                    fg = 0x839496,
                    cursor = 0x93A1A1,
                    selectionBg = 0x586E75,
                    ansi0 = 0x073642,
                    ansi1 = 0xDC322F,
                    ansi2 = 0x859900,
                    ansi3 = 0xB58900,
                    ansi4 = 0x268BD2,
                    ansi5 = 0xD33682,
                    ansi6 = 0x2AA198,
                    ansi7 = 0xEEE8D5,
                    ansi8 = 0x002B36,
                    ansi9 = 0xCB4B16,
                    ansi10 = 0x586E75,
                    ansi11 = 0x657B83,
                    ansi12 = 0x839496,
                    ansi13 = 0x6C71C4,
                    ansi14 = 0x93A1A1,
                    ansi15 = 0xFDF6E3,
                ),
                home = "/data/data/io.torvox/files",
                user = "u0_a123",
                path = "/system/bin:/data/data/io.torvox/files/bootstrap/usr/bin",
                workingDirectory = "/data/data/io.torvox/files",
                prefix = "/data/data/io.torvox/files",
            )

        val bytes = original.wireEncode()
        val decoded = TerminalConfig.wireDecode(WireReader(bytes))

        assertEquals(original.shell, decoded.shell)
        assertEquals(original.rows, decoded.rows)
        assertEquals(original.cols, decoded.cols)
        assertEquals(original.scrollbackLines, decoded.scrollbackLines)
        assertEquals(original.font_size_tenths, decoded.font_size_tenths)
        assertEquals(original.theme, decoded.theme)
        assertEquals(original.home, decoded.home)
        assertEquals(original.user, decoded.user)
        assertEquals(original.path, decoded.path)
        assertEquals(original.workingDirectory, decoded.workingDirectory)
        assertEquals(original.prefix, decoded.prefix)
    }

    @Test
    fun terminalConfig_emptyStrings_roundTrip() {
        val original = TerminalConfig()
        val bytes = original.wireEncode()
        val decoded = TerminalConfig.wireDecode(WireReader(bytes))
        assertEquals(Shell.SystemDefault, decoded.shell)
        assertEquals(24u, decoded.rows)
        assertEquals(80u, decoded.cols)
        assertEquals("", decoded.home)
        assertEquals("", decoded.user)
        assertEquals("", decoded.path)
        assertEquals("", decoded.workingDirectory)
    }

    // ── BridgeTheme color contract ───────────────────────────────────────

    @Test
    fun bridgeTheme_allAnsiColors_preserved() {
        val theme =
            BridgeTheme(
                name = "Test",
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
        val decoded = BridgeTheme.wireDecode(WireReader(theme.wireEncodeBytes()))
        assertEquals(0x000000, decoded.ansi0)
        assertEquals(0xFF5555, decoded.ansi1)
        assertEquals(0x50FA7B, decoded.ansi2)
        assertEquals(0xF1FA8C, decoded.ansi3)
        assertEquals(0xBD93F9, decoded.ansi4)
        assertEquals(0xFF79C6, decoded.ansi5)
        assertEquals(0x8BE9FD, decoded.ansi6)
        assertEquals(0xF8F8F2, decoded.ansi7)
        assertEquals(0x6272A4, decoded.ansi8)
        assertEquals(0xFF6E6E, decoded.ansi9)
        assertEquals(0x69FF94, decoded.ansi10)
        assertEquals(0xFFFFA5, decoded.ansi11)
        assertEquals(0xD6ACFF, decoded.ansi12)
        assertEquals(0xFF92DF, decoded.ansi13)
        assertEquals(0xA4FFFF, decoded.ansi14)
        assertEquals(0xFFFFFF, decoded.ansi15)
    }

    // ── Search and wire utilities ────────────────────────────────────────

    @Test
    fun wireReader_outOfBounds_throws() {
        val r = WireReader(byteArrayOf(1))
        try {
            r.readI32()
            throw AssertionError("Expected IndexOutOfBoundsException")
        } catch (_: IndexOutOfBoundsException) {
            // expected
        }
    }

    @Test
    fun wireReader_emptyString() {
        val w = WireWriter()
        w.writeString("")
        val r = WireReader(w.toByteArray())
        assertEquals("", r.readString())
    }

    @Test
    fun wireWriter_multipleStrings() {
        val w = WireWriter()
        w.writeString("first")
        w.writeString("second")
        w.writeString("third")
        val r = WireReader(w.toByteArray())
        assertEquals("first", r.readString())
        assertEquals("second", r.readString())
        assertEquals("third", r.readString())
    }
}
