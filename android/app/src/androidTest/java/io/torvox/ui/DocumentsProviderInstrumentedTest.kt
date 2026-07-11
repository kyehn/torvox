package io.torvox.ui

import android.provider.DocumentsContract
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith

@RunWith(AndroidJUnit4::class)
class DocumentsProviderInstrumentedTest {
    private val authority = "io.torvox.documents"

    @Test
    fun queryRoots_returns_terminal_home() {
        val resolver = InstrumentationRegistry.getInstrumentation().targetContext.contentResolver
        val rootUri = DocumentsContract.buildRootsUri(authority)
        val cursor = resolver.query(rootUri, null, null, null, null)
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
        val resolver = InstrumentationRegistry.getInstrumentation().targetContext.contentResolver
        val rootUri = DocumentsContract.buildRootsUri(authority)
        val cursor = resolver.query(rootUri, null, null, null, null)
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
        val resolver = InstrumentationRegistry.getInstrumentation().targetContext.contentResolver
        val rootUri = DocumentsContract.buildRootsUri(authority)
        val rootsCursor = resolver.query(rootUri, null, null, null, null)
        assertNotNull(rootsCursor)
        rootsCursor!!.use {
            it.moveToFirst()
            val docIdIndex = it.getColumnIndex(DocumentsContract.Root.COLUMN_DOCUMENT_ID)
            val rootDocId = it.getString(docIdIndex)
            val docUri = DocumentsContract.buildDocumentUri(authority, rootDocId)
            val docCursor = resolver.query(docUri, null, null, null, null)
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
