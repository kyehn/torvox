package terminal.emulator

import android.content.ContentResolver
import android.net.Uri
import android.os.Build
import androidx.test.core.app.ApplicationProvider
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config
import java.io.File

/**
 * ghostty-android BackgroundImageStore adoption:
 * - picked content:// URIs are copied into app-private storage so the
 *   persisted path never depends on a revocable SAF grant
 * - a missing/read-failing URI does not leave a partial copy behind
 */
@RunWith(RobolectricTestRunner::class)
@Config(sdk = [Build.VERSION_CODES.TIRAMISU])
class BackgroundImagePrivateStoreTest {

    private fun contentResolver(): ContentResolver = ApplicationProvider.getApplicationContext<android.content.Context>().contentResolver

    @Test
    fun `content uri copy writes bytes to private file`() {
        val ctx = ApplicationProvider.getApplicationContext<android.content.Context>()
        val filesDir = ctx.filesDir
        val testImage = File(filesDir, "picked_image.png")
        testImage.writeBytes(byteArrayOf(1, 2, 3, 4, 5))
        val uri = Uri.fromFile(testImage)

        val dst = File(filesDir, "terminal_background")
        dst.delete()
        val copied = copyContentUriToPrivateFile(contentResolver(), uri, dst)

        assertTrue("copy must succeed", copied)
        assertTrue("private copy must exist", dst.exists())
        assertEquals(
            "bytes must round-trip",
            byteArrayOf(1, 2, 3, 4, 5).toList(),
            dst.readBytes().toList(),
        )
        testImage.delete()
    }

    @Test
    fun `unreadable uri returns false and writes nothing`() {
        val ctx = ApplicationProvider.getApplicationContext<android.content.Context>()
        val dst = File(ctx.filesDir, "terminal_background")
        dst.delete()
        val missing = Uri.parse("content://com.example.nonexistent/missing")

        val copied = copyContentUriToPrivateFile(contentResolver(), missing, dst)

        assertFalse("unreadable URI must not report success", copied)
        assertFalse("no partial file may be left behind", dst.exists())
    }
}
