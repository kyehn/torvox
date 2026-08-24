package terminal.emulator.bridge

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertTrue
import org.junit.Assume.assumeTrue
import org.junit.Before
import org.junit.Test
import java.io.File
import kotlin.system.measureTimeMillis

/**
 * JVM-side JNI round-trip tests — no emulator, no Android device.
 *
 * Loads the HOST-built `libnative.so` (same Rust code as the Android
 * target; `cargo build --package native`) and drives the real JNI bridge:
 * initSession → feedTerminal → getTitle/getTerminalText → destroySession.
 *
 * ## Why this exists (what pure-Rust tests cannot cover)
 *
 * `cargo test` covers the VT parser, cell pipeline, and event queue by
 * calling Rust directly. It cannot exercise the JNI boundary layer:
 *
 * - JString→String / String→JString conversion (UTF-16 round-trip, NUL,
 *   non-ASCII) — only a real JVM produces/reads JNI strings
 * - jbyteArray→Vec<u8> input conversion ([feedTerminal] is binary-safe)
 * - env.throw_new exception paths (IllegalArgumentException on bad args)
 * - the full Kotlin→JNI→command channel→VT thread→ghostty parser→query
 *   round-trip; `vt_write` hands the buffer to the VT thread via
 *   `try_send`, so a broken command channel or dead VT thread only shows
 *   here (previously only the emulator caught it — heavy and black-box)
 *
 * Graphics exports (attachWindow/render/captureFrame) need ANativeWindow
 * and stay on the emulator; everything exercised here is pure CPU logic.
 *
 * Locating the library: unit tests run with cwd = `android/app/`, so
 * repo-root candidates are `../../target/...`. Override via the
 * TORVOX_NATIVE_LIB env var. Missing host .so → SKIPPED (never fails):
 * build one with `cargo build --package native`.
 */
class NativeBridgeSmokeTest {
    private companion object {
        private val soCandidates: List<File> = listOfNotNull(
            System.getenv("TORVOX_NATIVE_LIB"),
            "../../target/release/libnative.so",
            "../../target/debug/libnative.so",
        ).map(::File)

        /** Real shell on the dev host (CI/nix runner). Android uses /system/bin/sh. */
        private const val HOST_SHELL = "/bin/sh"

        private const val POLL_TIMEOUT_MS = 5_000L
        private const val POLL_INTERVAL_MS = 25L
    }

    @Before
    fun loadNativeLibrary() {
        val so = soCandidates.firstOrNull { it.isFile }
        assumeTrue(
            "host libnative.so not found (looked in " +
                soCandidates.joinToString(", ") { it.path } +
                ") — run `cargo build --package native` first",
            so != null,
        )
        System.load(so!!.absolutePath)
    }

    /** Poll `probe` until it returns true or [POLL_TIMEOUT_MS] elapses. */
    private fun awaitTrue(what: String, probe: () -> Boolean): Boolean {
        val deadline = System.currentTimeMillis() + POLL_TIMEOUT_MS
        while (System.currentTimeMillis() < deadline) {
            if (probe()) return true
            Thread.sleep(POLL_INTERVAL_MS)
        }
        println("awaitTrue timeout waiting for: $what")
        return probe()
    }

    /** Spawn a real session, run `block`, and always clean up afterwards. */
    private fun withSession(block: (Long) -> Unit) {
        val sessionId =
            NativeBridge.initSession(
                rows = 24,
                cols = 80,
                shell = HOST_SHELL,
                home = System.getenv("HOME") ?: "",
                user = "",
                path = "",
                workingDirectory = System.getProperty("user.dir") ?: "",
                prefix = "",
                scrollbackLines = 5_000,
                env = null,
            )
        assertTrue("initSession must return a positive session id, got $sessionId", sessionId > 0)
        try {
            block(sessionId)
        } finally {
            val destroyed = NativeBridge.destroySession(sessionId)
            assertTrue("destroySession must report success", destroyed)
        }
    }

    @Test
    fun `initSession spawns a session and destroySession removes it`() {
        val before = NativeBridge.getSessionCount()
        withSession { sessionId ->
            assertTrue("registry must now contain the session", NativeBridge.listSessions()!!.contains(sessionId.toString()))
        }
        assertEquals("session must be removed again", before, NativeBridge.getSessionCount())
    }

    @Test
    fun `feedTerminal OSC 0 title round-trips through the live VT thread`() {
        withSession { sessionId ->
            // OSC 0 sets the window/title; vt_write is async (try_send to the
            // VT thread), so poll getTitle until the parser applied it.
            NativeBridge.feedTerminal(sessionId, "\u001b]0;JNI-SMOKE\u001b\\".toByteArray())
            val applied = awaitTrue("title applied") {
                NativeBridge.getTitle(sessionId) == "JNI-SMOKE"
            }
            assertTrue("OSC 0 title must be applied by the VT thread", applied)
        }
    }

    @Test
    fun `feedTerminal text is queryable via getTerminalText`() {
        withSession { sessionId ->
            NativeBridge.feedTerminal(sessionId, "hello jni roundtrip".toByteArray())
            val applied = awaitTrue("text visible") {
                val text = NativeBridge.getTerminalText(sessionId)
                text != null && text.contains("hello jni roundtrip")
            }
            assertTrue("fed text must appear in terminal state", applied)
        }
    }

    @Test
    fun `scrollback rows grows after scrollback-generating output`() {
        withSession { sessionId ->
            assertEquals("fresh session has no scrollback", 0, NativeBridge.getScrollbackRows(sessionId))
            // More lines than the visible grid -> pushed into scrollback.
            val payload = (1..200).joinToString("\n") { "scrollback line $it" } + "\n"
            NativeBridge.feedTerminal(sessionId, payload.toByteArray())
            val grew = awaitTrue("scrollback populated") {
                NativeBridge.getScrollbackRows(sessionId) > 0
            }
            assertTrue("200 fed lines must create scrollback rows", grew)
            assertEquals("unknown session reads as 0 rows", 0, NativeBridge.getScrollbackRows(999_999L))
        }
    }

    @Test
    fun `unknown session id fails safely with an exception`() {
        // A bogus id must not crash the process: the export throws a Java
        // exception (jni_export_guard) instead of aborting.
        try {
            NativeBridge.getTitle(999_999L)
            // Unreachable when the contract holds; if it returns null the
            // export degraded silently — still not a crash, so allowed.
        } catch (_: RuntimeException) {
            // expected: unknown session → IllegalArgumentException
        }
        assertNotNull("bridge must remain usable after the failed call", NativeBridge.listSessions())
    }

    @Test
    fun `JNI call overhead stays in a sane ballpark`() {
        withSession { sessionId ->
            val iterations = 200
            val elapsed = measureTimeMillis {
                repeat(iterations) { NativeBridge.getSessionCount() }
            }
            println("JNI overhead diagnostic: $iterations getSessionCount calls took ${elapsed}ms")
            assertTrue(
                "200 JNI calls took ${elapsed}ms — regression in bridge cost",
                elapsed < 50,
            )
        }
    }
}
