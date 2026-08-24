package terminal.emulator.runtime

import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test
import java.io.File

/**
 * isElf — the linker-wrapper gate: only real ELF binaries may be execve'd
 * through the prefix-shell path; shebang scripts and corrupt files must be
 * excluded. BootstrapInstallerTest reuses ELF magic when faking anchors,
 * but this function itself had no direct coverage.
 */
class IsElfTest {
    private fun fileWith(vararg bytes: Byte): File = File.createTempFile("elf-magic-test", ".bin").apply {
        writeBytes(bytes)
        deleteOnExit()
    }

    @Test
    fun `real elf magic is detected`() {
        val file = fileWith(0x7f.toByte(), 'E'.code.toByte(), 'L'.code.toByte(), 'F'.code.toByte(), 2, 0, 1)
        assertTrue(isElf(file))
    }

    @Test
    fun `shebang script is not an elf`() {
        val file = fileWith(*"#!/bin/sh\n".toByteArray())
        assertFalse(isElf(file))
    }

    @Test
    fun `text file is not an elf`() {
        val file = fileWith(*"hello world".toByteArray())
        assertFalse(isElf(file))
    }

    @Test
    fun `empty file is not an elf`() {
        val file = fileWith()
        assertFalse(isElf(file))
    }

    @Test
    fun `too short magic prefixes are not an elf`() {
        val file = fileWith(0x7f.toByte(), 'E'.code.toByte())
        assertFalse(isElf(file))
    }

    @Test
    fun `missing file is not an elf`() {
        assertFalse(isElf(File("/nonexistent/definitely-missing")))
    }
}
