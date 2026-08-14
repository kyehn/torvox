package terminal.emulator

import android.net.Uri
import android.os.Build
import androidx.test.core.app.ApplicationProvider
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.Shadows.shadowOf
import org.robolectric.annotation.Config
import java.io.ByteArrayInputStream
import java.io.File
import java.io.IOException

/**
 * ghostty-android BackgroundImageStore adoption:
 * - picked content:// URIs are copied into app-private storage so the
 *   persisted path never depends on a revocable SAF grant
 * - a missing/read-failing URI does not leave a partial copy behind
 */
@RunWith(RobolectricTestRunner::class)
@Config(sdk = [Build.VERSION_CODES.TIRAMISU])
class BackgroundImagePrivateStoreTest {

    private fun filesDir(): File = ApplicationProvider.getApplicationContext<android.content.Context>().filesDir

    @Test
    fun `content uri copy writes bytes to private file`() {
        val ctx = ApplicationProvider.getApplicationContext<android.content.Context>()
        val resolver = ctx.contentResolver
        val pickedBytes = byteArrayOf(1, 2, 3, 4, 5)
        val uri = Uri.parse("content://com.example.picker/picked_image.png")
        shadowOf(resolver).registerInputStream(uri, ByteArrayInputStream(pickedBytes))

        val dst = File(filesDir(), "terminal_background")
        dst.delete()
        val copied = copyContentUriToPrivateFile(resolver, uri, dst)

        assertTrue("copy must succeed", copied)
        assertTrue("private copy must exist", dst.exists())
        assertEquals(
            "bytes must round-trip",
            pickedBytes.toList(),
            dst.readBytes().toList(),
        )
    }

    @Test
    fun `unreadable uri returns false and writes nothing`() {
        val ctx = ApplicationProvider.getApplicationContext<android.content.Context>()
        val dst = File(ctx.filesDir, "terminal_background")
        dst.delete()
        val missing = Uri.parse("content://com.example.nonexistent/missing")

        val copied = copyContentUriToPrivateFile(ctx.contentResolver, missing, dst)

        assertFalse("unreadable URI must not report success", copied)
        assertFalse("no partial file may be left behind", dst.exists())
    }

    @Test
    fun `stream failure mid-copy deletes the partial file`() {
        val ctx = ApplicationProvider.getApplicationContext<android.content.Context>()
        val resolver = ctx.contentResolver
        val uri = Uri.parse("content://com.example.picker/vanishing.png")
        // Yields a few bytes, then the media source disappears: the copy
        // must not leave the already-written partial file behind.
        val vanishing = object : java.io.InputStream() {
            private var remaining = 3
            override fun read(): Int {
                if (remaining > 0) {
                    remaining--
                    return 7
                }
                throw IOException("media vanished mid-copy")
            }
        }
        shadowOf(resolver).registerInputStream(uri, vanishing)

        val dst = File(ctx.filesDir, "terminal_background")
        dst.delete()
        val copied = copyContentUriToPrivateFile(resolver, uri, dst)

        assertFalse("failed copy must not report success", copied)
        assertFalse("partial file must be removed", dst.exists())
    }
}
