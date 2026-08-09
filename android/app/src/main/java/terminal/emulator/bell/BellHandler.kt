package terminal.emulator.bell

import android.content.Context
import android.media.AudioManager
import android.media.ToneGenerator
import android.os.Build
import android.os.VibrationEffect
import android.os.Vibrator
import android.os.VibratorManager
import android.view.View
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
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
    private val toneGenerator: ToneGenerator? by lazy {
        try {
            ToneGenerator(TONE_STREAM, TONE_VOLUME)
        } catch (_: Exception) {
            null
        }
    }
    private val vibrator: Vibrator? by lazy {
        try {
            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.S) {
                (context.getSystemService(Context.VIBRATOR_MANAGER_SERVICE) as? VibratorManager)?.defaultVibrator
            } else {
                @Suppress("DEPRECATION")
                context.getSystemService(Context.VIBRATOR_SERVICE) as? Vibrator
            }
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
            BellMode.SOUND -> toneGenerator?.startTone(TONE_TYPE, 200)

            BellMode.VIBRATE -> vibrator?.let {
                if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
                    it.vibrate(VibrationEffect.createOneShot(VIBRATE_DURATION_MS, VibrationEffect.DEFAULT_AMPLITUDE))
                } else {
                    @Suppress("DEPRECATION")
                    it.vibrate(VIBRATE_DURATION_MS)
                }
            }

            BellMode.SCREEN_FLASH -> targetView?.let { v ->
                val originalBg = v.background
                v.setBackgroundColor(android.graphics.Color.WHITE)
                scope.launch {
                    delay(FLASH_DURATION_MS)
                    v.background = originalBg
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
