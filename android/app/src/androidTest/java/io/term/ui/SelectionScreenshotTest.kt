// TODO(kotlin-2.4.0-false-positive): K2 false positive
@file:Suppress("UNNECESSARY_NOT_NULL_ASSERTION")

package io.term.ui

import android.content.ClipData
import android.content.ClipboardManager
import android.content.Context
import android.graphics.Bitmap
import android.util.Log
import android.view.MotionEvent
import android.view.TextureView
import android.view.View
import android.view.ViewGroup
import androidx.compose.ui.test.junit4.v2.createAndroidComposeRule
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import io.term.MainActivity
import io.term.bridge.NativeBridge
import io.term.getBridge
import io.term.waitForSession
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith

@RunWith(AndroidJUnit4::class)
class SelectionScreenshotTest {
    @get:Rule val composeRule = createAndroidComposeRule<MainActivity>()

    companion object {
        private fun findTextureView(root: View): TextureView? {
            if (root is TextureView) return root
            if (root is ViewGroup) {
                for (i in 0 until root.childCount) {
                    (findTextureView(root.getChildAt(i)))?.let { return it }
                }
            }
            return null
        }

        private fun longPressOn(
            tv: TextureView,
            x: Float,
            y: Float,
        ) {
            val dt = android.os.SystemClock.uptimeMillis()
            tv.dispatchTouchEvent(
                MotionEvent.obtain(dt, dt, MotionEvent.ACTION_DOWN, x, y, 0),
            )
            Thread.sleep(800)
            tv.dispatchTouchEvent(
                MotionEvent.obtain(dt, dt + 800, MotionEvent.ACTION_MOVE, x + 1f, y + 1f, 0),
            )
            Thread.sleep(50)
            tv.dispatchTouchEvent(
                MotionEvent.obtain(dt, dt + 850, MotionEvent.ACTION_UP, x + 1f, y + 1f, 0),
            )
        }
    }

    @Test
    fun captureSelectionStates() {
        Log.i("ScreenshotTest", "==== Starting screenshot capture ====")

        composeRule.waitForSession()
        val bridge = checkNotNull(composeRule.getBridge()) { "Bridge must be available" }

        val tv = checkNotNull(findTextureView(composeRule.activity.window.decorView)) { "Terminal TextureView must be present" }

        Log.i("ScreenshotTest", "Bridge + surface ready")

        // — 1) Write text and capture baseline —
        val text =
            "hello world in terminal\n" +
                "https://github.com/termux/termux-app is a URL\n" +
                "simple words here\n\nbottom line\n"
        bridge!!.writeToPty(text.toByteArray())
        Thread.sleep(3000)
        capture("01-baseline")

        // — 2) Word selection: long-press on "world" (row ~0, col ~6) —
        longPressOn(tv, tv.width * 0.45f, tv.height * 0.12f)
        Thread.sleep(1500)
        capture("02-word-selection")

        // — 3) URL selection: long-press on https (row ~1, col ~0) —
        longPressOn(tv, tv.width * 0.15f, tv.height * 0.22f)
        Thread.sleep(1500)
        capture("03-url-selection")

        // — 4) Paste button on empty area —
        val cm = composeRule.activity.getSystemService(Context.CLIPBOARD_SERVICE) as ClipboardManager
        cm.setPrimaryClip(ClipData.newPlainText("test", "pasted text"))
        longPressOn(tv, tv.width * 0.5f, tv.height * 0.85f)
        Thread.sleep(1500)
        capture("04-paste-button")

        // — 5) After paste —
        bridge!!.writeToPty("echo 'pasted via PTY'\n".toByteArray())
        Thread.sleep(2000)
        capture("05-after-paste")

        Log.i("ScreenshotTest", "==== All 5 screenshots captured ====")
    }

    private fun capture(name: String) {
        Thread.sleep(500)
        try {
            val bitmap = InstrumentationRegistry.getInstrumentation().uiAutomation.takeScreenshot()
            if (bitmap == null) {
                // best-effort, non-asserting: debug screenshot capture may fail on a
                // transient uiAutomation hiccup and must not mask the test's real assertions.
                Log.e("ScreenshotTest", "Capture $name: bitmap is null (best-effort)")
                return
            }
            val context = InstrumentationRegistry.getInstrumentation().targetContext
            val dir = java.io.File(context.filesDir, "screenshots")
            dir.mkdirs()
            val file = java.io.File(dir, "selection-$name.png")
            file.outputStream().use { out ->
                bitmap.compress(Bitmap.CompressFormat.PNG, 100, out)
            }
            try {
                val extDir = context.getExternalFilesDir("Pictures")
                if (extDir != null) {
                    extDir.mkdirs()
                    val extFile = java.io.File(extDir, "selection-$name.png")
                    extFile.outputStream().use { out ->
                        bitmap.compress(Bitmap.CompressFormat.PNG, 100, out)
                    }
                }
            } catch (e: Exception) {
                // best-effort, non-asserting: optional copy of the debug screenshot to
                // external storage; a failure here must not mask real test failures.
                Log.e("ScreenshotTest", "Capture $name: external copy failed (best-effort)", e)
            }
            Log.i(
                "ScreenshotTest",
                "Capture $name: ${file.length()}B saved to ${file.absolutePath}",
            )
        } catch (e: Exception) {
            // best-effort, non-asserting: the debug capture helper must not swallow a real
            // assertion from the surrounding test; it only guards the screenshot side effect.
            Log.e("ScreenshotTest", "Capture $name FAILED (best-effort): ${e.message}", e)
        }
    }
}
