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

    private fun rootDir(): java.io.File = java.io.File(org.robolectric.RuntimeEnvironment.getApplication().filesDir, "home").apply { mkdirs() }

    private fun ensureProvider(): TerminalDocumentsProvider {
        if (!::provider.isInitialized) {
            provider = org.robolectric.Robolectric.setupContentProvider(TerminalDocumentsProvider::class.java)
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
        assertEquals("docId must encode the new path", TerminalDocumentsProvider.encodeDocId(renamed, rootDir), newDocId)
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
        val home = java.io.File(provider.context!!.filesDir, "home").apply { mkdirs() }
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
        val home = java.io.File(provider.context!!.filesDir, "home").apply { mkdirs() }
        val target = java.io.File(home, "log.txt").apply { writeText("start|") }
        provider.openDocument("log.txt", "wa", null).use { fd ->
            java.io.FileOutputStream(fd.fileDescriptor).write("more".toByteArray())
        }
        assertEquals("start|more", target.readText())
    }

    @Test
    fun openDocument_unknown_mode_throws() {
        val home = java.io.File(provider.context!!.filesDir, "home").apply { mkdirs() }
        java.io.File(home, "f.txt").writeText("x")
        try {
            provider.openDocument("f.txt", "rwx", null)
            fail("unknown mode must be rejected")
        } catch (expected: IllegalArgumentException) {
            // Mode strings outside the ParcelFileDescriptor.parseMode set
            // are a client contract violation — reject loudly.
        }
    }
}
