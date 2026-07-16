package io.torvox.installer

import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test
import java.io.File

class BootstrapInstallerTest {
    private val prefixDir = File("/tmp/test-prefix")
    private val homeDir = File("/tmp/test-home")
    private val stagingDir = File("/tmp/test-staging")
    private val installer = BootstrapInstaller(prefixDir, homeDir, stagingDir)

    @Test
    fun `parseSymlinks with empty string returns empty list`() {
        assertTrue(installer.parseSymlinks("").isEmpty())
    }

    @Test
    fun `parseSymlinks with blank string returns empty list`() {
        assertTrue(installer.parseSymlinks("   ").isEmpty())
        assertTrue(installer.parseSymlinks("\n").isEmpty())
        assertTrue(installer.parseSymlinks("\r\n").isEmpty())
    }

    @Test
    fun `parseSymlinks with old style arrow separator`() {
        val result = installer.parseSymlinks("../foo.bar -> usr/lib/foo.bar")
        assertEquals(listOf("../foo.bar" to "usr/lib/foo.bar"), result)
    }

    @Test
    fun `parseSymlinks with left arrow unicode separator`() {
        val result = installer.parseSymlinks("../foo.bar\u2190usr/lib/foo.bar")
        assertEquals(listOf("../foo.bar" to "usr/lib/foo.bar"), result)
    }

    @Test
    fun `parseSymlinks with right arrow unicode separator`() {
        val result = installer.parseSymlinks("../foo.bar\u2192usr/lib/foo.bar")
        assertEquals(listOf("../foo.bar" to "usr/lib/foo.bar"), result)
    }

    @Test
    fun `parseSymlinks with left-right arrow unicode separator`() {
        val result = installer.parseSymlinks("../foo.bar\u2194usr/lib/foo.bar")
        assertEquals(listOf("../foo.bar" to "usr/lib/foo.bar"), result)
    }

    @Test
    fun `parseSymlinks with arrow separator and surrounding spaces`() {
        val result = installer.parseSymlinks("  ../foo.bar   ->   usr/lib/foo.bar  ")
        assertEquals(listOf("../foo.bar" to "usr/lib/foo.bar"), result)
    }

    @Test
    fun `parseSymlinks with unicode arrow and spaces`() {
        val result = installer.parseSymlinks("  ../foo.bar  \u2190  usr/lib/foo.bar  ")
        assertEquals(listOf("../foo.bar" to "usr/lib/foo.bar"), result)
    }

    @Test
    fun `parseSymlinks with multiple entries`() {
        val content =
            """
            ../foo.bar -> usr/lib/foo.bar
            ../baz.qux ← usr/lib/baz.qux
            ../abc.def → usr/lib/abc.def
            """.trimIndent()
        val result = installer.parseSymlinks(content)
        assertEquals(3, result.size)
        assertEquals("../foo.bar" to "usr/lib/foo.bar", result[0])
        assertEquals("../baz.qux" to "usr/lib/baz.qux", result[1])
        assertEquals("../abc.def" to "usr/lib/abc.def", result[2])
    }

    @Test
    fun `parseSymlinks with malformed line returns null filtered`() {
        val content =
            """
            ../foo.bar -> usr/lib/foo.bar
            just_a_single_part
            ../baz.qux -> usr/lib/baz.qux -> extra_part
            """.trimIndent()
        val result = installer.parseSymlinks(content)
        assertEquals(1, result.size)
        assertEquals("../foo.bar" to "usr/lib/foo.bar", result[0])
    }

    @Test
    fun `parseSymlinks filters empty lines`() {
        val content =
            """
            ../foo.bar -> usr/lib/foo.bar

            ../baz.qux -> usr/lib/baz.qux

            """.trimIndent()
        val result = installer.parseSymlinks(content)
        assertEquals(2, result.size)
    }

    @Test
    fun `parseSymlinks trims whitespace in both parts`() {
        val content = "  ../foo.bar  ->  usr/lib/foo.bar  "
        val result = installer.parseSymlinks(content)
        assertEquals(listOf("../foo.bar" to "usr/lib/foo.bar"), result)
    }

    @Test
    fun `symlinkSeparator handles old arrow format at start of real content`() {
        val line = "../foo/bar.h -> include/foo/bar.h"
        val parts = line.split(installer.symlinkSeparator)
        assertEquals(2, parts.size)
        assertEquals("../foo/bar.h", parts[0].trim())
        assertEquals("include/foo/bar.h", parts[1].trim())
    }

    @Test
    fun `symlinkSeparator handles unicode arrow format`() {
        val line = "../foo/bar.h\u2190include/foo/bar.h"
        val parts = line.split(installer.symlinkSeparator)
        assertEquals(2, parts.size)
        assertEquals("../foo/bar.h", parts[0].trim())
        assertEquals("include/foo/bar.h", parts[1].trim())
    }

    @Test
    fun `parseSymlinks handles real-world like mix of separators`() {
        val content =
            """
            ../term.h -> include/ncurses/term.h
../term_entry.h${"\u2190"}include/ncurses/term_entry.h
../unctrl.h -> include/ncurses/unctrl.h
            """.trimIndent()
        val result = installer.parseSymlinks(content)
        assertEquals(3, result.size)
        assertEquals("../term.h" to "include/ncurses/term.h", result[0])
        assertEquals("../term_entry.h" to "include/ncurses/term_entry.h", result[1])
        assertEquals("../unctrl.h" to "include/ncurses/unctrl.h", result[2])
    }

    @Test
    fun `symlinkSeparator does not split on single hyphen`() {
        val line = "/some-path-with-hyphens -> /target-path-with-hyphens"
        val parts = line.split(installer.symlinkSeparator)
        assertEquals(2, parts.size)
        assertEquals("/some-path-with-hyphens", parts[0].trim())
        assertEquals("/target-path-with-hyphens", parts[1].trim())
    }
}
