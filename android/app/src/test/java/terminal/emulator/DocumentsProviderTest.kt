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
    private val authority = "terminal.emulator.documents"

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

    private fun rootDir(): java.io.File = java.io.File(org.robolectric.RuntimeEnvironment.getApplication().filesDir, "home").apply {
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
    fun renameDocument_sanitizes_escape_names() {
        // ".." and "/" in the requested name are sanitized (never allowed
        // to escape the root), so the rename succeeds with a safe name.
        val rootDir = rootDir()
        val original = java.io.File(rootDir, "escape-me.txt")
        original.writeText("x")
        val docId = requireNotNull(TerminalDocumentsProvider.encodeDocId(original, rootDir))
        val newDocId = ensureProvider().renameDocument(docId, "../escaped.txt")
        assertTrue("original must be gone", !original.exists())
        val renamed = java.io.File(rootDir, newDocId)
        assertTrue("renamed file must exist inside root", renamed.exists())
        assertTrue("no path separators allowed", !newDocId.contains("/") && !newDocId.contains("\\"))
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
        } catch (expected: IllegalArgumentException) {
            // Mode strings outside the ParcelFileDescriptor.parseMode set
            // are a client contract violation — reject loudly.
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

    private fun createSymlink(
        linkName: String,
        target: java.io.File,
    ) {
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
    fun openDocument_rw_creates_missing_file() {
        val provider = ensureProvider()
        val target = java.io.File(rootDir(), "fresh.txt")
        assertTrue(!target.exists())
        provider.openDocument("fresh.txt", "rw", null).close()
        assertTrue(target.exists())
    }
}
