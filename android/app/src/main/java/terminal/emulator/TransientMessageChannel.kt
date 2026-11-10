package terminal.emulator

import kotlinx.coroutines.channels.BufferOverflow
import kotlinx.coroutines.flow.MutableSharedFlow
import kotlinx.coroutines.flow.SharedFlow
import kotlinx.coroutines.flow.asSharedFlow

/**
 * 应用内瞬时提示的投递通道。
 *
 * 提示由界面层订阅本通道并以 Snackbar 呈现，不借用平台提示窗口：平台提示窗口受两条
 * 约束——同一 UID 同时只允许一个窗口（连发时框架打出窗口冲突告警），以及按包的提示
 * 频率限额（超额的提示被直接丢弃，用户看不到反馈）。两条约束的告警都由平台打出，
 * 根因却在应用的调用方式。
 *
 * 消息只对当下有订阅者的界面可见：没有界面时消息直接丢弃，不排队补发——补发一条过时
 * 反馈比不反馈更糟。
 */
internal class TransientMessageChannel {
    private val messages =
        MutableSharedFlow<String>(
            replay = 0,
            extraBufferCapacity = 16,
            onBufferOverflow = BufferOverflow.DROP_OLDEST,
        )

    /** 提示流：界面层订阅它并逐条呈现。 */
    val flow: SharedFlow<String> = messages.asSharedFlow()

    /** 投递一条提示。没有订阅者时消息直接丢弃。 */
    fun send(text: String) {
        messages.tryEmit(text)
    }
}
