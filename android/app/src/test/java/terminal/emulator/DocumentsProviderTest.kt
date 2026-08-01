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
        cursor!!.use {
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
        cursor!!.use {
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
        rootsCursor!!.use {
            it.moveToFirst()
            val docIdIndex = it.getColumnIndex(DocumentsContract.Root.COLUMN_DOCUMENT_ID)
            val rootDocId = it.getString(docIdIndex)
            val docUri = DocumentsContract.buildDocumentUri(authority, rootDocId)
            val docCursor = provider.query(docUri, null, android.os.Bundle(), null)
            assertNotNull("Document cursor should not be null", docCursor)
            docCursor!!.use { dc ->
                assertTrue("Root document should exist", dc.count == 1)
                val mimeIndex = dc.getColumnIndex(DocumentsContract.Document.COLUMN_MIME_TYPE)
                if (mimeIndex >= 0) {
                    dc.moveToFirst()
                    assertEquals(DocumentsContract.Document.MIME_TYPE_DIR, dc.getString(mimeIndex))
                }
            }
        }
    }
}
