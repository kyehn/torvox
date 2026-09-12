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
    @SerialName("clipboard")
    data class Clipboard(
        @SerialName("session_id") val sessionId: Long = 0,
        val text: String = "",
    ) : PollEvent()

    @Serializable
    @SerialName("exit")
    data class Exit(
        @SerialName("session_id") val sessionId: Long = 0,
        val code: Int = 0,
        // child lifetime (ms, fork → waitpid) measured natively.
        @SerialName("alive_ms") val aliveMs: Long = 0,
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
 * JSON codec for [PollEvent].
 *
 * - `ignoreUnknownKeys`: Rust may add fields without breaking this side.
 * - `coerceInputValues`: missing/illegal values fall back to defaults
 * matches the previous `opt*` tolerance).
 * - `exceptionsWithDebugInfo = false`: decode errors must not embed the
 *   offending JSON (which may contain clipboard text / URLs) in logs.
 */
@OptIn(ExperimentalSerializationApi::class)
val pollEventJson: Json =
    Json {
        // Rust serialises Event with `#[serde(tag = "event")]` (internal
        // tagging); kotlinx default discriminator is "type", which would
        // reject every event with "Class discriminator was missing" and
        // silently drop clipboard/exit — the exit event
        // never reached Kotlin, so a dead shell left the terminal frozen
        // with the render thread running forever, emulator-
        // verified via `kill -9 <shell>`).
        classDiscriminator = "event"
        ignoreUnknownKeys = true
        coerceInputValues = true
        exceptionsWithDebugInfo = false
    }
