package terminal.emulator

import android.provider.DocumentsContract
import androidx.test.platform.app.InstrumentationRegistry
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotNull
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

    /**
     * 外部文件管理器走**树 URI** 的修改/复制/移动。
     *
     * 这三个动作都在真机上被反馈为「无法修改/无法复制/无法移动」。它们与列举、
     * 新建的差别只在寻址形式：客户端持有的是 `content://…/tree/<root>/document/<docId>`
     * 形式的 URI，docId 含 '/' 且需经 URI 编解码往返；平台对本形式的每次访问都会先跑
     * `DocumentsProvider.enforceTree`（即 `isChildDocument`），再经 `call()` 路由到本提供者
     * 的 copyDocument / moveDocument。以下按客户端真实序列逐段断言落盘结果，
     * 任一环节不通都会在这里失败，而不是在用户的文件管理器里表现为无声无响应。
     */
    @Test
    fun tree_uri_modify_copy_move_round_trip() {
        val context = InstrumentationRegistry.getInstrumentation().targetContext
        val home = File(context.filesDir, "home").apply { mkdirs() }
        val targetDir = File(home, "tree-mutations-target").apply {
            deleteRecursively()
            mkdirs()
        }
        // 只清残留、不预建：`tree-mutations` 由本用例经提供者创建。预建会让提供者
        // 按与 Termux 一致的冲突命名（已存在则追加 " (2)"）返回 `tree-mutations (2)`，
        // 下面的 docId 断言随之对不上。这里它只是事后读盘的参照，无需先存在。
        val probeDir = File(home, "tree-mutations").apply { deleteRecursively() }
        try {
            val rootTreeUri = DocumentsContract.buildTreeDocumentUri(authority, "terminal_home")
            val rootDocumentUri = DocumentsContract.buildDocumentUri(authority, "terminal_home")

            // 客户端经树 URI 在子目录里建文件（与上面那个用例同一条路）。
            val childDirUri = DocumentsContract.createDocument(
                context.contentResolver,
                rootDocumentUri,
                DocumentsContract.Document.MIME_TYPE_DIR,
                "tree-mutations",
            )
            requireNotNull(childDirUri)
            val childDirDocId = DocumentsContract.getDocumentId(childDirUri)
            assertEquals("tree-mutations", childDirDocId)
            val childrenInDir = DocumentsContract.buildChildDocumentsUriUsingTree(rootTreeUri, childDirDocId)
            val sourceUri = DocumentsContract.createDocument(
                context.contentResolver,
                childrenInDir,
                "text/plain",
                "source.txt",
            )
            requireNotNull(sourceUri) { "tree URI create must return a uri" }
            context.contentResolver.openOutputStream(sourceUri, "rwt").use { stream ->
                requireNotNull(stream).write("original body".toByteArray())
            }
            assertEquals("original body", File(probeDir, "source.txt").readText())

            val sourceDocId = DocumentsContract.getDocumentId(sourceUri)
            assertEquals("tree-mutations/source.txt", sourceDocId)
            val sourceViaTree = DocumentsContract.buildDocumentUriUsingTree(rootTreeUri, sourceDocId)

            // ① 修改：以树 URI 打开已存在文件并覆写（"w" 截断语义）。
            context.contentResolver.openOutputStream(sourceViaTree, "w").use { stream ->
                requireNotNull(stream) { "existing file must be writable through a tree uri" }
                stream.write("edited body".toByteArray())
            }
            assertEquals("edited body", File(probeDir, "source.txt").readText())

            // ② 复制：树 URI 之间复制，源必须保留，副本必须落盘。
            val targetChildren =
                DocumentsContract.buildChildDocumentsUriUsingTree(
                    rootTreeUri,
                    "tree-mutations-target",
                )
            val copiedUri = DocumentsContract.copyDocument(context.contentResolver, sourceViaTree, targetChildren)
            requireNotNull(copiedUri) { "copyDocument must return the new document uri" }
            val copiedDocId = DocumentsContract.getDocumentId(copiedUri)
            assertEquals("tree-mutations-target/source.txt", copiedDocId)
            assertEquals("edited body", File(targetDir, "source.txt").readText())
            assertTrue("copy must leave the source in place", File(probeDir, "source.txt").exists())
            assertTrue(
                "copied document must be addressable through the tree",
                "text/plain" == context.contentResolver.getType(copiedUri),
            )

            // ③ 移动：把副本移回子目录，原名冲突时按唯一名落在磁盘上（不抛错）。
            val movedUri = DocumentsContract.moveDocument(
                context.contentResolver,
                copiedUri,
                targetChildren,
                childrenInDir,
            )
            requireNotNull(movedUri) { "moveDocument must return the new document uri" }
            val movedDocId = DocumentsContract.getDocumentId(movedUri)
            // 冲突命名按提供者的唯一名策略（`DocumentMutations.uniqueChild`）：
            // 序号插在扩展名前，冲突名是 `source (2).txt`。
            assertEquals("tree-mutations/source (2).txt", movedDocId)
            assertTrue(File(probeDir, "source (2).txt").exists())
            assertTrue(
                "move must remove the document from its old parent",
                !File(targetDir, "source.txt").exists(),
            )
            assertEquals("edited body", File(probeDir, "source (2).txt").readText())
        } finally {
            probeDir.deleteRecursively()
            targetDir.deleteRecursively()
        }
    }

    /**
     * 外部文件管理器走**树 URI** 的重命名。
     *
     * 真机反馈「重命名文件卡死」：对话框关掉、列表里名字不变，既无报错也无落盘。
     * 与复制/移动的差别只在入口：平台把 `renameDocument` 直接路由到提供者（不经
     * `call()`），客户端交出的是 `tree/<root>/document/<docId>` 形式的 URI。这里按
     * 客户端真实序列断言改名落盘、旧名消失、**返回的 URI 立刻可用** —— 返回 null
     * 会让客户端当「名字没变」处理并静默收场，正是无响应观感的来源。
     */
    @Test
    fun tree_uri_rename_round_trip() {
        val context = InstrumentationRegistry.getInstrumentation().targetContext
        val home = File(context.filesDir, "home").apply { mkdirs() }
        val probeDir = File(home, "tree-rename").apply { deleteRecursively() }
        try {
            val rootDocumentUri = DocumentsContract.buildDocumentUri(authority, "terminal_home")
            val rootTreeUri = DocumentsContract.buildTreeDocumentUri(authority, "terminal_home")

            val dirUri = DocumentsContract.createDocument(
                context.contentResolver,
                rootDocumentUri,
                DocumentsContract.Document.MIME_TYPE_DIR,
                "tree-rename",
            )
            requireNotNull(dirUri) { "probe dir create must return a uri" }
            val dirDocId = DocumentsContract.getDocumentId(dirUri)
            val childrenInDir = DocumentsContract.buildChildDocumentsUriUsingTree(rootTreeUri, dirDocId)

            val sourceUri = DocumentsContract.createDocument(
                context.contentResolver,
                childrenInDir,
                "text/plain",
                "before.txt",
            )
            requireNotNull(sourceUri) { "tree URI create must return a uri" }
            context.contentResolver.openOutputStream(sourceUri, "rwt").use { stream ->
                requireNotNull(stream).write("rename body".toByteArray())
            }
            val sourceDocId = DocumentsContract.getDocumentId(sourceUri)
            assertEquals("tree-rename/before.txt", sourceDocId)
            val sourceViaTree = DocumentsContract.buildDocumentUriUsingTree(rootTreeUri, sourceDocId)

            // ① 改名：必须返回新 URI（返回 null 等于「名字没变」，客户端静默收场）。
            val renamedUri = DocumentsContract.renameDocument(
                context.contentResolver,
                sourceViaTree,
                "after.txt",
            )
            requireNotNull(renamedUri) { "renameDocument must return the renamed document uri" }
            assertEquals("tree-rename/after.txt", DocumentsContract.getDocumentId(renamedUri))
            assertTrue("rename must land on disk", File(probeDir, "after.txt").exists())
            assertTrue("rename must remove the old name", !File(probeDir, "before.txt").exists())
            assertEquals("rename must preserve content", "rename body", File(probeDir, "after.txt").readText())

            // ② 新 docId 立刻可寻址：客户端改名后接着读/写，不能只认旧 id。
            assertEquals("text/plain", context.contentResolver.getType(renamedUri))
            context.contentResolver.openInputStream(renamedUri).use { stream ->
                val text = requireNotNull(stream) { "renamed document must be readable" }.readBytes().decodeToString()
                assertEquals("rename body", text)
            }

            // ③ 名字没变：提供者按框架合约回 null（原 docId 仍有效），平台据此把同一个
            // URI 交回客户端；磁盘不得因此多出副本，也不得改名。
            val unchangedUri =
                DocumentsContract.renameDocument(context.contentResolver, renamedUri, "after.txt")
            assertEquals(renamedUri, unchangedUri)
            assertTrue(File(probeDir, "after.txt").exists())
            assertEquals("rename body", File(probeDir, "after.txt").readText())
            assertEquals(listOf("after.txt"), probeDir.list()?.sorted())
        } finally {
            probeDir.deleteRecursively()
        }
    }

    /**
     * 终端侧改动必须通知打开着的列表（幽灵行的根治点）。
     *
     * 提供者此前只广播自己的变更操作，shell 里 `rm`/`mv`/解压后的列表全是旧快照，
     * 客户端拿着残留的幽灵行把创建/改名/粘贴拒成「路径已存在」。本用例走真实
     * inotify：查询目录登记监视 → 绕过提供者直接落盘（等价于终端里的 `touch`）→
     * 断言该目录的列表通知到达。
     */
    @Test
    fun terminal_side_change_notifies_open_listing() {
        val context = InstrumentationRegistry.getInstrumentation().targetContext
        val home = File(context.filesDir, "home").apply { mkdirs() }
        val watchDir = File(home, "watch-probe").apply {
            deleteRecursively()
            mkdirs()
        }
        val childrenUri = DocumentsContract.buildChildDocumentsUri(authority, "watch-probe")
        // 查询子项即登记监视：客户端看到哪一层，提供者就守哪一层。
        context.contentResolver.query(childrenUri, null, null, null, null)?.use {
            assertTrue("探测目录的子项查询必须成功", it.count == 0)
        }

        val notified = java.util.concurrent.atomic.AtomicBoolean(false)
        val observer =
            object : android.database.ContentObserver(android.os.Handler(android.os.Looper.getMainLooper())) {
                override fun onChange(selfChange: Boolean) {
                    notified.set(true)
                }
            }
        context.contentResolver.registerContentObserver(childrenUri, true, observer)
        try {
            File(watchDir, "terminal-side.txt").writeText("x")
            assertNotNull(
                "终端侧新建文件后，打开该目录的列表必须收到通知（5 秒内未收到）",
                UxTestUtils.pollUntilTrue(timeoutMs = 5_000, intervalMs = 50) { notified.get() },
            )
        } finally {
            context.contentResolver.unregisterContentObserver(observer)
            watchDir.deleteRecursively()
        }
    }

    /**
     * 终端侧删除：用户真机日志里唯一见过的提供者现场是 tree 前缀 docId 的
     * `deleteDocument`，spec 场景「终端侧删除后列表自动消失」此前只被
     * CREATE 用例覆盖。本用例用同一真实 inotify 通道复刻 DELETE：
     * 列表持有方注册观察者后，`rm` 文件必须在 5 秒内收到通知。
     */
    @Test
    fun terminal_side_delete_notifies_open_listing() {
        val context = InstrumentationRegistry.getInstrumentation().targetContext
        val home = File(context.filesDir, "home").apply { mkdirs() }
        val watchDir = File(home, "watch-delete-probe").apply {
            deleteRecursively()
            mkdirs()
        }
        File(watchDir, "to-remove.txt").writeText("x")
        val childrenUri = DocumentsContract.buildChildDocumentsUri(authority, "watch-delete-probe")
        context.contentResolver.query(childrenUri, null, null, null, null)?.use {
            assertTrue("探测目录的子项查询必须成功", it.count == 1)
        }
        val notified = java.util.concurrent.atomic.AtomicBoolean(false)
        val observer =
            object : android.database.ContentObserver(android.os.Handler(android.os.Looper.getMainLooper())) {
                override fun onChange(selfChange: Boolean) {
                    notified.set(true)
                }
            }
        context.contentResolver.registerContentObserver(childrenUri, true, observer)
        try {
            assertTrue("探针删除必须成功", File(watchDir, "to-remove.txt").delete())
            assertNotNull(
                "终端侧删除文件后，打开该目录的列表必须收到通知（5 秒内未收到）",
                UxTestUtils.pollUntilTrue(timeoutMs = 5_000, intervalMs = 50) { notified.get() },
            )
        } finally {
            context.contentResolver.unregisterContentObserver(observer)
            watchDir.deleteRecursively()
        }
    }

    /**
     * 外部文件管理器经树 URI 前缀寻址时，列表游标必须收到终端侧改动的通知。
     *
     * 用户报障（复制/移动提示「路径已存在」、创建文件失败）的直连回归：客户端把观察者
     * 注册在**列表游标**上（DocumentsUI 的 DirectoryLoader 正是
     * `cursor.registerContentObserver`），游标是否刷新完全取决于提供者在
     * `setNotificationUri` 与 `notifyChange` 两侧是否用同一个 URI。前缀形式寻址时
     * （`terminal_home/…`，真机 logcat 里 delete 即此形式）若不归一，两者落在兄弟
     * URI 上，平台按 path 段下降匹配永不命中，游标停在旧快照——客户端便拿幽灵行把
     * 创建/重命名/粘贴拒成「路径已存在」。
     */
    @Test
    fun cursor_from_tree_prefixed_listing_observes_terminal_side_changes() {
        val context = InstrumentationRegistry.getInstrumentation().targetContext
        val home = File(context.filesDir, "home").apply { mkdirs() }
        val probeDir = File(home, "prefixed-observe").apply {
            deleteRecursively()
            mkdirs()
        }
        try {
            val treeUri = DocumentsContract.buildTreeDocumentUri(authority, "terminal_home")
            val childrenViaTree =
                DocumentsContract.buildChildDocumentsUriUsingTree(treeUri, "terminal_home/prefixed-observe")
            val cursor = context.contentResolver.query(childrenViaTree, null, null, null, null)
            requireNotNull(cursor) { "前缀寻址的子项查询必须成功" }
            val notified = java.util.concurrent.atomic.AtomicBoolean(false)
            val observer =
                object : android.database.ContentObserver(android.os.Handler(android.os.Looper.getMainLooper())) {
                    override fun onChange(selfChange: Boolean) {
                        notified.set(true)
                    }
                }
            cursor.registerContentObserver(observer)
            try {
                // 终端侧改动：绕过提供者直接落盘，等价于 shell 里的 touch/解压。
                File(probeDir, "terminal-side.txt").writeText("x")
                assertNotNull(
                    "前缀寻址注册的列表游标必须收到终端侧改动（5 秒内未收到）",
                    UxTestUtils.pollUntilTrue(timeoutMs = 5_000, intervalMs = 50) { notified.get() },
                )
            } finally {
                cursor.unregisterContentObserver(observer)
                cursor.close()
            }
        } finally {
            probeDir.deleteRecursively()
        }
    }

    /**
     * 终端侧只改内容（`>>`、`sed -i`）必须让打开的客户端看到新的大小/时间。
     *
     * 内核为此只报 MODIFY，不改 inode 元数据；[DocumentWatcher] 的订阅集若漏了它，
     * 列表的元数据列就停在旧值。按现行契约（单测「modify notifies the document
     * without reloading the listing」），MODIFY 只广播**文档 URI**，不触发条目列表
     * 重载；客户端的列表要靠自身观察/重列来取新元数据。本用例钉住：① 文档 URI
     * 的广播及时到达；② 追加完成后重列直接给出新大小与时间。
     */
    @Test
    fun terminal_side_content_append_notifies_document_and_metadata_updates() {
        val context = InstrumentationRegistry.getInstrumentation().targetContext
        val home = File(context.filesDir, "home").apply { mkdirs() }
        val watchDir = File(home, "modify-probe").apply {
            deleteRecursively()
            mkdirs()
        }
        val target = File(watchDir, "appended.log").apply { writeText("first") }
        try {
            // 监视按「客户端查询过的目录」登记：先重列一次才启用监听。
            context.contentResolver.query(
                DocumentsContract.buildChildDocumentsUri(authority, "modify-probe"),
                null,
                null,
                null,
                null,
            )?.use { assertTrue("探测目录的子项查询必须成功", it.count == 1) }
            val documentUri = DocumentsContract.buildDocumentUri(authority, "modify-probe/appended.log")
            val notified = java.util.concurrent.atomic.AtomicBoolean(false)
            val observer =
                object : android.database.ContentObserver(android.os.Handler(android.os.Looper.getMainLooper())) {
                    override fun onChange(selfChange: Boolean) {
                        notified.set(true)
                    }
                }
            context.contentResolver.registerContentObserver(documentUri, true, observer)
            try {
                // 追加写：inode 不变，内核只报 MODIFY。
                target.appendText("second")
                assertNotNull(
                    "终端侧追加内容后，持有该文档的客户端必须收到元数据通知（5 秒内未收到）",
                    UxTestUtils.pollUntilTrue(timeoutMs = 5_000, intervalMs = 50) { notified.get() },
                )
                // 元数据随各次查询直读文件系统：追加后的重查必须已呈现新大小/时间。
                val expectedMtime = target.lastModified()
                context.contentResolver.query(documentUri, null, null, null, null)?.use {
                    assertTrue("追加后重查文档行必须命中", it.moveToFirst())
                    val sizeIndex = it.getColumnIndexOrThrow(DocumentsContract.Document.COLUMN_SIZE)
                    val mtimeIndex = it.getColumnIndexOrThrow(DocumentsContract.Document.COLUMN_LAST_MODIFIED)
                    assertEquals("追加后的大小必须反映新内容", target.length(), it.getLong(sizeIndex))
                    assertTrue(
                        "追加后的 mtime 必须落在追加窗口内",
                        it.getLong(mtimeIndex) >= expectedMtime - 100,
                    )
                }
            } finally {
                context.contentResolver.unregisterContentObserver(observer)
            }
        } finally {
            watchDir.deleteRecursively()
        }
    }

    /**
     * 编辑器回存周期：以 `wt`（截断写）模式打开并写回，随后立即重查行。
     *
     * 客户端报障「无法保存修改文件：超时」。真机日志（提供者 trace 或）未取到
     * 现场前，本用例在模拟器上钉住提供者侧周期：打开、写回、落盘、重查皆须在
     * 预算内完成且内容/时间戳一致。若真机复现再现，trace 行可直接对出哪一步
     * 吃掉了时间。
     */
    @Test
    fun external_save_cycle_writes_back_and_relists_in_budget() {
        val context = InstrumentationRegistry.getInstrumentation().targetContext
        val home = File(context.filesDir, "home").apply { mkdirs() }
        val docId = "save-cycle-probe.txt"
        val target = File(home, docId)
        try {
            val startedMs = System.currentTimeMillis()
            target.writeText("seed")
            val timeBudgetMs = 2_000L
            val payload = "saved-through-saf".toByteArray()
            // 编辑器「保存修改文件」的回存通道：wt = write + truncate + create。
            context.contentResolver.openOutputStream(
                DocumentsContract.buildDocumentUri(authority, docId),
                "wt",
            ).use { output ->
                requireNotNull(output).also { it.write(payload) }
            }
            val relisted = context.contentResolver.query(
                DocumentsContract.buildDocumentUri(authority, docId),
                arrayOf(
                    DocumentsContract.Document.COLUMN_DOCUMENT_ID,
                    DocumentsContract.Document.COLUMN_SIZE,
                    DocumentsContract.Document.COLUMN_LAST_MODIFIED,
                ),
                null,
                null,
                null,
            )
            requireNotNull(relisted).use {
                assertTrue("回存后重查必须命中最新的行", it.moveToFirst())
                assertEquals("回存后的大小必须反映写入内容", payload.size.toLong(), it.getLong(1))
                val mtime = it.getLong(2)
                val elapsed = System.currentTimeMillis() - startedMs
                // 毫秒精度文件系统与采样时钟有亚毫秒偏差，窗界留 100ms 余量。
                assertTrue(
                    "打开、写回、重查整周期 $elapsed ms 超出 $timeBudgetMs ms 预算",
                    elapsed <= timeBudgetMs,
                )
                assertTrue(
                    "mtime 必须落在保存窗口内（mtime=$mtime 窗口=$startedMs..${startedMs + elapsed}）",
                    mtime in (startedMs - 100)..(startedMs + elapsed + 100),
                )
            }
        } finally {
            target.delete()
        }
    }

    /**
     * 重命名后的列表即时性：改名返回新 docId 后，**下一次**父目录查询就必须
     * 只见到新名字（旧行已退场）。客户端报障「重命名文件：卡死」的界面再无
     * 反馈正发生在这一步。真机未见日志前，模拟器上钉住提供者侧的即时性。
     */
    @Test
    fun rename_reflected_in_next_listing_of_parent() {
        val context = InstrumentationRegistry.getInstrumentation().targetContext
        val home = File(context.filesDir, "home").apply { mkdirs() }
        val docId = "rename-probe-old.txt"
        val oldFile = File(home, docId)
        try {
            oldFile.writeText("rename me")
            val newDisplayName = "rename-probe-new.txt"
            val timeBudgetMs = 2_000L
            val startedAt = android.os.SystemClock.elapsedRealtime()
            val newUri = DocumentsContract.renameDocument(
                context.contentResolver,
                DocumentsContract.buildDocumentUri(authority, docId),
                newDisplayName,
            )
            requireNotNull(newUri)
            // 客户端改名后的第一步：以返回的新 URI 重查行、重列父目录。
            assertNotNull(
                "改名后第一时间重查必须命中（可直接发射失败提示）",
                UxTestUtils.pollUntilTrue(timeoutMs = timeBudgetMs, intervalMs = 20) {
                    context.contentResolver.query(
                        newUri,
                        arrayOf(DocumentsContract.Document.COLUMN_DOCUMENT_ID),
                        null,
                        null,
                        null,
                    )?.use { cursor ->
                        cursor.moveToFirst() && cursor.getString(0) == newDisplayName
                    } ?: false
                },
            )
            val childrenUri = DocumentsContract.buildChildDocumentsUri(authority, "terminal_home")
            assertNotNull(
                "下一次父目录列表必须只含新名字、无旧行",
                UxTestUtils.pollUntilTrue(timeoutMs = timeBudgetMs, intervalMs = 20) {
                    context.contentResolver.query(
                        childrenUri,
                        arrayOf(DocumentsContract.Document.COLUMN_DOCUMENT_ID),
                        null,
                        null,
                        null,
                    )?.use { cursor ->
                        val idIndex = cursor.getColumnIndexOrThrow(DocumentsContract.Document.COLUMN_DOCUMENT_ID)
                        var hasNew = false
                        var hasOld = false
                        while (cursor.moveToNext()) {
                            when (cursor.getString(idIndex)) {
                                newDisplayName -> hasNew = true
                                docId -> hasOld = true
                            }
                        }
                        hasNew && !hasOld
                    } ?: false
                },
            )
            val elapsed = android.os.SystemClock.elapsedRealtime() - startedAt
            assertTrue("重命名→重列整周期 $elapsed ms 超出 $timeBudgetMs ms 预算", elapsed <= timeBudgetMs)
        } finally {
            oldFile.delete()
            File(home, "rename-probe-new.txt").delete()
        }
    }

    /**
     * 重名新建走框架允许的消歧（同 Termux 策略）：已存在则生成唯一消歧名
     * （序号插扩展名前，`dup-create-probe (2).txt`）而非报「路径已存在」。
     * 直接覆盖客户端报障「无法创建文件/文件夹：路径已存在」这条外来现象的
     * 提供者侧契约：无论磁盘上已有同名文件/目录，新建必须成功落盘且新名字
     * 由返回 URI 下发。
     */
    @Test
    fun create_conflicting_name_lands_with_suffix() {
        val context = InstrumentationRegistry.getInstrumentation().targetContext
        val home = File(context.filesDir, "home").apply { mkdirs() }
        val parentUri = DocumentsContract.buildChildDocumentsUri(authority, "terminal_home")
        val existing = File(home, "dup-create-probe.txt").apply { writeText("already here") }
        try {
            val createdUri = DocumentsContract.createDocument(
                context.contentResolver,
                parentUri,
                "text/plain",
                "dup-create-probe.txt",
            )
            requireNotNull(createdUri)
            val docId = DocumentsContract.getDocumentId(createdUri)
            val suffixFile = File(home, "dup-create-probe (2).txt")
            try {
                assertEquals("重名新建必须追加消歧后缀", suffixFile.path, File(home, docId).path)
                suffixFile.writeText("with suffix")
            } finally {
                suffixFile.delete()
                existing.delete()
            }
        } finally {
            existing.delete()
        }
    }

    /** 重名新建目录走同名消歧：目录与文件用同一 uniqueChild 策略。 */
    @Test
    fun create_conflicting_directory_lands_with_suffix() {
        val context = InstrumentationRegistry.getInstrumentation().targetContext
        val home = File(context.filesDir, "home").apply { mkdirs() }
        val existing = File(home, "dup-dir-probe").apply { mkdirs() }
        val suffixDir = File(home, "dup-dir-probe (2)")
        try {
            val createdUri = DocumentsContract.createDocument(
                context.contentResolver,
                DocumentsContract.buildChildDocumentsUri(authority, "terminal_home"),
                DocumentsContract.Document.MIME_TYPE_DIR,
                "dup-dir-probe",
            )
            requireNotNull(createdUri)
            val docId = DocumentsContract.getDocumentId(createdUri)
            assertEquals("重名目录必须追加消歧后缀", suffixDir.path, File(home, docId).path)
            assertTrue(File(home, docId).isDirectory)
        } finally {
            suffixDir.deleteRecursively()
            existing.deleteRecursively()
        }
    }

    /**
     * 重名 rename 的消歧与 create/copy/move 同策略：改名为磁盘已有名字时
     * 生成唯一消歧名（序号插扩展名前），绝不静默失败——客户端依然拿到新
     * URI，重列即见新名字。
     */
    @Test
    fun rename_to_conflicting_name_lands_with_suffix() {
        val context = InstrumentationRegistry.getInstrumentation().targetContext
        val home = File(context.filesDir, "home").apply { mkdirs() }
        val original = File(home, "conflict-rename-old.txt").apply { writeText("x") }
        val occupant = File(home, "conflict-rename-new.txt").apply { writeText("occupied") }
        try {
            val newUri = DocumentsContract.renameDocument(
                context.contentResolver,
                DocumentsContract.buildDocumentUri(authority, "conflict-rename-old.txt"),
                "conflict-rename-new.txt",
            )
            requireNotNull(newUri)
            val docId = DocumentsContract.getDocumentId(newUri)
            assertEquals(
                "改名撞名必须追加消歧后缀",
                File(home, "conflict-rename-new (2).txt").path,
                File(home, docId).path,
            )
            assertTrue("改名后的内容必须原地保留", File(home, docId).readText() == "x")
        } finally {
            original.delete()
            occupant.delete()
            File(home, "conflict-rename-new (2).txt").delete()
        }
    }

    /**
     * 站外（终端侧）创建文件的 tree 前缀删除：用户日志里的唯一提供者现场
     * 是「tree 前缀 docId 的 deleteDocument」。本用例在模拟器真实客户端
     * （ContentResolver）上复刻同型删除：`tree/<root>/document/<docId>`
     * 经 `deleteDocument` 必须成功且磁盘条目同步退场。
     */
    @Test
    fun tree_uri_delete_removes_terminal_side_document() {
        val context = InstrumentationRegistry.getInstrumentation().targetContext
        val home = File(context.filesDir, "home").apply { mkdirs() }
        val probe = File(home, "tree-delete-probe.txt").apply { writeText("to be deleted") }
        val treeUri = DocumentsContract.buildDocumentUriUsingTree(
            DocumentsContract.buildTreeDocumentUri(authority, "terminal_home"),
            "tree-delete-probe.txt",
        )
        try {
            assertTrue(probe.exists())
            assertTrue(
                "tree 前缀删除必须成功",
                DocumentsContract.deleteDocument(context.contentResolver, treeUri),
            )
            assertTrue("删除后磁盘条目必须同步退场", !probe.exists())
        } finally {
            probe.delete()
        }
    }
}
