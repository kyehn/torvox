package terminal.emulator

import org.junit.Assert
import org.junit.Assert.assertEquals
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
        }
    }

    @Test
    fun `x86_64 maps to x86_64 and is 64-bit`() {
        withAbis("x86_64") {
            assertEquals("x86_64", detectArchFromAbi())
        }
    }

    @Test
    fun `unsupported abi fails fast instead of fetching the wrong arch`() {
        withAbis("riscv64") {
            try {
                detectArchFromAbi()
                Assert.fail("riscv64 必须抛而非回退 aarch64")
            } catch (exception: IllegalStateException) {
                Assert.assertTrue(exception.message!!.contains("riscv64"))
            }
        }
    }

    @Test
    fun `empty abi list fails fast`() {
        withAbis {
            try {
                detectArchFromAbi()
                Assert.fail("空 ABI 列表必须抛")
            } catch (exception: IllegalStateException) {
                // error() 即 IllegalStateException
            }
        }
    }
}
