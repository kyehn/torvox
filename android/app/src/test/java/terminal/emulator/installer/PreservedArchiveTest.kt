package terminal.emulator.installer

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Assert.fail
import org.junit.Rule
import org.junit.Test
import org.junit.rules.TemporaryFolder
import java.io.File
import java.io.IOException

/**
 * 离线安装（SAF 选包）的中转包生命周期：中转包落在用户数据树内，系统从不清理，
 * 故复制与安装**全过程**结束后都必须删掉它——复制中途失败留下的半截内容同样是
 * 永久占用（引导归档有数百 MB）。
 */
class PreservedArchiveTest {
    @get:Rule
    val temporaryFolder = TemporaryFolder()

    @Test
    fun `archive is handed to the block and removed afterwards`() {
        val source = temporaryFolder.newFile("source.zip").apply { writeText("payload") }
        val target = File(temporaryFolder.root, "preserved.zip")

        val seen = withPreservedArchive(target, source) { archive ->
            assertTrue("块内必须拿到已复制好的中转包", archive.isFile)
            archive.readText()
        }

        assertEquals("payload", seen)
        assertFalse("结束后中转包必须已删除", target.exists())
    }

    @Test
    fun `copy failure leaves no partial archive behind`() {
        val missing = File(temporaryFolder.root, "missing.zip")
        val target = File(temporaryFolder.root, "preserved.zip")

        try {
            withPreservedArchive(target, missing) { fail("源文件不存在时必须失败") }
            fail("源文件不存在时必须失败")
        } catch (expected: IOException) {
            assertTrue(expected.message.orEmpty().isNotEmpty())
        }
        assertFalse("复制失败后不得留下半截中转包", target.exists())
    }

    @Test
    fun `block failure still removes the archive`() {
        val source = temporaryFolder.newFile("source.zip").apply { writeText("payload") }
        val target = File(temporaryFolder.root, "preserved.zip")

        try {
            withPreservedArchive(target, source) { throw IOException("install failed") }
            fail("块内异常必须外传")
        } catch (expected: IOException) {
            assertEquals("install failed", expected.message)
        }
        assertFalse("安装失败后中转包也必须已删除", target.exists())
    }
}
