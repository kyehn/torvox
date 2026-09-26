package terminal.emulator

import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner

/**
 * detectArchFromAbi 把 Android ABI 名映射为 bootstrap 安装使用的链接器 triplet。
 * 只支持 arm64-v8a 与 x86_64（见 docs/specification/BUILD.md），因此只测这两个与回退。
 * ABI 列表是静态字段，每个用例用反射固定后再调用。
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
    fun `x86_64 maps to x86_64 and is 64-bit`() {
        withAbis("x86_64") {
            assertEquals("x86_64", detectArchFromAbi())
            assertTrue(is64BitAbi())
        }
    }

    @Test
    fun `unsupported abi falls back to aarch64`() {
        withAbis("riscv64") {
            assertEquals("aarch64", detectArchFromAbi())
        }
    }

    @Test
    fun `empty abi list falls back to aarch64`() {
        withAbis { assertEquals("aarch64", detectArchFromAbi()) }
    }
}
