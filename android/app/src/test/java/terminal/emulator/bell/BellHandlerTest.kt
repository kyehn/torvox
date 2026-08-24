package terminal.emulator.bell

import android.view.View
import androidx.test.core.app.ApplicationProvider
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertSame
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.Shadows.shadowOf
import org.robolectric.annotation.Config
import org.robolectric.annotation.LooperMode
import java.util.concurrent.TimeUnit

/**
 * Behavioural tests for [BellHandler] debounce, mode dispatch, and the
 * screen-flash restore. Timing is controlled via the injected [BellHandler.nowMs]
 * clock; the flash restore runs on the main looper and is advanced via
 * Robolectric shadow time.
 */
@RunWith(RobolectricTestRunner::class)
@Config(sdk = [34])
@LooperMode(LooperMode.Mode.PAUSED)
class BellHandlerTest {

    @Test
    fun debounce_drops_bells_within_window() {
        val context = ApplicationProvider.getApplicationContext<android.content.Context>()
        var now = 0L
        val handler = BellHandler(context, nowMs = { now })

        assertTrue(handler.fireBell())
        assertFalse("second bell inside the 150ms window must be dropped", handler.fireBell())
    }

    @Test
    fun debounce_allows_bell_after_window_elapses() {
        val context = ApplicationProvider.getApplicationContext<android.content.Context>()
        var now = 0L
        val handler = BellHandler(context, nowMs = { now })

        assertTrue(handler.fireBell())
        now += 149L
        assertFalse("still inside the window", handler.fireBell())
        now += 1L
        assertTrue("150ms elapsed — bell allowed again", handler.fireBell())
    }

    @Test
    fun silent_mode_accepts_bell_and_stays_silent() {
        val context = ApplicationProvider.getApplicationContext<android.content.Context>()
        val handler = BellHandler(context, nowMs = { 0L })
        handler.setMode(BellMode.SILENT)

        assertTrue(handler.fireBell())
    }

    @Test
    fun accessibility_callback_observes_accepted_bells_for_every_mode() {
        val context = ApplicationProvider.getApplicationContext<android.content.Context>()
        var now = 0L
        val handler = BellHandler(context, nowMs = { now })

        var calls = 0
        assertTrue(handler.fireBell(onAccessibility = { calls++ }))
        now += 150L
        handler.setMode(BellMode.SILENT)
        assertTrue(handler.fireBell(onAccessibility = { calls++ }))
        now += 150L
        assertTrue(handler.fireBell(onAccessibility = { calls++ }))
        assertEquals("one accessibility callback per accepted bell", 3, calls)
    }

    @Test
    fun screen_flash_without_view_falls_back_silently() {
        val context = ApplicationProvider.getApplicationContext<android.content.Context>()
        var now = 0L
        val handler = BellHandler(context, nowMs = { now })
        handler.setMode(BellMode.SCREEN_FLASH)

        // No target view — falls back to the tone path which is a no-op when
        // no audio HAL is available.
        assertTrue(handler.fireBell())
    }

    @Test
    fun screen_flash_with_view_turns_white_then_restores_background() {
        val context = ApplicationProvider.getApplicationContext<android.content.Context>()
        var now = 0L
        val handler = BellHandler(context, nowMs = { now })
        handler.setMode(BellMode.SCREEN_FLASH)

        // A non-ColorDrawable background survives the flash: Robolectric's
        // setBackgroundColor mutates an existing ColorDrawable in place (real
        // Android uses mutate()), so a ColorDrawable cannot distinguish the
        // captured original from the flashed one by colour.
        val view = View(context)
        val originalBg =
            android.graphics.drawable.BitmapDrawable(
                view.resources,
                android.graphics.Bitmap.createBitmap(1, 1, android.graphics.Bitmap.Config.ARGB_8888),
            )
        view.background = originalBg

        assertTrue(handler.fireBell(targetView = view))
        assertTrue(
            "flash replaces the background with a white ColorDrawable",
            view.background is android.graphics.drawable.ColorDrawable,
        )
        assertEquals(
            "flash paints the view white",
            android.graphics.Color.WHITE,
            (view.background as android.graphics.drawable.ColorDrawable).color,
        )

        shadowOf(android.os.Looper.getMainLooper()).idleFor(200L, TimeUnit.MILLISECONDS)
        assertSame(
            "flash restore returns the captured original drawable",
            originalBg,
            view.background,
        )
        handler.dispose()
    }

    @Test
    fun setMode_updates_currentMode_flow() {
        val context = ApplicationProvider.getApplicationContext<android.content.Context>()
        val handler = BellHandler(context, nowMs = { 0L })

        assertEquals(BellMode.SOUND, handler.currentMode.value)
        handler.setMode(BellMode.VIBRATE)
        assertEquals(BellMode.VIBRATE, handler.currentMode.value)
        handler.setMode(BellMode.SCREEN_FLASH)
        assertEquals(BellMode.SCREEN_FLASH, handler.currentMode.value)
    }
}
