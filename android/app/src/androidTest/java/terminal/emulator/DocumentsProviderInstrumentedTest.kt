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
}
