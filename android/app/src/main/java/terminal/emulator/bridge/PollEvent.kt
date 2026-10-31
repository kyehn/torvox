package terminal.emulator.bridge

import kotlinx.serialization.ExperimentalSerializationApi
import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable
import kotlinx.serialization.json.Json

/**
 * Rust 原生侧推送的事件，以 internal tagging 序列化，Kotlin 侧匹配 `event` 判别字段。
 * 所有字段均带默认值，缺失或未知字段退化为合理值（coerceInputValues + ignoreUnknownKeys）。
 * Rust 与 Kotlin 的 schema 必须保持一致。
 */
@Serializable
sealed class PollEvent {
    @Serializable
    @SerialName("clipboard")
    data class Clipboard(@SerialName("session_id") val sessionId: Long = 0, val text: String = "") : PollEvent()

    @Serializable
    @SerialName("exit")
    data class Exit(
        @SerialName("session_id") val sessionId: Long = 0,
        /**
         * 退出码；`null` 表示原生侧 `waitpid` 失败、子进程已退出但码无从取得。
         *
         * 刻意**不给默认值**：缺省成 0 会把「查不到原因」读成「正常退出」，界面随即按
         * exit 0 直接关闭会话，用户连一个 `[Process completed]` 提示都看不到。原生侧
         * 序列化为显式 `null`，故真实事件永远带该字段。
         */
        val code: Int?,
    ) : PollEvent()

    @Serializable
    @SerialName("clipboard_read")
    data class ClipboardRead(
        @SerialName("session_id") val sessionId: Long = 0,
        @SerialName("request_id") val requestId: Long = 0,
        val selection: String = "",
    ) : PollEvent()
}

/**
 * [PollEvent] 的 JSON 编解码器。
 *
 * `ignoreUnknownKeys` 允许 Rust 增字段；`coerceInputValues` 让缺失/非法值回落默认值。
 * `exceptionsWithDebugInfo = false`：解码错误不得把涉事的 JSON（可能含剪贴板文本/URL）写入日志。
 */
@OptIn(ExperimentalSerializationApi::class)
val pollEventJson: Json =
    Json {
        // Rust 用 `#[serde(tag = "event")]`（internal tagging）序列化，
        // 而 kotlinx 默认判别字段是 "type"，会导致所有事件被拒并静默丢弃——
        // 退出事件永远到不了 Kotlin，shell 死后终端将卡死而渲染线程空转。
        classDiscriminator = "event"
        ignoreUnknownKeys = true
        coerceInputValues = true
        exceptionsWithDebugInfo = false
    }
