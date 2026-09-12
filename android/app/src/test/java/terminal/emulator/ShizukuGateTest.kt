package terminal.emulator

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test
import java.io.File

/** ShizukuGate ELF sniffing: synthetic fixtures only, no host assumptions. */
class ShizukuGateTest {

    private fun put16(data: ByteArray, at: Int, value: Int) {
        data[at] = (value and 0xff).toByte()
        data[at + 1] = ((value shr 8) and 0xff).toByte()
    }

    private fun put32(data: ByteArray, at: Int, value: Long) {
        for (shift in 0 until 4) data[at + shift] = ((value shr (8 * shift)) and 0xff).toByte()
    }

    private fun put64(data: ByteArray, at: Int, value: Long) {
        for (shift in 0 until 8) data[at + shift] = ((value shr (8 * shift)) and 0xff).toByte()
    }

    /** Minimal 64-bit ELF with one PT_INTERP entry pointing at [interp]. */
    private fun syntheticElf(interp: String?): File {
        val name = interp?.toByteArray() ?: ByteArray(0)
        val file = File.createTempFile("fake-elf", ".bin")
        val size = 128 + name.size + 1
        val data = ByteArray(size)
        data[0] = 0x7f.toByte()
        data[1] = 'E'.code.toByte()
        data[2] = 'L'.code.toByte()
        data[3] = 'F'.code.toByte()
        data[4] = 2.toByte() // ELFCLASS64
        put64(data, 0x20, 64) // e_phoff
        put16(data, 0x36, 56) // e_phentsize
        put16(data, 0x38, if (interp == null) 0 else 1) // e_phnum
        if (interp != null) {
            put32(data, 64, 3) // p_type = PT_INTERP
            put64(data, 72, 128) // p_offset
            name.copyInto(data, 128)
        }
        file.writeBytes(data)
        return file
    }

    @Test
    fun readInterp_nixInterp_detected() {
        val interp = "/nix/store/abc-glibc-2.42/lib/ld-linux-x86-64.so.2"
        val file = syntheticElf(interp)
        try {
            assertEquals(interp, ShizukuGate.readInterp(file.absolutePath))
        } finally {
            file.delete()
        }
    }

    @Test
    fun readInterp_noPhdrs_empty() {
        val file = syntheticElf(null)
        try {
            assertEquals("", ShizukuGate.readInterp(file.absolutePath))
        } finally {
            file.delete()
        }
    }

    @Test
    fun readInterp_script_empty() {
        val file = File.createTempFile("probe", ".sh")
        file.writeText("#!/system/bin/sh\necho hi\n")
        try {
            assertEquals("", ShizukuGate.readInterp(file.absolutePath))
        } finally {
            file.delete()
        }
    }

    @Test
    fun chainLoadPrefix_script_null() {
        val file = File.createTempFile("probe", ".sh")
        file.writeText("#!/system/bin/sh\n")
        try {
            assertNull(ShizukuGate.chainLoadPrefix(file.absolutePath, requireNotNull(file.parent)))
        } finally {
            file.delete()
        }
    }

    @Test
    fun chainLoadPrefix_nixElf_usesPrefixLoader() {
        val prefix: File = requireNotNull(kotlin.io.path.createTempDirectory("fake-prefix").toFile())
        try {
            val libDir: File = File(prefix, "nix/store/abc-glibc-2.42/lib")
            libDir.mkdirs()
            val loader: File = File(libDir, "ld-linux-x86-64.so.2")
            loader.writeBytes(byteArrayOf(0x7f, 'E'.code.toByte()))
            val login: File = File(prefix, "bin/login")
            requireNotNull(login.parentFile).mkdirs()
            val interp = "/nix/store/abc-glibc-2.42/lib/ld-linux-x86-64.so.2"
            val elf = syntheticElf(interp)
            elf.renameTo(login)
            val command =
                requireNotNull(ShizukuGate.chainLoadPrefix(login.absolutePath, prefix.absolutePath))
            assertTrue(command.contains(loader.absolutePath))
            assertTrue(command.contains(login.absolutePath))
        } finally {
            prefix.deleteRecursively()
        }
    }
}
