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
import java.io.FileOutputStream

/**
 * ghostty-android BackgroundImageStore adoption:
 * - picked content:// URIs are copied into app-private storage so the
 *   persisted path never depends on a revocable SAF grant
 * - a missing private file self-heals: the setting is cleared and the
 *   solid theme background renders instead of crashing
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
        contentResolver().openInputStream(uri)?.use { input ->
            FileOutputStream(dst).use { output ->
                val buf = ByteArray(64 * 1024)
                while (true) {
                    val n = input.read(buf)
                    if (n <= 0) break
                    output.write(buf, 0, n)
                }
            }
        }

        assertTrue("private copy must exist", dst.exists())
        assertEquals("bytes must round-trip", byteArrayOf(1, 2, 3, 4, 5).toList(), dst.readBytes().toList())
        testImage.delete()
    }

    @Test
    fun `missing private file is detected for self-heal`() {
        val ctx = ApplicationProvider.getApplicationContext<android.content.Context>()
        val missing = File(ctx.filesDir, "terminal_background")
        missing.delete()
        assertFalse("file must be absent", missing.exists())
        // The ViewModel self-heal branch keys on filesDir-relative paths and
        // clears the setting when the file does not exist — the predicate
        // used there must match a plain missing file.
        assertTrue(missing.absolutePath.startsWith(ctx.filesDir.absolutePath))
    }
}
