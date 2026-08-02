package terminal.emulator.bridge

import kotlinx.serialization.ExperimentalSerializationApi
import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable
import kotlinx.serialization.json.Json

/**
 * Events pushed from the Rust native side, serialised with internal
 * tagging (`#[serde(tag = "event", rename_all = "snake_case")]` — see
 * `native/src/event.rs`). Kotlin matches on the `event` discriminator.
 *
 * All fields carry defaults so a missing/unknown field degrades to a
 * sensible value (`coerceInputValues` + `ignoreUnknownKeys`), mirroring
 * the previous `org.json` `opt*` tolerance. Rust and Kotlin schema must
 * stay in sync; a schema drift now surfaces at decode time.
 */
@Serializable
sealed class PollEvent {
    @Serializable
    @SerialName("bell")
    data class Bell(
        @SerialName("session_id") val sessionId: Long = 0,
    ) : PollEvent()

    @Serializable
    @SerialName("clipboard")
    data class Clipboard(
        @SerialName("session_id") val sessionId: Long = 0,
        val text: String = "",
    ) : PollEvent()

    @Serializable
    @SerialName("notification")
    data class Notification(
        @SerialName("session_id") val sessionId: Long = 0,
        val title: String = "",
        val body: String = "",
    ) : PollEvent()

    @Serializable
    @SerialName("exit")
    data class Exit(
        @SerialName("session_id") val sessionId: Long = 0,
        val code: Int = 0,
    ) : PollEvent()

    @Serializable
    @SerialName("show_dialog")
    data class ShowDialog(
        @SerialName("session_id") val sessionId: Long = 0,
        @SerialName("request_id") val requestId: Long = 0,
        @SerialName("dialog_type") val dialogType: String = "",
        val title: String = "",
        val message: String = "",
        val options: List<String> = emptyList(),
    ) : PollEvent()

    @Serializable
    @SerialName("pick_file")
    data class PickFile(
        @SerialName("session_id") val sessionId: Long = 0,
        @SerialName("request_id") val requestId: Long = 0,
        @SerialName("starting_path") val startingPath: String = "",
        val filter: String = "",
    ) : PollEvent()

    @Serializable
    @SerialName("get_clipboard")
    data class GetClipboard(
        @SerialName("session_id") val sessionId: Long = 0,
        @SerialName("request_id") val requestId: Long = 0,
    ) : PollEvent()

    @Serializable
    @SerialName("toast")
    data class Toast(val text: String = "") : PollEvent()

    @Serializable
    @SerialName("open_url")
    data class OpenUrl(val url: String = "") : PollEvent()
}

/**
 * JSON codec for [PollEvent].
 *
 * - `ignoreUnknownKeys`: Rust may add fields without breaking this side.
 * - `coerceInputValues`: missing/illegal values fall back to defaults
 *   (matches the previous `opt*` tolerance).
 * - `exceptionsWithDebugInfo = false`: decode errors must not embed the
 *   offending JSON (which may contain clipboard text / URLs) in logs.
 */
@OptIn(ExperimentalSerializationApi::class)
val pollEventJson: Json =
    Json {
        ignoreUnknownKeys = true
        coerceInputValues = true
        exceptionsWithDebugInfo = false
    }
