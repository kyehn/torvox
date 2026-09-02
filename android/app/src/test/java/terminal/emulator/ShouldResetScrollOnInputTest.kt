package terminal.emulator

import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * table-driven tests for [shouldResetScrollOnInput] — the pure input-driven scroll-reset decision
 * (spec terminal-scrolling "五维归零判定": user input containing CR/LF snaps the viewport to the live
 * screen without waiting for PTY output).
 *
 * Boundary semantics pinned here:
 * - a bare `\r` (Enter on an idle prompt, no echo) MUST trigger the snap;
 * - plain printable input (no newline) MUST NOT move the viewport;
 * - an active selection or handle drag suppresses the snap exactly like the output-driven path;
 * - empty writes and control bytes other than CR/LF never trigger.
 */
class ShouldResetScrollOnInputTest {

  private val cr = byteArrayOf('\r'.code.toByte())
  private val lf = byteArrayOf('\n'.code.toByte())
  private val crlf = byteArrayOf('l'.code.toByte(), 's'.code.toByte(), '\r'.code.toByte())

  @Test
  fun bareCarriageReturnTriggersSnap() {
    assertTrue(shouldResetScrollOnInput(cr, hasSelectionOrDrag = false))
  }

  @Test
  fun bareLineFeedTriggersSnap() {
    assertTrue(shouldResetScrollOnInput(lf, hasSelectionOrDrag = false))
  }

  @Test
  fun commandWithTrailingNewlineTriggersSnap() {
    assertTrue(shouldResetScrollOnInput(crlf, hasSelectionOrDrag = false))
  }

  @Test
  fun printableInputWithoutNewlineDoesNotSnap() {
    assertFalse(
        shouldResetScrollOnInput(
            byteArrayOf('l'.code.toByte(), 's'.code.toByte()),
            hasSelectionOrDrag = false,
        ),
    )
  }

  @Test
  fun emptyWriteDoesNotSnap() {
    assertFalse(shouldResetScrollOnInput(ByteArray(0), hasSelectionOrDrag = false))
  }

  @Test
  fun activeSelectionSuppressesSnap() {
    assertFalse(shouldResetScrollOnInput(crlf, hasSelectionOrDrag = true))
  }

  @Test
  fun activeHandleDragSuppressesSnap() {
    assertFalse(shouldResetScrollOnInput(cr, hasSelectionOrDrag = true))
  }

  @Test
  fun controlBytesOtherThanCrLfDoNotTrigger() {
    // Tab, ESC, CSI bytes are common in raw keyboard mode; none of them
    // commits "back to live screen" intent.
    val tabEsc = byteArrayOf('\t'.code.toByte(), 0x1B, 0x5B) // \t ESC [
    assertFalse(shouldResetScrollOnInput(tabEsc, hasSelectionOrDrag = false))
  }

  @Test
  fun crlfInsideLargerPasteTriggersSnapOncePerCall() {
    // Multi-line paste: decision is per-call boolean, caller applies it once.
    val paste = "\n".repeat(3).toByteArray()
    assertTrue(shouldResetScrollOnInput(paste, hasSelectionOrDrag = false))
  }
}
