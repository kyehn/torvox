package terminal.emulator

import android.provider.DocumentsContract
import androidx.test.platform.app.InstrumentationRegistry
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test
import terminal.emulator.TerminalLogcatTest
import java.io.File

class DocumentsProviderInstrumentedTest : TerminalLogcatTest() {
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

    @Test
    fun resolver_create_then_modify_round_trip() {
        val context = InstrumentationRegistry.getInstrumentation().targetContext
        val home = File(context.filesDir, "home").apply { mkdirs() }
        File(home, "resolver-create-probe.txt").delete()
        val parentUri = DocumentsContract.buildChildDocumentsUri(authority, "terminal_home")
        val createdUri = DocumentsContract.createDocument(
            context.contentResolver,
            parentUri,
            "text/plain",
            "resolver-create-probe.txt",
        )
        requireNotNull(createdUri) { "resolver create must return a uri" }
        context.contentResolver.openOutputStream(createdUri, "rwt").use { stream ->
            requireNotNull(stream) { "created file must be writable" }
            stream.write("created then edited".toByteArray())
        }
        assertEquals("created then edited", File(home, "resolver-create-probe.txt").readText())
        assertTrue(DocumentsContract.deleteDocument(context.contentResolver, createdUri))
    }

    @Test
    fun resolver_create_directory_round_trip() {
        val context = InstrumentationRegistry.getInstrumentation().targetContext
        val home = File(context.filesDir, "home").apply { mkdirs() }
        File(home, "resolver-dir-probe").deleteRecursively()
        val parentUri = DocumentsContract.buildChildDocumentsUri(authority, "terminal_home")
        val createdUri = DocumentsContract.createDocument(
            context.contentResolver,
            parentUri,
            DocumentsContract.Document.MIME_TYPE_DIR,
            "resolver-dir-probe",
        )
        requireNotNull(createdUri) { "resolver create dir must return a uri" }
        assertTrue("created dir must land on disk", File(home, "resolver-dir-probe").isDirectory)
        assertTrue(DocumentsContract.deleteDocument(context.contentResolver, createdUri))
    }

    /**
     * 嵌套目录走 tree URI：SAF 的 tree URI 按路径段定位文档（`tree/<root>/document/<docId>`），
     * 而本提供者的 docId 是含 `/` 的相对路径，因此「docId 能否在 tree URI 里原样往返」只有
     * 走客户端真实路径才验得到——`DocumentsUI` 进入子目录后再新建文件正是这条路径。
     * 直调 provider 的单测拿不到这一层（它绕过了 `DocumentsContract` 的 URI 编解码）。
     *
     * 建目录的 parent 用 `buildDocumentUri`（`DocumentsUI` 的真实做法：框架的
     * `enforceTree` 对裸 `tree/<docId>` 会因 `getDocumentId` 抛 `IllegalArgumentException`，
     * 客户端因此从不这样传）；在子目录里建文件则用 tree 形式的 children URI。
     */
    @Test
    fun tree_uri_nested_create_and_write_back_round_trip() {
        val context = InstrumentationRegistry.getInstrumentation().targetContext
        val home = File(context.filesDir, "home").apply { mkdirs() }
        val nestedDirName = "tree-uri-nested-dir"
        File(home, nestedDirName).deleteRecursively()

        val rootDocumentUri = DocumentsContract.buildDocumentUri(authority, "terminal_home")
        val rootTreeUri = DocumentsContract.buildTreeDocumentUri(authority, "terminal_home")
        fun dirOnDisk(name: String): File = File(home, name)
        val nestedDirUri =
            DocumentsContract.createDocument(
                context.contentResolver,
                rootDocumentUri,
                DocumentsContract.Document.MIME_TYPE_DIR,
                nestedDirName,
            )
        requireNotNull(nestedDirUri) { "nested dir create must return a uri" }
        assertTrue("created dir must land on disk", File(home, nestedDirName).isDirectory)

        // 子目录的 docId 是相对路径，必须能原样取回，否则后续寻址全部指错。
        val nestedDocId = DocumentsContract.getDocumentId(nestedDirUri)
        assertEquals(nestedDirName, nestedDocId)

        val childrenInNested =
            DocumentsContract.buildChildDocumentsUriUsingTree(rootTreeUri, nestedDocId)
        val createdFileUri =
            DocumentsContract.createDocument(
                context.contentResolver,
                childrenInNested,
                "text/plain",
                "tree-uri-nested.txt",
            )
        requireNotNull(createdFileUri) { "tree URI create file must return a uri" }

        context.contentResolver.openOutputStream(createdFileUri, "rwt").use { stream ->
            requireNotNull(stream) { "nested created file must be writable" }
            stream.write("nested write back".toByteArray())
        }
        assertEquals("nested write back", File(dirOnDisk(nestedDirName), "tree-uri-nested.txt").readText())

        // 按 docId 经 tree 形式重新寻址必须还是同一个目录（子 docId 含 '/'，靠 URI 编解码往返）。
        val nestedViaTree = DocumentsContract.buildDocumentUriUsingTree(rootTreeUri, nestedDocId)
        assertEquals(
            DocumentsContract.Document.MIME_TYPE_DIR,
            context.contentResolver.getType(nestedViaTree),
        )
        // 子目录里的子文档 docId 是**相对根**的路径（客户端须原样回传）：
        // 若这里只回文件名，客户端后续的 open/delete 就会打到根目录同名文件上。
        val childrenCursor =
            context.contentResolver.query(childrenInNested, null, null, null, null)
        val nestedFileDocId =
            requireNotNull(childrenCursor).use {
                val idIndex = it.getColumnIndexOrThrow(DocumentsContract.Document.COLUMN_DOCUMENT_ID)
                val ids = mutableListOf<String>()
                while (it.moveToNext()) ids.add(it.getString(idIndex))
                ids.singleOrNull()
            }
        assertEquals("$nestedDirName/tree-uri-nested.txt", nestedFileDocId)

        // 删「嵌套目录里的文件」：客户端用列举时拿到的 docId 原样删除。
        val deleteByDocId =
            DocumentsContract.buildDocumentUriUsingTree(rootTreeUri, nestedFileDocId!!)
        assertTrue(DocumentsContract.deleteDocument(context.contentResolver, deleteByDocId))
        assertTrue(
            "delete must remove only the nested file",
            !File(dirOnDisk(nestedDirName), "tree-uri-nested.txt").exists(),
        )
        assertTrue("nested dir must survive its child's deletion", dirOnDisk(nestedDirName).isDirectory)
        assertTrue(DocumentsContract.deleteDocument(context.contentResolver, nestedDirUri))
        assertTrue("cleanup must remove probe dir", !File(home, nestedDirName).exists())
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
