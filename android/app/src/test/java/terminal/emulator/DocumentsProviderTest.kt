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
            // OnCloseListener 是 post 到主 Looper 的回调，Robolectric 默认暂停主 Looper，
            // 必须显式跑空队列它才会执行。
            org.robolectric.Shadows.shadowOf(android.os.Looper.getMainLooper()).idle()
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
    fun openDocument_rw_creates_missing_file() {
        val provider = ensureProvider()
        val target = java.io.File(rootDir(), "fresh.txt")
        assertTrue(!target.exists())
        provider.openDocument("fresh.txt", "rw", null).close()
        assertTrue(target.exists())
    }
}
