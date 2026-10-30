package terminal.emulator.runtime

import android.Manifest
import android.content.Context
import android.content.pm.PackageManager
import android.media.AudioManager
import android.media.ToneGenerator
import android.os.Build
import android.os.SystemClock
import android.os.VibrationEffect
import android.os.Vibrator
import android.os.VibratorManager
import terminal.emulator.settings.BellCharacter

/** 两次响铃之间的最小间隔：连打 BEL 的程序不该把手机震成蜂鸣器。 */
private const val MIN_RING_INTERVAL_MS = 120L

/** 震动时长（毫秒）。取「短促一下」，与系统通知的默认轻震同量级。 */
private const val VIBRATE_MS = 40L

/** 提示音音量（ToneGenerator 的 0..100 刻度）。取中段：够听见，又不至于吓人。 */
private const val BEEP_VOLUME = 60

/** 提示音时长（毫秒）。 */
private const val BEEP_MS = 150

/**
 * 终端响铃的落地：按 `~/.termux/termux.properties` 的 `bell-character` 震动、出声或忽略。
 *
 * 三态与 termux 一致——`vibrate`（默认）/`beep`/`ignore`。BEL 由原生 `on_bell` 回调捕获
 * 并合并成「响过」一个二值信号，故这里收到的已经是最少次数。
 *
 * [ToneGenerator] 的构造要开一条音频输出，失败（无音频设备、音频服务不可用）时返回 null
 * 并不再重试：响铃是提示，不是功能，为它反复尝试只会拖慢渲染线程。
 */
internal class BellAlerter(context: Context) {
    private val appContext = context.applicationContext
    private var lastRingMs = 0L
    private var toneGenerator: ToneGenerator? = null
    private var toneUnavailable = false

    /**
     * 按 [character] 提示一次。
     *
     * 调用点是渲染线程（事件派发），故这里只做系统服务的异步调用，不碰 UI。
     *
     * 震动权限在**调用点**检查而不是靠 `@RequiresPermission` 传播：后者会把要求一路
     * 推到渲染循环，最终只能整条链加注解或抑制，而真正需要回答的问题只有一个——
     * 权限有没有被用户拒绝。
     */
    fun ring(character: BellCharacter) {
        if (character == BellCharacter.IGNORE) return
        val now = SystemClock.uptimeMillis()
        if (now - lastRingMs < MIN_RING_INTERVAL_MS) return
        lastRingMs = now
        when (character) {
            BellCharacter.VIBRATE -> vibrate()
            BellCharacter.BEEP -> beep()
            // 上面的提前返回已挡掉 IGNORE；列出它是为了让新增取值在编译期就暴露，
            // 而不是悄悄落进某个已有分支。
            BellCharacter.IGNORE -> Unit
        }
    }

    private fun vibrate() {
        if (appContext.checkSelfPermission(Manifest.permission.VIBRATE) != PackageManager.PERMISSION_GRANTED) {
            return
        }
        val vibrator = vibrator() ?: return
        if (!vibrator.hasVibrator()) return
        vibrator.vibrate(
            VibrationEffect.createOneShot(VIBRATE_MS, VibrationEffect.DEFAULT_AMPLITUDE),
        )
    }

    private fun vibrator(): Vibrator? =
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.S) {
            (appContext.getSystemService(Context.VIBRATOR_MANAGER_SERVICE) as? VibratorManager)
                ?.defaultVibrator
        } else {
            @Suppress("DEPRECATION")
            appContext.getSystemService(Context.VIBRATOR_SERVICE) as? Vibrator
        }

    private fun beep() {
        if (toneUnavailable) return
        val generator =
            toneGenerator
                ?: try {
                    ToneGenerator(AudioManager.STREAM_NOTIFICATION, BEEP_VOLUME).also {
                        toneGenerator = it
                    }
                } catch (error: RuntimeException) {
                    // 音频服务不可用（无输出设备的模拟器常见）。记一次即可，
                    // 后续响铃直接跳过——反复抛异常只会每帧刷一条日志。
                    toneUnavailable = true
                    LogUtil.warn("Bell", "ToneGenerator unavailable: ${error.message}")
                    return
                }
        generator.startTone(ToneGenerator.TONE_PROP_BEEP, BEEP_MS)
    }
}
