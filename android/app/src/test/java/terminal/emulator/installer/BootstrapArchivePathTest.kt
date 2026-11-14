package terminal.emulator.installer

import androidx.test.core.app.ApplicationProvider
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import java.io.File

/**
 * 引导归档来源路径的边界用例。
 *
 * 外部 Intent 能指定的路径只在应用私有目录内才可信：归档内容会被写进
 * `files/usr` 并以 shell 身份执行，放开任意路径等于让任何应用借本应用
 * 安装自己准备的二进制。
 */
@RunWith(RobolectricTestRunner::class)
class BootstrapArchivePathTest {
    private val context = ApplicationProvider.getApplicationContext<android.content.Context>()
    private val roots = listOf(context.filesDir, context.cacheDir)

    @Test
    fun `archive inside the app private dirs is allowed`() {
        assertTrue(isInsideAppPrivateDir(File(context.cacheDir, "bootstrap.zip"), roots))
        assertTrue(isInsideAppPrivateDir(File(context.filesDir, "bootstrap.zip"), roots))
    }

    @Test
    fun `archive outside the app private dirs is rejected`() {
        val outside = File(context.filesDir.parentFile, "outside.zip")
        assertFalse(isInsideAppPrivateDir(outside, roots))
    }

    @Test
    fun `dot dot escape out of the app private dirs is rejected`() {
        val escape = File(context.cacheDir, "../escaped.zip")
        assertFalse(isInsideAppPrivateDir(escape, roots))
    }

    @Test
    fun `sibling directory sharing the prefix is rejected`() {
        // 字符串前缀会把 `files-other` 误判成 `files` 之下：判据必须按路径元素比较。
        val sibling = File(context.filesDir.path + "-other/bootstrap.zip")
        assertFalse(isInsideAppPrivateDir(sibling, roots))
    }
}
