package terminal.emulator

import android.provider.DocumentsContract
import androidx.test.platform.app.InstrumentationRegistry
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test
import java.io.File

class DocumentsProviderInstrumentedTest {
    private val authority = "com.termux.documents"

    @Test
    fun roots_via_resolver_returns_terminal_home() {
        val context = InstrumentationRegistry.getInstrumentation().targetContext
        val cursor = context.contentResolver.query(
            DocumentsContract.buildRootsUri(authority),
            null,
            null,
            null,
            null,
        )
        requireNotNull(cursor).use {
            assertTrue(it.count >= 1)
            it.moveToFirst()
            assertEquals(
                "terminal_home",
                it.getString(it.getColumnIndexOrThrow(DocumentsContract.Root.COLUMN_ROOT_ID)),
            )
        }
    }

    @Test
    fun subset_projection_does_not_crash() {
        val context = InstrumentationRegistry.getInstrumentation().targetContext
        val cursor = context.contentResolver.query(
            DocumentsContract.buildRootsUri(authority),
            arrayOf(
                DocumentsContract.Root.COLUMN_ROOT_ID,
                DocumentsContract.Root.COLUMN_FLAGS,
            ),
            null,
            null,
            null,
        )
        requireNotNull(cursor).use {
            assertTrue(it.count >= 1)
            it.moveToFirst()
            assertEquals(
                "terminal_home",
                it.getString(it.getColumnIndexOrThrow(DocumentsContract.Root.COLUMN_ROOT_ID)),
            )
        }
    }

    @Test
    fun child_documents_round_trip_create_delete() {
        val context = InstrumentationRegistry.getInstrumentation().targetContext
        val home = File(context.filesDir, "home").apply { mkdirs() }
        val probe = File(home, "instrumented-probe.txt")
        if (!probe.exists()) probe.writeText("probe")
        val childUri = DocumentsContract.buildChildDocumentsUri(authority, "terminal_home")
        val cursor = context.contentResolver.query(childUri, null, null, null, null)
        requireNotNull(cursor).use {
            val idIndex = it.getColumnIndexOrThrow(DocumentsContract.Document.COLUMN_DOCUMENT_ID)
            var found = false
            while (it.moveToNext()) {
                if (it.getString(idIndex) == "instrumented-probe.txt") found = true
            }
            assertTrue("created probe must be listed", found)
        }
        val docUri = DocumentsContract.buildDocumentUri(authority, "instrumented-probe.txt")
        assertTrue(DocumentsContract.deleteDocument(context.contentResolver, docUri))
        assertTrue(!probe.exists())
    }

    /**
     * 跨进程写回：经 `ContentResolver` 打开文档 URI 写入，验证内容真正落盘。
     *
     * Robolectric 单测直调 provider 对象，绕过 `ContentProvider.Transport` 与清单属性，
     * 覆盖不到「其他应用能编辑并回写」这条契约（DESIGN 终端页文本选择节）。
     */
    @Test
    fun external_process_write_back_lands_on_disk() {
        val context = InstrumentationRegistry.getInstrumentation().targetContext
        val home = File(context.filesDir, "home").apply { mkdirs() }
        val probe = File(home, "write-back-probe.txt")
        probe.writeText("stale content that must be truncated away")
        val docUri = DocumentsContract.buildDocumentUri(authority, "write-back-probe.txt")

        context.contentResolver.openOutputStream(docUri, "rwt").use { stream ->
            requireNotNull(stream) { "provider must open a writable stream" }
            stream.write("edited by another app".toByteArray())
        }
        assertEquals("edited by another app", probe.readText())

        // 追加语义：wa 不截断。
        context.contentResolver.openOutputStream(docUri, "wa").use { stream ->
            requireNotNull(stream) { "provider must open an append stream" }
            stream.write("|more".toByteArray())
        }
        assertEquals("edited by another app|more", probe.readText())
    }

    /** 读取通道对不持 `MANAGE_DOCUMENTS` 的进程开放（清单不得声明无效权限）。 */
    @Test
    fun external_process_read_succeeds_without_manage_documents() {
        val context = InstrumentationRegistry.getInstrumentation().targetContext
        val home = File(context.filesDir, "home").apply { mkdirs() }
        File(home, "external-read-probe.txt").writeText("readable")
        val docUri = DocumentsContract.buildDocumentUri(authority, "external-read-probe.txt")
        context.contentResolver.openInputStream(docUri).use { stream ->
            val text = requireNotNull(stream) { "read stream must open" }.readBytes().decodeToString()
            assertEquals("readable", text)
        }
    }
}
