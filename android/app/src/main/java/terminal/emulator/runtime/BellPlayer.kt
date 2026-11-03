package terminal.emulator.runtime

import android.content.Context
import android.media.AudioManager
import android.media.ToneGenerator
import android.os.VibrationEffect
import android.os.VibratorManager
import terminal.emulator.settings.BellCharacter

/**
 * 终端响铃（BEL，`\a`）的三态执行器，语义对齐 termux-app `TerminalEmulator`：
 * `vibrate` 振动 [VIBRATION_MILLIS]、`beep` 播放系统提示音、`ignore` 什么都不做。
 *
 * 限频 [MIN_INTERVAL_NANOS]：BEL 是**远端程序**可无限发的字节，`yes '\a'` 一秒能刷出
 * 上万次。不限频会把振动器与音频通道打成持续占用（termux 的 `BellHandler.MIN_PAUSE`
 * 是同一个理由）。
 *
 * 动作以 lambda 注入：单测在 Robolectric 之外直接验证三态与限频，不必伪造系统服务。
 * 依赖系统服务的默认动作在伴生对象中以**纯函数**实现——次构造函数的 lambda 体内
 * 不得引用实例成员（主构造尚未执行，Kotlin 按 `INSTANCE_ACCESS_BEFORE_SUPER_CALL`
 * 拒绝），ToneGenerator 的复用槽也由工厂闭包捕获而非挂在实例上。
 */
class BellPlayer internal constructor(
    private val vibrateAction: () -> Unit,
    private val beepAction: () -> Unit,
    private val nowNanos: () -> Long = System::nanoTime,
) {
    constructor(context: Context) : this(
        vibrateAction = { vibrateOnce(context) },
        beepAction = beepAction(context),
    )

    /**
     * 是否已执行过动作。单独的布尔哨兵而不能用「时间戳非 0」判定：
     * `System.nanoTime` 允许返回任意值（含 0 与负数，JDK 文档只保证跨调用差值
     * 单调），拿任何固定值当哨兵都会与真实时间戳撞车——首铃在时钟归零前后
     * 触发时限频会整个失效。
     */
    private var hasActionRun = false

    /** 上次真正执行动作的时刻（纳秒）。 */
    private var lastActionNanos = 0L

    /**
     * 按 [mode] 响一次铃。限频窗口内的后续调用直接丢弃——铃是提示而非状态，
     * 丢弃不会让任何东西失步（与剪贴板事件的「绝不丢」是两种语义）。
     */
    fun ring(mode: BellCharacter) {
        if (mode == BellCharacter.IGNORE) return
        val timestamp = nowNanos()
        if (hasActionRun && timestamp - lastActionNanos < MIN_INTERVAL_NANOS) return
        hasActionRun = true
        lastActionNanos = timestamp
        when (mode) {
            BellCharacter.VIBRATE -> vibrateAction()
            BellCharacter.BEEP -> beepAction()
            BellCharacter.IGNORE -> Unit
        }
    }

    private companion object {
        const val TAG = "BellPlayer"

        /** 振动时长（毫秒），与 termux `BellHandler` 的 `VIBRATION_DURATION` 一致。 */
        const val VIBRATION_MILLIS = 50L

        /** 提示音时长（毫秒）。 */
        const val BEEP_MILLIS = 150

        /** 提示音音量百分比（`ToneGenerator` 的取值范围是 0..100）。 */
        const val TONE_VOLUME_PERCENT = 80

        /** 两次可感知动作的最小间隔（纳秒）。 */
        const val MIN_INTERVAL_NANOS = 150_000_000L

        private fun vibrateOnce(context: Context) {
            val vibrator =
                (context.getSystemService(Context.VIBRATOR_MANAGER_SERVICE) as? VibratorManager)?.defaultVibrator
            if (vibrator == null || !vibrator.hasVibrator()) {
                LogUtil.warn(TAG, "no vibrator; bell vibration skipped")
                return
            }
            vibrator.vibrate(VibrationEffect.createOneShot(VIBRATION_MILLIS, VibrationEffect.DEFAULT_AMPLITUDE))
        }

        /**
         * 构造提示音动作：`ToneGenerator` 持有原生 AudioTrack，不可每次响铃新建，
         * 复用槽由返回的闭包捕获（进程级单实例语义与挂实例字段等价）。
         */
        private fun beepAction(context: Context): () -> Unit {
            val holder = arrayOfNulls<ToneGenerator>(1)
            return beep@{
                val generator =
                    holder[0] ?: runCatching {
                        ToneGenerator(AudioManager.STREAM_NOTIFICATION, TONE_VOLUME_PERCENT)
                    }
                        .onFailure { LogUtil.warn(TAG, "ToneGenerator unavailable; bell beep skipped", it) }
                        .getOrNull()
                        ?.also { holder[0] = it }
                        ?: return@beep
                if (!generator.startTone(ToneGenerator.TONE_PROP_BEEP, BEEP_MILLIS)) {
                    // 音频通道被通话/勿扰占用时 startTone 返回 false。记一条即可：
                    // 这不是应用缺陷，用户此刻也确实听不到提示音。
                    LogUtil.warn(TAG, "bell beep rejected by audio system")
                }
            }
        }
    }
}
