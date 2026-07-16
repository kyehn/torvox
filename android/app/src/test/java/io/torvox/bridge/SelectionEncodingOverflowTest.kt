package io.torvox.bridge

import org.junit.Assert.assertThrows
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * K5 — Selection encoding overflow guard.
 *
 * `expandAndSetSelection` packs row/col into 16 bits. Values above `0xFFFF`
 * would be silently truncated by the wire packing, so the method must reject
 * them with a clear [IllegalArgumentException] before reaching the native call.
 *
 * The overflow path is exercised on a `TorvoxBridge(0L)` because the guard
 * throws before `ensureLib()` loads the native library. The "valid values"
 * path is checked only up to the guard: the call must not be rejected by the
 * 16-bit validation (the downstream native `UnsatisfiedLinkError` in a unit
 * environment proves the guard was passed).
 */
class SelectionEncodingOverflowTest {
    @Test
    fun expandAndSetSelection_throwsOnRowOverflow() {
        val bridge = TorvoxBridge(0L)
        val exception =
            assertThrows(IllegalArgumentException::class.java) {
                bridge.expandAndSetSelection(row = 0x10000u, col = 0u)
            }
        assertTrue(
            "message should mention the 16-bit packing range, was: ${exception.message}",
            exception.message?.contains("16-bit") == true,
        )
    }

    @Test
    fun expandAndSetSelection_throwsOnColOverflow() {
        val bridge = TorvoxBridge(0L)
        val exception =
            assertThrows(IllegalArgumentException::class.java) {
                bridge.expandAndSetSelection(row = 0u, col = 0x10000u)
            }
        assertTrue(exception.message?.contains("16-bit") == true)
    }

    @Test
    fun expandAndSetSelection_throwsOnBothOverflow() {
        val bridge = TorvoxBridge(0L)
        val exception =
            assertThrows(IllegalArgumentException::class.java) {
                bridge.expandAndSetSelection(row = 0x1FFFFu, col = 0x1FFFFu)
            }
        assertTrue(exception.message?.contains("16-bit") == true)
    }

    @Test
    fun expandAndSetSelection_acceptsMaxPackedValues() {
        val bridge = TorvoxBridge(0L)
        // 0xFFFF is exactly the top of the 16-bit range and must not be rejected
        // by the validation guard. The downstream native call is expected to fail
        // in a unit environment (no .so on the classpath) which proves the guard
        // allowed the value through.
        try {
            bridge.expandAndSetSelection(row = 0xFFFFu, col = 0xFFFFu)
        } catch (overflow: IllegalArgumentException) {
            throw AssertionError("valid 16-bit values must not be rejected by the guard", overflow)
        } catch (_: UnsatisfiedLinkError) {
            // Expected: native library not available in unit tests — proves the
            // guard allowed the value through to the native call.
        } catch (_: IllegalStateException) {
            // Expected: JNA init may throw IllegalStateException when native
            // library cannot be loaded in a unit-test environment. Same proof
            // value as UnsatisfiedLinkError — the guard was passed.
        }
    }

    @Test
    fun expandAndSetSelection_acceptsSmallValues() {
        val bridge = TorvoxBridge(0L)
        try {
            bridge.expandAndSetSelection(row = 10u, col = 5u)
        } catch (overflow: IllegalArgumentException) {
            throw AssertionError("small values must not be rejected by the guard", overflow)
        } catch (_: UnsatisfiedLinkError) {
            // Expected: native library not available in unit tests — proves the
            // guard allowed the value through to the native call.
        } catch (_: IllegalStateException) {
            // Expected: JNA init may throw IllegalStateException when native
            // library cannot be loaded in a unit-test environment. Same proof
            // value as UnsatisfiedLinkError — the guard was passed.
        }
    }
}
