package terminal.emulator.bell

import android.content.Context
import android.media.AudioManager
import android.media.ToneGenerator
import android.os.VibrationEffect
import android.os.Vibrator
import android.os.VibratorManager
import android.view.View
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancel
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.launch

class BellHandler(private val context: Context) {
    companion object {
        private const val DEBOUNCE_MS = 150L
        private const val VIBRATE_DURATION_MS = 50L
        private const val FLASH_DURATION_MS = 200L
        private const val TONE_TYPE = ToneGenerator.TONE_PROP_ACK
        private const val TONE_VOLUME = 50
        private const val TONE_STREAM = AudioManager.STREAM_NOTIFICATION
    }

    private val _currentMode = MutableStateFlow(BellMode.SOUND)
    val currentMode: StateFlow<BellMode> = _currentMode
    private var lastBellTime = 0L
    private val scope = CoroutineScope(Dispatchers.Main + SupervisorJob())

    /** Restores the view background after a screen-flash bell. Tracked so a
     *  second bell cancels the first restore — otherwise the two restore
     *  coroutines race and the older one can clear the newer flash early. */
    private var flashRestoreJob: Job? = null

    /** Pre-flash background, remembered so a cancelled restore does not
     *  capture white as the "original" and permanently wash the view. */
    private var flashOriginalBackground: android.graphics.drawable.Drawable? = null
    private val toneGenerator: ToneGenerator? by lazy {
        try {
            ToneGenerator(TONE_STREAM, TONE_VOLUME)
        } catch (_: Exception) {
            null
        }
    }
    private val vibrator: Vibrator? by lazy {
        try {
            (context.getSystemService(Context.VIBRATOR_MANAGER_SERVICE) as? VibratorManager)?.defaultVibrator
        } catch (_: Exception) {
            null
        }
    }

    fun setMode(mode: BellMode) {
        _currentMode.value = mode
    }

    fun fireBell(
        targetView: View? = null,
        onAccessibility: (String) -> Unit = {},
    ): Boolean {
        val now = System.currentTimeMillis()
        if (now - lastBellTime < DEBOUNCE_MS) return false
        lastBellTime = now
        when (_currentMode.value) {
            BellMode.SOUND -> try {
                toneGenerator?.startTone(TONE_TYPE, 200)
            } catch (_: Exception) {
                // Ignore tone generation failures
            }

            BellMode.VIBRATE -> vibrator?.let {
                // SAFETY: Samsung Android 8 VibrationEffect.createOneShot can throw NPE in VibratorService$Vibration.mEffect
                try {
                    it.vibrate(VibrationEffect.createOneShot(VIBRATE_DURATION_MS, VibrationEffect.DEFAULT_AMPLITUDE))
                } catch (_: Exception) {
                    // Ignore vibration failures
                }
            }

            BellMode.SCREEN_FLASH -> {
                val v = targetView
                if (v != null) {
                    flashRestoreJob?.cancel()
                    // Capture the pre-flash background ONCE: taking it after
                    // cancel could read a still-white view (previous flash's
                    // restore was cancelled), permanently restoring white.
                    val originalBg = flashOriginalBackground ?: v.background
                    flashOriginalBackground = originalBg
                    v.setBackgroundColor(android.graphics.Color.WHITE)
                    flashRestoreJob =
                        scope.launch {
                            delay(FLASH_DURATION_MS)
                            v.background = originalBg
                            flashOriginalBackground = null
                        }
                } else {
                    // Fallback to sound when no view is available (runtime singleton).
                    // ToneGenerator is unavailable on devices without an audio
                    // output (e.g. emulators with no audio HAL); silence is the
                    // only sensible fallback, so the failure is intentionally
                    // swallowed.
                    try {
                        toneGenerator?.startTone(TONE_TYPE, 200)
                    } catch (_: Exception) {
                        // No audio HAL — bell stays silent.
                    }
                }
            }

            BellMode.SILENT -> {}
        }
        onAccessibility("Bell")
        return true
    }

    fun dispose() {
        scope.cancel()
        toneGenerator?.release()
    }
}
