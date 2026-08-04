package terminal.emulator.bridge

import kotlinx.serialization.decodeFromString
import org.junit.Assert.assertEquals
import org.junit.Test

/**
 * Verifies [pollEventJson] can decode the exact JSON shape Rust emits
 * (`native/src/event.rs`, `#[serde(tag = "event", rename_all = "snake_case")]`).
 *
 * Round-213 regression: the Json instance lacked `classDiscriminator =
 * "event"`, so kotlinx.serialization looked for the default "type" field,
 * rejected every event with "Class discriminator was missing", and the
 * poll loop dropped bell/clipboard/exit/notification silently — a dead
 * shell left the render thread running forever (emulator-verified).
 */
class PollEventTest {

    @Test
    fun `bell event decodes with event discriminator`() {
        val event: PollEvent = pollEventJson.decodeFromString("""{"event":"bell","session_id":7}""")
        assertEquals(PollEvent.Bell(sessionId = 7L), event)
    }

    @Test
    fun `clipboard event decodes with event discriminator`() {
        val event: PollEvent =
            pollEventJson.decodeFromString("""{"event":"clipboard","session_id":7,"text":"hello"}""")
        assertEquals(PollEvent.Clipboard(sessionId = 7L, text = "hello"), event)
    }

    @Test
    fun `notification event decodes with event discriminator`() {
        val event: PollEvent =
            pollEventJson.decodeFromString(
                """{"event":"notification","session_id":7,"title":"t","body":"b"}""",
            )
        assertEquals(PollEvent.Notification(sessionId = 7L, title = "t", body = "b"), event)
    }

    @Test
    fun `exit event decodes with event discriminator`() {
        val event: PollEvent =
            pollEventJson.decodeFromString("""{"event":"exit","session_id":7,"code":137}""")
        assertEquals(PollEvent.Exit(sessionId = 7L, code = 137), event)
    }

    @Test
    fun `show dialog event decodes with event discriminator`() {
        val event: PollEvent =
            pollEventJson.decodeFromString(
                """{"event":"show_dialog","session_id":7,"request_id":9,"dialog_type":"confirm","title":"t","message":"m","options":["a","b"]}""",
            )
        assertEquals(
            PollEvent.ShowDialog(
                sessionId = 7L,
                requestId = 9L,
                dialogType = "confirm",
                title = "t",
                message = "m",
                options = listOf("a", "b"),
            ),
            event,
        )
    }

    @Test
    fun `unknown fields are ignored`() {
        val event: PollEvent =
            pollEventJson.decodeFromString("""{"event":"bell","session_id":1,"future_field":42}""")
        assertEquals(PollEvent.Bell(sessionId = 1L), event)
    }

    @Test
    fun `missing optional fields fall back to defaults`() {
        val event: PollEvent = pollEventJson.decodeFromString("""{"event":"bell"}""")
        assertEquals(PollEvent.Bell(sessionId = 0L), event)
    }

    @Test
    fun `every rust event variant has a decodable discriminator`() {
        // Keep in sync with native/src/event.rs variant list. Decoding
        // must succeed for every discriminator the Rust side can emit.
        val samples =
            listOf(
                """{"event":"bell","session_id":1}""",
                """{"event":"clipboard","session_id":1,"text":"x"}""",
                """{"event":"notification","session_id":1,"title":"t","body":"b"}""",
                """{"event":"exit","session_id":1,"code":0}""",
                """{"event":"show_dialog","session_id":1,"request_id":2,"dialog_type":"input","title":"t","message":"m","options":[]}""",
                """{"event":"pick_file","session_id":1,"request_id":2,"starting_path":"/sdcard","filter":""}""",
                """{"event":"dialog_cancel","session_id":1,"request_id":2}""",
                """{"event":"get_clipboard","session_id":1,"request_id":2}""",
                """{"event":"clipboard_read","session_id":1,"request_id":2,"selection":"x"}""",
                """{"event":"toast","text":"t"}""",
                """{"event":"open_url","url":"https://example.com"}""",
            )
        samples.forEach { sample ->
            // Decoding succeeding is the assertion: a missing/renamed
            // discriminator throws SerializationException here.
            pollEventJson.decodeFromString<PollEvent>(sample)
        }
    }
}
