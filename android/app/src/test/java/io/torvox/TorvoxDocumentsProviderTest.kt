package io.torvox

import android.provider.DocumentsContract.Document
import androidx.test.core.app.ApplicationProvider
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import java.io.File

@RunWith(RobolectricTestRunner::class)
class TorvoxDocumentsProviderTest {
    private lateinit var testDir: File
    private lateinit var testRootDir: File

    @Before
    fun setUp() {
        val context = ApplicationProvider.getApplicationContext<android.app.Application>()
        testDir = File(context.filesDir, "test_home_${System.nanoTime()}")
        testDir.mkdirs()
        testRootDir = File(context.filesDir, "test_root_${System.nanoTime()}")
        testRootDir.mkdirs()
    }

    @Test
    fun encodeDocIdDecodeDocIdRoundTrip() {
        val file = File(testRootDir, "test.txt")
        file.createNewFile()
        val docId = TorvoxDocumentsProvider.encodeDocId(file, testRootDir)
        assertEquals("test.txt", docId)
        val decoded = TorvoxDocumentsProvider.decodeDocId(docId, testRootDir)
        assertEquals(file.absolutePath, decoded.absolutePath)
    }

    @Test
    fun decodeDocIdCreatesFileObject() {
        val path = "some/path/file.txt"
        val file = TorvoxDocumentsProvider.decodeDocId(path, testRootDir)
        assertEquals(File(testRootDir, path).absolutePath, file.absolutePath)
    }

    @Test
    fun encodeDocIdReturnsRelativePath() {
        val file = File(testRootDir, "test")
        file.createNewFile()
        val docId = TorvoxDocumentsProvider.encodeDocId(file, testRootDir)
        assertEquals("test", docId)
    }

    @Test
    fun isChildDocumentWithNestedPath() {
        val parent = File("/data/data/com.termux/files/home")
        val child = File("/data/data/com.termux/files/home/subdir/file.txt")
        assertTrue(child.absolutePath.startsWith(parent.absolutePath + File.separator))
    }

    @Test
    fun isChildDocumentWithSamePath() {
        val parent = File("/data/data/com.termux/files/home")
        assertFalse(parent.absolutePath.startsWith(parent.absolutePath + File.separator))
    }

    @Test
    fun isChildDocumentWithUnrelatedPath() {
        val parent = File("/data/data/com.termux/files/home")
        val child = File("/data/data/com.termux/files/other/file.txt")
        assertFalse(child.absolutePath.startsWith(parent.absolutePath + File.separator))
    }

    @Test
    fun documentMimeTypeDirConstant() {
        assertEquals("vnd.android.document/directory", Document.MIME_TYPE_DIR)
    }

    @Test
    fun createAndDeleteFile() {
        val file = File(testDir, "test_file.txt")
        assertTrue(file.createNewFile())
        assertTrue(file.exists())
        assertTrue(file.delete())
        assertFalse(file.exists())
    }

    @Test
    fun createAndDeleteDirectory() {
        val dir = File(testDir, "test_dir")
        assertTrue(dir.mkdirs())
        assertTrue(dir.isDirectory)
        assertTrue(dir.deleteRecursively())
        assertFalse(dir.exists())
    }

    @Test
    fun listDirectoryShowsChildren() {
        File(testDir, "file.txt").writeText("content")
        File(testDir, "subdir").mkdirs()

        val children = testDir.listFiles()
        assertNotNull(children)
        assertEquals(2, children!!.size)
        val names = children.map { it.name }.toSet()
        assertTrue("file.txt" in names)
        assertTrue("subdir" in names)
    }

    @Test
    fun directoriesSortedFirst() {
        File(testDir, "aaa.txt").writeText("a")
        File(testDir, "zzz_dir").mkdirs()
        File(testDir, "bbb.txt").writeText("b")

        val children =
            testDir.listFiles()!!.sortedWith(
                compareByDescending<File> { it.isDirectory }.thenBy { it.name.lowercase() },
            )
        assertEquals(3, children.size)
        assertTrue(children[0].isDirectory)
        assertEquals("aaa.txt", children[1].name)
        assertEquals("bbb.txt", children[2].name)
    }

    @Test
    fun encodeDocIdWithSpecialCharacters() {
        val subDir = File(testRootDir, "path with spaces")
        subDir.mkdirs()
        val file = File(subDir, "file (1).txt")
        file.createNewFile()
        val docId = TorvoxDocumentsProvider.encodeDocId(file, testRootDir)
        val decoded = TorvoxDocumentsProvider.decodeDocId(docId, testRootDir)
        assertEquals(file.absolutePath, decoded.absolutePath)
    }

    @Test
    fun encodeDocIdDeepPath() {
        val subDir = File(testRootDir, "a/b/c/d/e")
        subDir.mkdirs()
        val file = File(subDir, "f.txt")
        file.createNewFile()
        val docId = TorvoxDocumentsProvider.encodeDocId(file, testRootDir)
        assertEquals("a/b/c/d/e/f.txt", docId)
    }
}
