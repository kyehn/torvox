package terminal.emulator.bridge

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test
import java.io.File
import java.nio.file.Files

/**
 * `~/.termux/font.ttf` 探测缓存：同一文件只探测一次，内容变化才重新探测。
 *
 * Bridge 按会话创建，缓存必须是进程级；实例级缓存会让每次新建会话都把同一文件
 * 再装进原生字体库（2026-11-19 运行日志的第二次 `loadFontFile`）。
 */
class FontProbeCacheTest {
    @Test
    fun `same file is probed once until size or mtime changes`() {
        val dir = Files.createTempDirectory("font-probe-cache").toFile()
        try {
            val font = File(dir, "font.ttf").apply { writeBytes(ByteArray(8)) }
            var loads = 0
            val loadFamily: (String) -> String? = {
                loads++
                FAMILY
            }
            val cache = FontProbeCache()

            assertEquals(FAMILY, cache.familyOf(font, loadFamily))
            assertEquals(FAMILY, cache.familyOf(font, loadFamily))
            assertEquals(1, loads)

            font.appendBytes(ByteArray(4))
            assertEquals(FAMILY, cache.familyOf(font, loadFamily))
            assertEquals(2, loads)

            assertNull(cache.familyOf(null, loadFamily))
            assertEquals(FAMILY, cache.familyOf(font, loadFamily))
            assertEquals(3, loads)
        } finally {
            dir.deleteRecursively()
        }
    }

    private companion object {
        const val FAMILY = "Probe Family"
    }
}
