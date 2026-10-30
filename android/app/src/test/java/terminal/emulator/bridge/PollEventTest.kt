package terminal.emulator.bridge

import kotlinx.serialization.decodeFromString
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotNull
import org.junit.Test

/**
 * Verifies [pollEventJson] can decode the exact JSON shape Rust emits
 * (`native/src/event.rs`, `#[serde(tag = "event", rename_all = "snake_case")]`).
 *
 * Regression: the Json instance lacked `classDiscriminator =
 * "event"`, so kotlinx.serialization looked for the default "type" field,
 * rejected every event with "Class discriminator was missing", and the
 * poll loop dropped clipboard/exit silently — a dead
 * shell left the render thread running forever (emulator-verified).
 */
class PollEventTest {

    @Test
    fun `clipboard event decodes with event discriminator`() {
        val event: PollEvent =
            pollEventJson.decodeFromString("""{"event":"clipboard","session_id":7,"text":"hello"}""")
        assertEquals(PollEvent.Clipboard(sessionId = 7L, text = "hello"), event)
    }

    @Test
    fun `exit event decodes with event discriminator`() {
        val event: PollEvent =
            pollEventJson.decodeFromString("""{"event":"exit","session_id":7,"code":137}""")
        assertEquals(PollEvent.Exit(sessionId = 7L, code = 137), event)
    }

    @Test
    fun `unknown fields are ignored`() {
        val event: PollEvent =
            pollEventJson.decodeFromString("""{"event":"clipboard","session_id":1,"text":"x","future_field":42}""")
        assertEquals(PollEvent.Clipboard(sessionId = 1L, text = "x"), event)
    }

    @Test
    fun `missing optional fields fall back to defaults`() {
        val event: PollEvent = pollEventJson.decodeFromString("""{"event":"clipboard"}""")
        assertEquals(PollEvent.Clipboard(sessionId = 0L, text = ""), event)
    }

    @Test
    fun `every rust event variant has a decodable discriminator`() {
        // Keep in sync with native/src/event.rs variant list. Decoding
        // must succeed for every discriminator the Rust side can emit.
        val samples =
            listOf(
                """{"event":"clipboard","session_id":1,"text":"x"}""",
                """{"event":"exit","session_id":1,"code":0}""",
                """{"event":"clipboard_read","session_id":1,"request_id":2,"selection":"x"}""",
            )
        samples.forEach { sample ->
            // Decoding succeeding is the assertion: a missing/renamed
            // discriminator throws SerializationException here.
            pollEventJson.decodeFromString<PollEvent>(sample)
        }
    }

    @Test
    fun `unknown discriminator is rejected`() {
        val thrown =
            try {
                pollEventJson.decodeFromString<PollEvent>("""{"event":"no_such_event","session_id":1}""")
                null
            } catch (exception: kotlinx.serialization.SerializationException) {
                exception
            }
        // A renamed Rust discriminator must surface here so Kotlin's
        // decoder learns about it instead of silently misreading events.
        assertNotNull("unknown event discriminator must throw", thrown)
    }
}
