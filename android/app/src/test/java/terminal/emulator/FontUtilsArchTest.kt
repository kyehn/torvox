package terminal.emulator

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner

/**
 * detectArchFromAbi / is64BitAbi map Android ABI names to the linker
 * triplet used by bootstrap installs. The ABI list is a static field, so
 * each case pins it via reflection before calling the function.
 */
@RunWith(RobolectricTestRunner::class)
class FontUtilsArchTest {

    private fun withAbis(vararg abis: String, block: () -> Unit) {
        val field = android.os.Build::class.java.getDeclaredField("SUPPORTED_ABIS")
        field.isAccessible = true
        field.set(null, arrayOf(*abis))
        block()
    }

    @Test
    fun `arm64-v8a maps to aarch64 and is 64-bit`() {
        withAbis("arm64-v8a") {
            assertEquals("aarch64", detectArchFromAbi())
            assertTrue(is64BitAbi())
        }
    }

    @Test
    fun `armeabi-v7a maps to arm and is 32-bit`() {
        withAbis("armeabi-v7a") {
            assertEquals("arm", detectArchFromAbi())
            assertFalse(is64BitAbi())
        }
    }

    @Test
    fun `x86_64 maps to x86_64 and is 64-bit`() {
        withAbis("x86_64") {
            assertEquals("x86_64", detectArchFromAbi())
            assertTrue(is64BitAbi())
        }
    }

    @Test
    fun `x86 maps to i686 and is 32-bit`() {
        withAbis("x86") {
            assertEquals("i686", detectArchFromAbi())
            assertFalse(is64BitAbi())
        }
    }

    @Test
    fun `unknown abi falls back to aarch64`() {
        withAbis("riscv64") {
            assertEquals("aarch64", detectArchFromAbi())
            assertTrue("riscv64 contains 64, so 64-bit", is64BitAbi())
        }
    }

    @Test
    fun `empty abi list falls back to aarch64 and 32-bit`() {
        withAbis {
            assertEquals("aarch64", detectArchFromAbi())
            assertFalse(is64BitAbi())
        }
    }
}
