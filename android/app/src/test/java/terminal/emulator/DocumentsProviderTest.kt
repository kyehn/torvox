// ! Robolectric unit tests for TerminalDocumentsProvider.
// !
// ! The provider is pure ContentProvider logic (query roots/documents,
// ! flags, MIME types) with no rendering or PTY dependency, so the full
// ! instrumented suite in src/androidTest was migrated here — the same
// ! assertions run on the JVM in seconds instead of on an emulator.
// !
// ! # Requirements
// ! - FR-058 — DocumentsProvider exposes terminal home via SAF

package terminal.emulator

import android.provider.DocumentsContract
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertThrows
import org.junit.Assert.assertTrue
import org.junit.Assert.fail
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner

@RunWith(RobolectricTestRunner::class)
class DocumentsProviderTest {
    private val authority = "com.termux.documents"

    // Robolectric's ShadowContentResolver does not perform the Android-O+
    // ContentResolver → DocumentsProvider Bundle-extras conversion, so the
    // 5-arg query would hit the "Pre-Android-O query format" rejection.
    // Call the provider directly with the 6-arg form (same contract).
    private lateinit var provider: TerminalDocumentsProvider

    @org.junit.Before
    fun setUp() {
        // setupContentProvider attaches the provider, calls attachInfo +
        // onCreate, and returns the instance ready for direct queries.
        provider =
            org.robolectric.Robolectric.setupContentProvider(TerminalDocumentsProvider::class.java)
    }

    @Test
    fun queryRoots_returns_terminal_home() {
        val rootUri = DocumentsContract.buildRootsUri(authority)
        val cursor = provider.query(rootUri, null, android.os.Bundle(), null)
        assertNotNull("Roots cursor should not be null", cursor)
        requireNotNull(cursor).use {
            assertTrue("Should have at least one root", it.count >= 1)
            val idIndex = it.getColumnIndex(DocumentsContract.Root.COLUMN_ROOT_ID)
            val titleIndex = it.getColumnIndex(DocumentsContract.Root.COLUMN_TITLE)
            it.moveToFirst()
            assertEquals("terminal_home", it.getString(idIndex))
            assertEquals("Terminal Home", it.getString(titleIndex))
        }
    }

    @Test
    fun root_has_expected_flags() {
        val rootUri = DocumentsContract.buildRootsUri(authority)
        val cursor = provider.query(rootUri, null, android.os.Bundle(), null)
        assertNotNull(cursor)
        requireNotNull(cursor).use {
            it.moveToFirst()
            val flagsIndex = it.getColumnIndex(DocumentsContract.Root.COLUMN_FLAGS)
            val flags = it.getInt(flagsIndex)
            assertTrue(
                "Root should support create",
                flags and DocumentsContract.Root.FLAG_SUPPORTS_CREATE != 0,
            )
            assertTrue(
                "Root should support is_child",
                flags and DocumentsContract.Root.FLAG_SUPPORTS_IS_CHILD != 0,
            )
        }
    }

    @Test
    fun query_root_document_returns_directory() {
        val rootUri = DocumentsContract.buildRootsUri(authority)
        val rootsCursor = provider.query(rootUri, null, android.os.Bundle(), null)
        assertNotNull(rootsCursor)
        requireNotNull(rootsCursor).use {
            it.moveToFirst()
            val docIdIndex = it.getColumnIndex(DocumentsContract.Root.COLUMN_DOCUMENT_ID)
            val rootDocId = it.getString(docIdIndex)
            val docUri = DocumentsContract.buildDocumentUri(authority, rootDocId)
            val docCursor = provider.query(docUri, null, android.os.Bundle(), null)
            assertNotNull("Document cursor should not be null", docCursor)
            requireNotNull(docCursor).use { dc ->
                assertTrue("Root document should exist", dc.count == 1)
                val mimeIndex = dc.getColumnIndex(DocumentsContract.Document.COLUMN_MIME_TYPE)
                // The column must exist AND be a directory — a guard here
                // would silently lose the assertion when the provider
                // drops the column.
                assertTrue("MIME_TYPE column must exist", mimeIndex >= 0)
                dc.moveToFirst()
                assertEquals(DocumentsContract.Document.MIME_TYPE_DIR, dc.getString(mimeIndex))
            }
        }
    }

    private fun rootDir(): java.io.File = java.io.File(
        org.robolectric.RuntimeEnvironment.getApplication().filesDir,
        "home",
    ).apply {
        mkdirs()
    }

    private fun ensureProvider(): TerminalDocumentsProvider {
        if (!::provider.isInitialized) {
            provider =
                org.robolectric.Robolectric.setupContentProvider(TerminalDocumentsProvider::class.java)
        }
        return provider
    }

    @Test
    fun renameDocument_renames_file_inside_root() {
        val rootDir = rootDir()
        val original = java.io.File(rootDir, "rename-me.txt")
        original.writeText("content")
        val docId = requireNotNull(TerminalDocumentsProvider.encodeDocId(original, rootDir))

        val newDocId = ensureProvider().renameDocument(docId, "renamed.txt")
        assertTrue("original must be gone", !original.exists())
        val renamed = java.io.File(rootDir, "renamed.txt")
        assertTrue("renamed must exist", renamed.exists())
        assertEquals("content", renamed.readText())
        assertEquals(
            "docId must encode the new path",
            TerminalDocumentsProvider.encodeDocId(renamed, rootDir),
            newDocId,
        )
    }

    @Test
    fun renameDocument_rejects_escape_names() {
        // 路径分隔符与整体 "." / ".." 是客户端契约违反：大声失败而非静默改名。
        // 静默改名会让客户端拿到的 docId 与自己请求的名字对不上。
        val rootDir = rootDir()
        val original = java.io.File(rootDir, "escape-me.txt")
        original.writeText("x")
        val docId = requireNotNull(TerminalDocumentsProvider.encodeDocId(original, rootDir))
        for (illegal in listOf("../escaped.txt", "..\\escaped.txt", ".", "..", "   ")) {
            try {
                ensureProvider().renameDocument(docId, illegal)
                fail("rename to '$illegal' must be rejected")
            } catch (expected: java.io.FileNotFoundException) {
                // 跨 Binder 需为可处理失败，而非 IllegalArgumentException（会打崩调用方）。
            }
        }
        assertTrue("source must survive every rejected rename", original.exists())
    }

    @Test
    fun renameDocument_keeps_dots_inside_name() {
        // 名字内部的 ".." 是合法文件名，File(parent, name) 不会跳出父目录，
        // 必须原样保留而不是被替换成下划线。
        val rootDir = rootDir()
        val original = java.io.File(rootDir, "dots.txt")
        original.writeText("x")
        val docId = requireNotNull(TerminalDocumentsProvider.encodeDocId(original, rootDir))
        val newDocId = ensureProvider().renameDocument(docId, "a..b.txt")
        assertEquals("a..b.txt", newDocId)
        assertTrue(java.io.File(rootDir, "a..b.txt").exists())
    }

    @Test
    fun renameDocument_rejects_root() {
        try {
            ensureProvider().renameDocument("terminal_home", "x")
            throw AssertionError("root rename must fail")
        } catch (expected: java.io.FileNotFoundException) {
            // expected
        }
    }

    @Test
    fun createDocument_creates_file_and_deleteDocument_removes_it() {
        val provider = ensureProvider()
        val docId = provider.createDocument("terminal_home", "text/plain", "newfile.txt")
        val file = java.io.File(rootDir(), "newfile.txt")
        assertEquals("createDocument must create the file", true, file.exists())
        assertTrue("created document must be a file", file.isFile)
        // Query it back (round-trip).
        val cursor = provider.queryDocument(docId, null)
        assertTrue("created doc must be queryable", cursor.moveToFirst())
        cursor.close()
        // Delete it.
        provider.deleteDocument(docId)
        assertEquals("file must be gone after delete", false, file.exists())
    }

    /**
     * 空名新建：DocumentsUI 在名字为空时依然放行 SAVE，异常只进日志、
     * 界面毫无反馈（真机表现为「无法创建新文件」）。系统自带 DownloadsProvider
     * 对同一手势会建出占位文件，故此处必须成功创建而不是抛异常。
     */
    @Test
    fun createDocument_with_blank_name_creates_default_named_file() {
        val provider = ensureProvider()
        val docId = provider.createDocument("terminal_home", "text/plain", "   ")
        val created = java.io.File(rootDir(), "New Document.txt")
        assertTrue("blank name must fall back to the default file name", created.exists())
        assertEquals("returned docId must point at the created file", "New Document.txt", docId)
    }

    /** 空名 + 目录 mimeType：默认名不带扩展名，且落盘为目录。 */
    @Test
    fun createDocument_with_blank_name_creates_default_named_directory() {
        val provider = ensureProvider()
        provider.createDocument("terminal_home", DocumentsContract.Document.MIME_TYPE_DIR, "")
        val created = java.io.File(rootDir(), "New Folder")
        assertTrue("blank directory name must fall back to the default", created.exists())
        assertTrue("fallback directory must be a directory", created.isDirectory)
    }

    /** 重命名仍走严格校验：空名是客户端契约违反，不得被默认名掩盖。 */
    @Test
    fun renameDocument_with_blank_name_throws() {
        val provider = ensureProvider()
        val file = java.io.File(rootDir(), "renamed.txt").apply { writeText("x") }
        try {
            provider.renameDocument("renamed.txt", "  ")
            fail("blank rename name must be rejected")
        } catch (expected: java.io.FileNotFoundException) {
            assertTrue(file.exists())
        }
    }

    @Test
    fun openDocument_returns_parcel_file_for_existing_file() {
        val provider = ensureProvider()
        val file = java.io.File(rootDir(), "readme.txt").apply { writeText("hello") }
        val pfd = provider.openDocument("readme.txt", "r", null)
        pfd.use { pfd ->
            android.os.ParcelFileDescriptor.AutoCloseInputStream(pfd).use { input ->
                val text = input.readBytes().decodeToString()
                assertEquals("hello", text)
            }
        }
    }

    @Test
    fun deleteDocument_rejects_root() {
        val provider = ensureProvider()
        try {
            provider.deleteDocument("terminal_home")
            throw AssertionError("root delete must fail")
        } catch (expected: java.io.FileNotFoundException) {
            // expected — root delete would wipe the whole home.
        }
    }

    /**
     * 真机 logcat 实证：外部文件管理器经树 URI 寻址时传 `terminal_home/nix.conf`（带树根前缀），
     * 而行内只发行无前缀的相对路径。该前缀形此前落到 `home/terminal_home/…`（不存在）而失败。
     * 存在 `home/nix.conf` 时，前缀形删除必须删到它；返回后文件必须消失。
     */
    @Test
    fun deleteDocument_accepts_tree_root_prefixed_id() {
        val provider = ensureProvider()
        val target = java.io.File(rootDir(), "prefixed-victim.txt").apply { writeText("x") }
        provider.deleteDocument("terminal_home/prefixed-victim.txt")
        assertTrue("prefixed delete must remove home/prefixed-victim.txt", !target.exists())
    }

    /**
     * 精确路径优先：家目录里真有 `terminal_home` 子目录时，`terminal_home/victim.txt`
     * 首先指它自己，无前缀别名不得抢占行内地址。
     */
    @Test
    fun deleteDocument_prefixed_id_prefers_exact_path() {
        val provider = ensureProvider()
        val inner = java.io.File(rootDir(), "terminal_home").apply { mkdirs() }
        val exactVictim = java.io.File(inner, "victim.txt").apply { writeText("exact") }
        val outerVictim = java.io.File(rootDir(), "victim.txt").apply { writeText("outer") }
        provider.deleteDocument("terminal_home/victim.txt")
        assertTrue("exact path must be deleted", !exactVictim.exists())
        assertTrue("outer file must survive", outerVictim.exists())
    }

    /**
     * 同一别名覆盖改名：前缀形 rename 必须改到去前缀后的文件，且返回无前缀的新 docId
     *（调用方凭返回 id 自愈为行内形式）。
     */
    @Test
    fun renameDocument_accepts_tree_root_prefixed_id() {
        val provider = ensureProvider()
        java.io.File(rootDir(), "prefixed-source.txt").apply { writeText("content") }
        val newDocId = provider.renameDocument("terminal_home/prefixed-source.txt", "prefixed-dest.txt")
        assertEquals("prefixed-dest.txt", newDocId)
        assertTrue(java.io.File(rootDir(), "prefixed-dest.txt").exists())
    }

    @Test
    fun openDocument_mode_w_truncates_existing_content() {
        val home = java.io.File(requireNotNull(provider.context).filesDir, "home").apply { mkdirs() }
        val target = java.io.File(home, "notes.txt").apply { writeText("long original content") }
        // Document id is the path relative to the root (decodeDocId resolves
        // against rootDir), same id the SAF clients receive.
        val id = "notes.txt"

        provider.openDocument(id, "w", null).use { fd ->
            java.io.FileOutputStream(fd.fileDescriptor).write("hi".toByteArray())
        }
        assertEquals("hi", target.readText())
    }

    @Test
    fun openDocument_mode_wa_appends_instead_of_truncating() {
        val home = java.io.File(requireNotNull(provider.context).filesDir, "home").apply { mkdirs() }
        val target = java.io.File(home, "log.txt").apply { writeText("start|") }
        provider.openDocument("log.txt", "wa", null).use { fd ->
            java.io.FileOutputStream(fd.fileDescriptor).write("more".toByteArray())
        }
        assertEquals("start|more", target.readText())
    }

    /**
     * 平台创建惯例：客户端按 `FLAG_DIR_SUPPORTS_CREATE` 的承诺经 openDocument 落盘
     * （编辑器 SAVE、文件管理器覆盖写都走这条）。此前 openDocument 要求条目已存在，
     * 这类客户端于是拿到 FileNotFoundException——「无法修改文件」的直接成因。
     */
    @Test
    fun openDocument_mode_w_creates_missing_file() {
        val home = java.io.File(requireNotNull(provider.context).filesDir, "home").apply { mkdirs() }
        val target = java.io.File(home, "fresh.txt")
        // docId 取相对根路径，与 SAF 客户端从列举里拿到的一致。
        provider.openDocument("fresh.txt", "w", null).use { fd ->
            java.io.FileOutputStream(fd.fileDescriptor).write("saved".toByteArray())
        }
        assertTrue("openDocument(\"w\") must create the document it addresses", target.exists())
        assertEquals("saved", target.readText())
    }

    /**
     * 创建只由模式位决定：只读模式不含 CREATE，文件不存在时仍必须是可处理的失败。
     * 移除存在性检查后这条是唯一还拦着「打开不存在的文档」的东西，故必须钉住。
     */
    @Test
    fun openDocument_mode_r_still_rejects_missing_file() {
        val home = java.io.File(requireNotNull(provider.context).filesDir, "home").apply { mkdirs() }
        val thrown =
            assertThrows(java.io.FileNotFoundException::class.java) {
                provider.openDocument("absent.txt", "r", null)
            }
        assertTrue(
            "read-only open must report the missing document, got: ${thrown.message}",
            thrown.message?.contains("absent.txt") == true,
        )
    }

    /**
     * 移除存在性检查不得打开穿越口：docId 必须仍在根内解析，越界照样拒绝。
     * 位运算里 MODE_CREATE 能造文件，故这条守卫的优先级高于一切模式。
     */
    @Test
    fun openDocument_cannot_create_outside_root() {
        val home = java.io.File(requireNotNull(provider.context).filesDir, "home").apply { mkdirs() }
        val outside = java.io.File(requireNotNull(provider.context).filesDir, "escaped.txt")
        val thrown =
            assertThrows(java.io.FileNotFoundException::class.java) {
                // "../escaped.txt" 解析到 filesDir，越出 filesDir/home 根。
                provider.openDocument("../escaped.txt", "w", null)
            }
        assertTrue(
            "escape must be refused, got: ${thrown.message}",
            thrown.message?.contains("escaped.txt") == true,
        )
        assertTrue("no file may be created outside the root", !outside.exists())
    }

    /**
     * 钉住「createDocument 不拦点文件」这一事实：它正是 openDocument 原先那条
     * 「只打开已存在条目」的安全前提为何不成立的原因——客户端一次 createDocument
     * 就能造出 .mkshrc，那道检查拦得住的情形另一条路同样放行。
     *
     * 若哪天 createDocument 开始拒绝点文件，本用例失败，届时必须重新裁定
     * openDocument 要不要恢复存在性检查（安全前提才会真正成立）。
     */
    @Test
    fun createDocument_does_not_filter_dotfiles() {
        val home = java.io.File(requireNotNull(provider.context).filesDir, "home").apply { mkdirs() }
        val created = provider.createDocument("terminal_home", "text/plain", ".mkshrc")
        val expected = java.io.File(home, ".mkshrc")
        assertTrue("dotfile names must be creatable as documented", expected.exists())
        assertEquals(".mkshrc", created)
    }

    /**
     * 客户端取消后不得再开句柄：本重写不走基类实现，取消检查得自己做，
     * 否则选择器放弃缩略图/打开后我们仍在磁盘上做完 I/O。
     */
    @Test
    fun cancelled_signal_stops_openDocument_and_thumbnail() {
        val provider = ensureProvider()
        provider.createDocument("terminal_home", "text/plain", "cancelled.txt")
        val cancelled = android.os.CancellationSignal().apply { cancel() }
        for (label in listOf("openDocument", "openDocumentThumbnail")) {
            // 不用 runCatching：它连 CancellationException 一并吞掉，而这里的被测
            // 对象正是取消语义本身，任何其它异常必须原样冒泡而不是被改写成 null。
            val thrown =
                assertThrows(android.os.OperationCanceledException::class.java) {
                    when (label) {
                        "openDocument" -> provider.openDocument("cancelled.txt", "r", cancelled)

                        else ->
                            provider.openDocumentThumbnail(
                                "cancelled.txt",
                                android.graphics.Point(64, 64),
                                cancelled,
                            )
                    }
                }
            assertTrue("$label must report cancellation", thrown != null)
        }
    }

    /**
     * 跑空全部 Looper 队列直到 [expected] 里的通知全部到达。
     *
     * 写回通知跨两条队列：OnCloseListener 由写回线程 post（不占主 Looper），
     * `notifyChange` 再经主 Looper 派发给 observer。`getAllLoopers()` 的顺序不保证，
     * 主 Looper 一旦排在写回线程之前被 idle，写回线程随后投进主队列的通知就
     * 没人跑——单遍扫描只是对调度顺序的假设，整类并行跑时必现。
     * 每轮重取全部 Looper 再 idle，直到到齐；投递是同步的，故无需真实时间等待，
     * 上限只防「通知根本不发」时无限空转（那仍由下方断言大声失败）。
     */
    private fun idleLoopersUntilNotified(notified: Set<String>, expected: List<String>, maxRounds: Int = 8) {
        repeat(maxRounds) {
            if (expected.all { label -> notified.contains(label) }) return
            org.robolectric.shadows.ShadowLooper.getAllLoopers().forEach { looper ->
                org.robolectric.Shadows.shadowOf(looper).idle()
            }
        }
    }

    @Test
    fun openDocument_unknown_mode_throws() {
        val home = java.io.File(requireNotNull(provider.context).filesDir, "home").apply { mkdirs() }
        java.io.File(home, "f.txt").writeText("x")
        try {
            provider.openDocument("f.txt", "rwx", null)
            fail("unknown mode must be rejected")
        } catch (expected: java.io.FileNotFoundException) {
            // 跨 Binder 的契约违反必须是可处理失败：IllegalArgumentException
            // 在调用方进程里是未捕获异常，直接打崩 DocumentsUI / 编辑器。
        }
    }

    @Test
    fun openDocument_write_notifies_document_and_parent() {
        val provider = ensureProvider()
        provider.createDocument("terminal_home", "text/plain", "watched.txt")
        val resolver = requireNotNull(provider.context).contentResolver
        val documentUri = DocumentsContract.buildDocumentUri(authority, "watched.txt")
        val childrenUri = DocumentsContract.buildChildDocumentsUri(authority, "terminal_home")
        val notified = java.util.Collections.synchronizedSet(mutableSetOf<String>())
        fun observerFor(label: String) =
            object : android.database.ContentObserver(android.os.Handler(android.os.Looper.getMainLooper())) {
                override fun onChange(selfChange: Boolean) {
                    notified.add(label)
                }
            }
        val documentObserver = observerFor("document")
        val childrenObserver = observerFor("children")
        resolver.registerContentObserver(documentUri, true, documentObserver)
        resolver.registerContentObserver(childrenUri, true, childrenObserver)
        try {
            provider.openDocument("watched.txt", "rwt", null).use { parcelFileDescriptor ->
                java.io.FileOutputStream(parcelFileDescriptor.fileDescriptor).write("written".toByteArray())
            }
            // OnCloseListener 是 post 出去的回调（写回专用线程，不占主 Looper），
            // Robolectric 默认暂停 looper，必须显式跑空队列它才会执行；
            // 断言只认「回调跑过并发了通知」，不绑死具体线程。
            idleLoopersUntilNotified(notified, listOf("document", "children"))
            assertEquals("written", java.io.File(rootDir(), "watched.txt").readText())
            assertTrue(
                "external write-back must notify the document, got $notified",
                notified.contains("document"),
            )
            assertTrue(
                "external write-back must notify the parent listing, got $notified",
                notified.contains("children"),
            )
        } finally {
            resolver.unregisterContentObserver(documentObserver)
            resolver.unregisterContentObserver(childrenObserver)
        }
    }

    @Test
    fun root_advertises_search_and_available_bytes() {
        val rootUri = DocumentsContract.buildRootsUri(authority)
        val cursor = provider.query(rootUri, null, android.os.Bundle(), null)
        requireNotNull(cursor).use {
            it.moveToFirst()
            val flags = it.getInt(it.getColumnIndex(DocumentsContract.Root.COLUMN_FLAGS))
            assertTrue(
                "Root should support search",
                flags and DocumentsContract.Root.FLAG_SUPPORTS_SEARCH != 0,
            )
            val bytesIndex = it.getColumnIndex(DocumentsContract.Root.COLUMN_AVAILABLE_BYTES)
            assertTrue("AVAILABLE_BYTES column must exist", bytesIndex >= 0)
            assertTrue("Available bytes must be non-negative", it.getLong(bytesIndex) >= 0)
        }
    }

    @Test
    fun createDocument_conflict_appends_suffix_like_termux() {
        val provider = ensureProvider()
        val first = provider.createDocument("terminal_home", "text/plain", "dup.txt")
        val second = provider.createDocument("terminal_home", "text/plain", "dup.txt")
        assertTrue("conflicting create must not reuse the id", first != second)
        assertTrue("second file must exist", java.io.File(rootDir(), second).exists())
        assertTrue("first file must be untouched", java.io.File(rootDir(), first).exists())
    }

    /**
     * 目录同享冲突命名：客户端若已把目录建在磁盘上再让提供者建同名目录，
     * 拿回的 docId 带 " (2)" 后缀（与文件路径同一条策略）。
     *
     * 这条断言是 `DocumentsProviderInstrumentedTest#tree_uri_modify_copy_move_round_trip`
     * 踩过坑后的守栏：那个用例曾先 `mkdirs()` 预建目录，再经 `ContentResolver`
     * 让提供者创建同名目录，于是断言 `docId == "tree-mutations"` 拿到的是
     * `tree-mutations (2)`。设备预置状态与自建状态都必须走这条策略，调用方
     * 不得假定「我请求什么名字就拿回什么 docId」。
     */
    @Test
    fun createDocument_directory_conflict_appends_suffix_like_termux() {
        val provider = ensureProvider()
        val first =
            provider.createDocument(
                "terminal_home",
                DocumentsContract.Document.MIME_TYPE_DIR,
                "dupdir",
            )
        val second =
            provider.createDocument(
                "terminal_home",
                DocumentsContract.Document.MIME_TYPE_DIR,
                "dupdir",
            )
        assertEquals("dupdir", first)
        assertEquals("dupdir (2)", second)
        assertTrue("second directory must exist", java.io.File(rootDir(), second).isDirectory)
        assertTrue("first directory must be untouched", java.io.File(rootDir(), first).isDirectory)
    }

    @Test
    fun querySearchDocuments_finds_by_name_case_insensitive() {
        val provider = ensureProvider()
        java.io.File(rootDir(), "MeetingNotes.txt").writeText("x")
        java.io.File(rootDir(), "unrelated.log").writeText("y")
        val cursor = provider.querySearchDocuments("terminal_home", "MEETING", null)
        cursor.use {
            assertEquals("search must match exactly one file", 1, it.count)
            it.moveToFirst()
            val name = it.getString(it.getColumnIndex(DocumentsContract.Document.COLUMN_DISPLAY_NAME))
            assertEquals("MeetingNotes.txt", name)
        }
    }

    @Test
    fun querySearchDocuments_skips_symlink_outside_home() {
        val provider = ensureProvider()
        val outside =
            java.io.File(requireNotNull(provider.context).filesDir, "outside-secret.txt").apply { writeText("x") }
        assertTrue(outside.exists())
        try {
            java.lang.Runtime.getRuntime()
                .exec(
                    arrayOf(
                        "ln",
                        "-sf",
                        outside.absolutePath,
                        java.io.File(rootDir(), "leak-link").absolutePath,
                    ),
                )
                .waitFor()
        } catch (expected: Exception) {
            fail("symlink creation must work for this test: $expected")
        }
        val cursor = provider.querySearchDocuments("terminal_home", "outside-secret", null)
        cursor.use {
            assertEquals("outside-home symlink target must never surface", 0, it.count)
        }
    }

    @Test
    fun image_row_advertises_thumbnail_and_opens() {
        val provider = ensureProvider()
        java.io.File(rootDir(), "photo.png").writeBytes(byteArrayOf(1, 2, 3, 4))
        val cursor = provider.queryDocument("photo.png", null)
        cursor.use {
            assertTrue(it.moveToFirst())
            val flags = it.getInt(it.getColumnIndex(DocumentsContract.Document.COLUMN_FLAGS))
            assertTrue(
                "image must advertise thumbnail",
                flags and DocumentsContract.Document.FLAG_SUPPORTS_THUMBNAIL != 0,
            )
        }
        provider.openDocumentThumbnail("photo.png", null, null).use { asset ->
            assertEquals(4, asset.length)
        }
    }

    private fun createSymlink(linkName: String, target: java.io.File) {
        java.nio.file.Files.createSymbolicLink(
            java.io.File(rootDir(), linkName).toPath(),
            target.toPath(),
        )
    }

    /**
     * 列表查询注册的通知 URI 必须与终端侧广播同一命名空间。
     *
     * 外部文件管理器经树 URI 寻址时传的是 `terminal_home/sub` 这种带树根前缀的 docId
     * （真机 logcat 里 delete 正是这个形式），而行内发行与 inotify 播报都是无前缀的
     * `sub`。注册用前者、广播用后者时两者构成**兄弟** URI，平台按 path 段下降的通知
     * 匹配永远对不上——shell 里 `rm`/`mv` 后列表停在旧快照，客户端便拿幽灵行把
     * 创建/重命名/粘贴拒成「路径已存在」。
     */
    @Test
    fun queryChildDocuments_registers_notification_uri_without_tree_prefix() {
        val provider = ensureProvider()
        java.io.File(rootDir(), "prefixed").mkdirs()

        val cursor =
            provider.query(
                DocumentsContract.buildChildDocumentsUri(authority, "terminal_home/prefixed"),
                null,
                android.os.Bundle(),
                null,
            )
        requireNotNull(cursor).use {
            assertEquals(
                "通知 URI 必须归一为行内形式，否则终端侧广播送不到",
                DocumentsContract.buildChildDocumentsUri(authority, "prefixed"),
                it.notificationUri,
            )
        }
    }

    /** 带前缀父目录里的变更，广播必须落在无前缀的列表 URI 上（与上面的注册端对称）。 */
    @Test
    fun createDocument_under_prefixed_parent_notifies_unprefixed_listing() {
        val provider = ensureProvider()
        java.io.File(rootDir(), "prefixed-create").mkdirs()
        val childrenUri = DocumentsContract.buildChildDocumentsUri(authority, "prefixed-create")
        val notified = java.util.Collections.synchronizedSet(mutableSetOf<String>())
        val observer =
            object : android.database.ContentObserver(android.os.Handler(android.os.Looper.getMainLooper())) {
                override fun onChange(selfChange: Boolean) {
                    notified.add("children")
                }
            }
        val resolver = requireNotNull(provider.context).contentResolver
        resolver.registerContentObserver(childrenUri, true, observer)
        try {
            provider.createDocument("terminal_home/prefixed-create", "text/plain", "in.txt")
            idleLoopersUntilNotified(notified, listOf("children"))
            assertTrue(
                "带前缀父目录的创建必须通知无前缀列表，got $notified",
                notified.contains("children"),
            )
            assertTrue(
                "文件须落在去前缀后的真实目录里",
                java.io.File(rootDir(), "prefixed-create/in.txt").exists(),
            )
        } finally {
            resolver.unregisterContentObserver(observer)
        }
    }

    @Test
    fun queryDocument_symlink_returns_link_itself() {
        val provider = ensureProvider()
        val target = java.io.File(rootDir(), "target.txt").apply { writeText("data") }
        createSymlink("link.txt", target)
        // 浏览与回查必须给出同一 docId：链接自身，而非其目标。
        val childCursor = provider.queryChildDocuments("terminal_home", null, null as String?)
        val childIds = mutableListOf<String>()
        childCursor.use {
            val idIndex = it.getColumnIndex(DocumentsContract.Document.COLUMN_DOCUMENT_ID)
            while (it.moveToNext()) childIds.add(it.getString(idIndex))
        }
        assertTrue("listing must address the link entry", childIds.contains("link.txt"))
        val docCursor = provider.queryDocument("link.txt", null)
        docCursor.use {
            assertEquals("link must resolve to exactly one row", 1, it.count)
            assertTrue(it.moveToFirst())
            assertEquals(
                "link.txt",
                it.getString(it.getColumnIndex(DocumentsContract.Document.COLUMN_DOCUMENT_ID)),
            )
            assertEquals(
                "link.txt",
                it.getString(it.getColumnIndex(DocumentsContract.Document.COLUMN_DISPLAY_NAME)),
            )
        }
    }

    @Test
    fun queryDocument_symlink_to_dir_lists_as_directory() {
        val provider = ensureProvider()
        val subdir = java.io.File(rootDir(), "subdir").apply { mkdirs() }
        createSymlink("linkdir", subdir)
        val cursor = provider.queryDocument("linkdir", null)
        cursor.use {
            assertTrue(it.moveToFirst())
            assertEquals(
                DocumentsContract.Document.MIME_TYPE_DIR,
                it.getString(it.getColumnIndex(DocumentsContract.Document.COLUMN_MIME_TYPE)),
            )
        }
    }

    @Test
    fun queryDocument_dotdot_escape_rejected() {
        try {
            ensureProvider().queryDocument("../outside.txt", null)
            fail("path escape must be rejected")
        } catch (expected: java.io.FileNotFoundException) {
            // ".." escapes the root — refuse.
        }
    }

    @Test
    fun openDocument_symlink_opens_target_content() {
        // 文件管理器点开 symlink，应读到目标文件内容（open 跟随链接）。
        val provider = ensureProvider()
        val target = java.io.File(rootDir(), "real.txt").apply { writeText("target-content") }
        createSymlink("alias.txt", target)
        provider.openDocument("alias.txt", "r", null).use { parcelFileDescriptor ->
            android.os.ParcelFileDescriptor.AutoCloseInputStream(parcelFileDescriptor).use { input ->
                assertEquals("target-content", input.readBytes().decodeToString())
            }
        }
    }

    @Test
    fun openDocument_write_modes_preserve_rwx_attributes() {
        // rwx 文件经 w 截断 / wa 追加 / rw 打开后，权限位不得丢失。
        val provider = ensureProvider()
        val file = java.io.File(rootDir(), "run.sh").apply { writeText("echo old") }
        val fullAccess =
            setOf(
                java.nio.file.attribute.PosixFilePermission.OWNER_READ,
                java.nio.file.attribute.PosixFilePermission.OWNER_WRITE,
                java.nio.file.attribute.PosixFilePermission.OWNER_EXECUTE,
                java.nio.file.attribute.PosixFilePermission.GROUP_READ,
                java.nio.file.attribute.PosixFilePermission.GROUP_EXECUTE,
                java.nio.file.attribute.PosixFilePermission.OTHERS_READ,
                java.nio.file.attribute.PosixFilePermission.OTHERS_EXECUTE,
            )
        java.nio.file.Files.setPosixFilePermissions(file.toPath(), fullAccess)
        provider.openDocument("run.sh", "w", null).use { parcelFileDescriptor ->
            java.io.FileOutputStream(parcelFileDescriptor.fileDescriptor).write("echo new".toByteArray())
        }
        provider.openDocument("run.sh", "wa", null).use { parcelFileDescriptor ->
            java.io.FileOutputStream(parcelFileDescriptor.fileDescriptor).write("+more".toByteArray())
        }
        provider.openDocument("run.sh", "rw", null).use { parcelFileDescriptor ->
            android.os.ParcelFileDescriptor.AutoCloseInputStream(parcelFileDescriptor).close()
        }
        assertEquals("echo new+more", file.readText())
        assertEquals(
            "write cycles must not strip permission bits",
            fullAccess,
            java.nio.file.Files.getPosixFilePermissions(file.toPath()),
        )
    }

    @Test
    fun renameDocument_symlink_renames_link_only() {
        // 重命名链接只改链接名，目标文件名与内容保持不动。
        val provider = ensureProvider()
        val target = java.io.File(rootDir(), "real.txt").apply { writeText("target-content") }
        createSymlink("alias.txt", target)
        assertEquals("alias-renamed.txt", provider.renameDocument("alias.txt", "alias-renamed.txt"))
        assertTrue(
            java.nio.file.Files.isSymbolicLink(java.io.File(rootDir(), "alias-renamed.txt").toPath()),
        )
        assertEquals("target-content", target.readText())
        assertEquals("real.txt", target.name)
    }

    /**
     * 同名改名沿用 create/copy/move 的冲突策略，而不是硬失败：框架合约允许提供者
     * 为避开重名改写名字。「先新建、后改名」的客户端（文件管理器建文件/建文件夹、
     * 另存为）此前拿到 IOException，界面报「路径已存在」且操作全丢。
     */
    @Test
    fun renameDocument_conflict_appends_suffix_like_termux() {
        val provider = ensureProvider()
        java.io.File(rootDir(), "rename-src.txt").apply { writeText("payload") }
        java.io.File(rootDir(), "rename-dst.txt").apply { writeText("occupied") }
        val newDocId = provider.renameDocument("rename-src.txt", "rename-dst.txt")
        assertEquals("rename-dst.txt (2)", newDocId)
        assertEquals("payload", java.io.File(rootDir(), "rename-dst.txt (2)").readText())
        assertEquals("occupied", java.io.File(rootDir(), "rename-dst.txt").readText())
        assertTrue("source must be gone", !java.io.File(rootDir(), "rename-src.txt").exists())
    }

    @Test
    fun renameDocument_with_unchanged_name_is_a_no_op() {
        val provider = ensureProvider()
        val target = java.io.File(rootDir(), "same-name.txt").apply { writeText("keep") }
        // 框架约定原 docId 仍有效时返回 null；此处若不拦，源文件本身会被当成冲突而
        // 静默变成 `same-name.txt (2)`。
        assertEquals(null, provider.renameDocument("same-name.txt", "same-name.txt"))
        assertTrue("file must keep its name", target.exists())
        assertEquals("keep", target.readText())
    }

    /**
     * 真机 logcat 实证：删已不存在的条目（文件管理器缓存的幽灵行、覆盖写先删目标）
     * 曾抛 IOException，跨 Binder 变 RuntimeException 而让客户端报操作失败。
     * 删除的后置条件是「不再存在」——已满足即成功。
     */
    @Test
    fun deleteDocument_of_missing_document_is_a_no_op() {
        val provider = ensureProvider()
        val survivor = java.io.File(rootDir(), "survivor.txt").apply { writeText("keep") }
        provider.deleteDocument("vanished.txt")
        assertTrue("siblings must survive", survivor.exists())
        assertEquals("keep", survivor.readText())
    }

    @Test
    fun deleteDocument_of_dangling_symlink_removes_the_link() {
        val provider = ensureProvider()
        val link = java.io.File(rootDir(), "ghost-link.txt")
        java.nio.file.Files.createSymbolicLink(
            link.toPath(),
            java.io.File(rootDir(), "no-such-target.txt").toPath(),
        )
        provider.deleteDocument("ghost-link.txt")
        assertTrue(
            "dangling link must be unlinked",
            !java.nio.file.Files.exists(link.toPath(), java.nio.file.LinkOption.NOFOLLOW_LINKS),
        )
    }

    @Test
    fun file_row_advertises_rename_copy_move() {
        val provider = ensureProvider()
        java.io.File(rootDir(), "doc.txt").apply { writeText("x") }
        val cursor = provider.queryDocument("doc.txt", null)
        cursor.use {
            assertTrue(it.moveToFirst())
            val flags = it.getInt(it.getColumnIndex(DocumentsContract.Document.COLUMN_FLAGS))
            assertTrue(flags and DocumentsContract.Document.FLAG_SUPPORTS_RENAME != 0)
            assertTrue(flags and DocumentsContract.Document.FLAG_SUPPORTS_COPY != 0)
            assertTrue(flags and DocumentsContract.Document.FLAG_SUPPORTS_MOVE != 0)
        }
    }

    @Test
    fun copyDocument_duplicates_file_content() {
        val provider = ensureProvider()
        java.io.File(rootDir(), "src.txt").apply { writeText("payload") }
        java.io.File(rootDir(), "dest").apply { mkdirs() }
        val newId = provider.copyDocument("src.txt", "dest")
        val copy = java.io.File(rootDir(), "dest/src.txt")
        assertEquals("dest/src.txt", newId)
        assertEquals("payload", copy.readText())
        assertTrue("source must survive a copy", java.io.File(rootDir(), "src.txt").exists())
    }

    @Test
    fun moveDocument_relocates_and_clears_source() {
        val provider = ensureProvider()
        java.io.File(rootDir(), "moving.txt").apply { writeText("m") }
        java.io.File(rootDir(), "box").apply { mkdirs() }
        val newId = provider.moveDocument("moving.txt", "terminal_home", "box")
        assertEquals("box/moving.txt", newId)
        assertTrue(java.io.File(rootDir(), "box/moving.txt").exists())
        assertTrue(java.io.File(rootDir(), "moving.txt").exists().not())
    }

    @Test
    fun moveDocument_into_own_descendant_is_refused() {
        val provider = ensureProvider()
        java.io.File(rootDir(), "tree/sub").apply { mkdirs() }
        try {
            provider.moveDocument("tree", "terminal_home", "tree/sub")
            throw AssertionError("move into own descendant must fail")
        } catch (expected: java.io.IOException) {
            assertTrue(java.io.File(rootDir(), "tree/sub").isDirectory)
        }
    }

    /**
     * 回归钉：把符号链接搬进其**解析目标**的子目录必须放行，与 copyDocument 一致。
     *
     * `isHomeLink` 对家目录内的链接返回链接自身，故此处 `source` 是链接而不是
     * `mvtree`；`canonicalFile` 却把链接解析成目标，于是不带 `isSymbolicLink` 限定
     * 的守卫会把一次普通 rename 误判成「搬进自己的子孙」而拒绝。move 不遍历子树，
     * 根本不存在 a/b/a/b 打转风险——copyDocument 早已按此放行。
     */
    @Test
    fun moveDocument_symlink_into_its_own_target_subtree_is_allowed() {
        val provider = ensureProvider()
        val tree = java.io.File(rootDir(), "mvtree").apply { mkdirs() }
        java.io.File(tree, "sub").mkdirs()
        createSymlink("mvlink", tree)
        val newId = provider.moveDocument("mvlink", "terminal_home", "mvtree/sub")
        assertEquals("mvtree/sub/mvlink", newId)
        val moved = java.io.File(rootDir(), newId)
        assertTrue(
            "moved entry must stay a symlink, not an expanded directory",
            java.nio.file.Files.isSymbolicLink(moved.toPath()),
        )
        assertEquals(tree.canonicalPath, moved.canonicalPath)
    }

    /** 守卫对真目录仍然生效：链接放行不等于放弃自环防护。 */
    @Test
    fun moveDocument_real_directory_into_own_descendant_is_still_refused() {
        val provider = ensureProvider()
        java.io.File(rootDir(), "realtree/sub").apply { mkdirs() }
        val failure =
            try {
                provider.moveDocument("realtree", "terminal_home", "realtree/sub")
                null
            } catch (expected: java.io.IOException) {
                expected
            }
        assertNotNull("真目录搬进自身子孙必须仍然被拒", failure)
    }

    @Test
    fun copyDocument_into_own_descendant_is_refused() {
        val provider = ensureProvider()
        java.io.File(rootDir(), "ctree/sub").apply { mkdirs() }
        val failure =
            try {
                provider.copyDocument("ctree", "ctree/sub")
                null
            } catch (expected: java.io.IOException) {
                expected
            }
        assertNotNull("copy into own descendant must fail", failure)
        // 无守卫时会先递归复制 a/b/a/b/… 直到 ENAMETOOLONG 再整体回滚，
        // 终态看似干净但耗尽路径长度与磁盘；守卫在动手前就拒绝。
        assertEquals(
            "must be refused before any copy runs, not after a runaway traversal",
            "Refusing to copy a directory into its own descendant",
            failure!!.message,
        )
        assertEquals(
            "no runaway copy may be left behind",
            emptyList<String>(),
            java.io.File(rootDir(), "ctree/sub").list()?.toList() ?: emptyList<String>(),
        )
    }

    /**
     * 符号链接复制进**其解析目标**的子目录必须放行：copyTreeInto 遇链接只复制
     * inode 自身、不展开目标树，故不存在 a/b/a/b/… 打转；按 canonical 路径比对
     * 反而把这条正常操作误判成「复制进自己的子孙」。
     */
    @Test
    fun copyDocument_symlink_into_its_own_target_subtree_is_allowed() {
        val provider = ensureProvider()
        val tree = java.io.File(rootDir(), "symtree").apply { mkdirs() }
        java.io.File(tree, "sub").mkdirs()
        createSymlink("symlink", tree)
        val newId = provider.copyDocument("symlink", "symtree/sub")
        assertEquals("symtree/sub/symlink", newId)
        val copied = java.io.File(rootDir(), newId)
        assertTrue(
            "copied entry must stay a symlink, not an expanded directory tree",
            java.nio.file.Files.isSymbolicLink(copied.toPath()),
        )
        assertEquals(tree.canonicalPath, copied.canonicalPath)
    }

    @Test
    fun querySearchDocuments_finds_matching_directories() {
        val provider = ensureProvider()
        java.io.File(rootDir(), "MyProjects").apply { mkdirs() }
        java.io.File(rootDir(), "unrelated.txt").apply { writeText("y") }
        val cursor = provider.querySearchDocuments("terminal_home", "project", null)
        cursor.use {
            assertEquals("search must match the directory", 1, it.count)
            it.moveToFirst()
            val name = it.getString(it.getColumnIndex(DocumentsContract.Document.COLUMN_DISPLAY_NAME))
            assertEquals("MyProjects", name)
            val mime = it.getString(it.getColumnIndex(DocumentsContract.Document.COLUMN_MIME_TYPE))
            assertEquals(DocumentsContract.Document.MIME_TYPE_DIR, mime)
        }
    }

    @Test
    fun isChildDocument_returns_false_for_escape_instead_of_throwing() {
        val provider = ensureProvider()
        assertEquals(false, provider.isChildDocument("terminal_home", "../outside.txt"))
        assertEquals(false, provider.isChildDocument("../outside", "sub.txt"))
    }

    @Test
    fun renameDocument_rejects_empty_id_as_root() {
        try {
            ensureProvider().renameDocument("", "x")
            fail("empty id decodes to root and must be rejected")
        } catch (expected: java.io.FileNotFoundException) {
        }
    }

    @Test
    fun copyDocument_rejects_root_aliases() {
        val provider = ensureProvider()
        java.io.File(rootDir(), "box").apply { mkdirs() }
        try {
            provider.copyDocument("", "box")
            fail("empty id must be rejected as root")
        } catch (expected: java.io.FileNotFoundException) {
        }
    }

    @Test
    fun openDocument_fallback_mode_rwa_writes() {
        val provider = ensureProvider()
        java.io.File(rootDir(), "fallback.txt").apply { writeText("a") }
        provider.openDocument("fallback.txt", "rwa", null).use { parcelFileDescriptor ->
            java.io.FileOutputStream(parcelFileDescriptor.fileDescriptor).write("b".toByteArray())
        }
        assertTrue(java.io.File(rootDir(), "fallback.txt").readText().contains("b"))
    }

    @Test
    fun queryDocument_subset_projection_does_not_crash() {
        val provider = ensureProvider()
        java.io.File(rootDir(), "subset.txt").apply { writeText("x") }
        val cursor = provider.queryDocument(
            "subset.txt",
            arrayOf(DocumentsContract.Document.COLUMN_DOCUMENT_ID),
        )
        cursor.use {
            assertTrue(it.moveToFirst())
            assertEquals(
                "subset.txt",
                it.getString(it.getColumnIndexOrThrow(DocumentsContract.Document.COLUMN_DOCUMENT_ID)),
            )
        }
    }

    @Test
    fun openDocument_creates_a_missing_document_for_create_capable_modes() {
        // 「rw」含 MODE_CREATE，而目录已声明 FLAG_DIR_SUPPORTS_CREATE：平台据此允许
        // 客户端经 openDocument 落盘。原先本条断的是「绝不创建」，理由是防止客户端
        // 凭空造出 .mkshrc——该前提不成立（createDocument 并不拦点文件，见
        // createDocument_does_not_filter_dotfiles），只把按平台惯例保存的编辑器与
        // 文件管理器挡在门外，正是「无法修改文件」的成因。
        val provider = ensureProvider()
        val target = java.io.File(rootDir(), "fresh.txt")
        assertTrue(!target.exists())
        provider.openDocument("fresh.txt", "rw", null).use { fd ->
            java.io.FileOutputStream(fd.fileDescriptor).write("rw".toByteArray())
        }
        assertTrue("openDocument(\"rw\") must create the document it addresses", target.exists())
        assertEquals("rw", target.readText())
    }

    /**
     * 复制**目录树**：子目录、孙目录、文件与符号链接都要各就各位。
     * 文件管理器的「复制」对文件夹同样生效，故这条不是边角料。
     */
    @Test
    fun copyDocument_duplicates_a_directory_tree() {
        val provider = ensureProvider()
        val source = java.io.File(rootDir(), "proj").apply { mkdirs() }
        java.io.File(source, "src").mkdirs()
        java.io.File(source, "build.log").writeText("built")
        java.io.File(java.io.File(source, "src"), "main.rs").writeText("fn main() {}")
        val nestedOutside = java.io.File(rootDir(), "outside.txt").apply { writeText("outside") }
        java.nio.file.Files.createSymbolicLink(
            java.io.File(source, "alias.txt").toPath(),
            nestedOutside.toPath(),
        )
        val targetParent = java.io.File(rootDir(), "backup").apply { mkdirs() }

        val newId = provider.copyDocument("proj", "backup")

        assertEquals("backup/proj", newId)
        val copy = java.io.File(rootDir(), "backup/proj")
        assertEquals("built", java.io.File(copy, "build.log").readText())
        assertEquals("fn main() {}", java.io.File(copy, "src/main.rs").readText())
        assertTrue("孙目录必须一并复制", java.io.File(copy, "src").isDirectory)
        assertTrue(
            "链接只复制 inode 自身，不展开成目标目录树",
            java.nio.file.Files.isSymbolicLink(java.io.File(copy, "alias.txt").toPath()),
        )
        assertTrue("源目录树必须原样保留", java.io.File(source, "src/main.rs").exists())
    }

    /** 移动**目录树**：整棵树改换父目录，源处不得有残留。 */
    @Test
    fun moveDocument_relocates_a_directory_tree() {
        val provider = ensureProvider()
        val tree = java.io.File(rootDir(), "workspace").apply { mkdirs() }
        java.io.File(tree, "deep").mkdirs()
        java.io.File(tree, "deep/leaf.txt").writeText("leaf")
        java.io.File(rootDir(), "archive").apply { mkdirs() }

        val newId = provider.moveDocument("workspace", "terminal_home", "archive")

        assertEquals("archive/workspace", newId)
        assertEquals("leaf", java.io.File(rootDir(), "archive/workspace/deep/leaf.txt").readText())
        assertTrue("源目录树必须整体搬走", !java.io.File(rootDir(), "workspace").exists())
    }

    /** 复制到已占用同名时取唯一名，且返回的 docId 与实际落盘一致。 */
    @Test
    fun copyDocument_conflict_gets_unique_name_matching_returned_id() {
        val provider = ensureProvider()
        val root = rootDir()
        java.io.File(root, "dup.txt").writeText("first")
        java.io.File(root, "dest").mkdirs()
        java.io.File(root, "dest/dup.txt").writeText("already here")

        val newId = provider.copyDocument("dup.txt", "dest")

        assertTrue(
            "返回的 docId 必须指向真实落盘文件，否则客户端回查询不到",
            newId == "dest/dup.txt (2)" && java.io.File(root, newId).isFile,
        )
        assertEquals("first", java.io.File(root, newId).readText())
        assertEquals("原有文件不得被覆盖", "already here", java.io.File(root, "dest/dup.txt").readText())
    }

    /**
     * 复制指向**家目录内**的符号链接：只复制链接 inode，不展开成目标目录树。
     *
     * 站外链接（指向 home 之外）本就不在列举结果里——`encodeDocId` 对站外目标返回
     * null，客户端无从从浏览得到它的 docId；寻址到它时拒绝是正确的收缩，不是缺口。
     */
    @Test
    fun copyDocument_of_symlink_inside_home_keeps_it_a_link() {
        val provider = ensureProvider()
        val root = rootDir()
        val target = java.io.File(root, "real.txt").apply { writeText("secret") }
        createSymlink("portal", target)
        java.io.File(root, "landing").mkdirs()

        val newId = provider.copyDocument("portal", "landing")

        assertEquals("landing/portal", newId)
        val copied = java.io.File(root, newId)
        assertTrue(
            "复制出的条目必须仍是符号链接，而不是展开成目标内容",
            java.nio.file.Files.isSymbolicLink(copied.toPath()),
        )
        assertEquals(
            "链接仍指向原目标",
            target.canonicalPath,
            copied.canonicalPath,
        )
        assertEquals("secret", copied.readText())
        assertTrue("目标本身不得被搬走或改名", target.exists())
    }

    /** 站外链接不可寻址：复制它必须拒绝，而不是把目标整棵树吸入家目录。 */
    @Test
    fun copyDocument_refuses_symlink_pointing_outside_home() {
        val provider = ensureProvider()
        val root = rootDir()
        val outside = java.io.File(requireNotNull(provider.context).filesDir, "outside-secret.txt")
            .apply { writeText("secret") }
        createSymlink("portal", outside)
        java.io.File(root, "landing").mkdirs()

        try {
            provider.copyDocument("portal", "landing")
            fail("复制站外链接必须被拒绝")
        } catch (expected: java.io.FileNotFoundException) {
            // 站外目标不得进入家目录——这正是 encodeDocId 对站外链接返回 null 的原因。
        }
        assertTrue("站外目标不得被搬走", outside.exists())
        assertTrue(
            "家目录内不得留下任何复制产物",
            java.io.File(root, "landing").list().isNullOrEmpty(),
        )
    }
}
