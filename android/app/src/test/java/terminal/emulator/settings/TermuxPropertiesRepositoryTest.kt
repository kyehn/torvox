package terminal.emulator.settings

import android.content.Context
import androidx.test.core.app.ApplicationProvider
import kotlinx.coroutines.runBlocking
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import java.io.File

/**
 * 配置文件的**读取规则**：首个可读常规文件生效、符号链接跳过、超大文件跳过。
 *
 * 这两条是 `config.md` 明写的要求，也是防止「外部写进来的东西把应用带歪」的边界：
 * 文件由 `adb push`、SAF 等外部途径写入，链接与超大文件都不该被当成配置读进来。
 */
@RunWith(RobolectricTestRunner::class)
class TermuxPropertiesRepositoryTest {

    private val context = ApplicationProvider.getApplicationContext<Context>()

    private fun homeDirectory(): File = File(context.filesDir, "home")

    private fun writeConfig(relativePath: String, text: String): File {
        val file = File(homeDirectory(), relativePath)
        file.parentFile?.mkdirs()
        file.writeText(text)
        return file
    }

    private fun load(repository: TermuxPropertiesRepository) = runBlocking { repository.reloadNow() }

    @Test
    fun `the primary path is read`() {
        writeConfig(".termux/termux.properties", "fullscreen = true")
        val repository = TermuxPropertiesRepository(context)
        load(repository)
        assertTrue(repository.configuration.value.properties.fullscreen)
    }

    /** 符号链接不是常规文件：跳过它，而不是跟着链接把别处的文件读成配置。 */
    @Test
    fun `a symlinked config file is skipped`() {
        val target = writeConfig("elsewhere/termux.properties", "fullscreen = true")
        val link = File(homeDirectory(), ".termux/termux.properties")
        link.parentFile?.mkdirs()
        java.nio.file.Files.createSymbolicLink(link.toPath(), target.toPath())
        val repository = TermuxPropertiesRepository(context)
        load(repository)
        assertFalse(repository.configuration.value.properties.fullscreen)
        assertNull(repository.configuration.value.propertiesPath)
    }

    /** 超过 64 KiB 的文件按无效处理：配置是手写的小文件，大文件只会把内存吃满。 */
    @Test
    fun `an oversized config file is skipped`() {
        writeConfig(".termux/termux.properties", "fullscreen = true\n" + "#".repeat(64 * 1024))
        val repository = TermuxPropertiesRepository(context)
        load(repository)
        assertFalse(repository.configuration.value.properties.fullscreen)
        assertTrue(
            "超大文件必须留痕: ${repository.configuration.value.errors}",
            repository.configuration.value.errors.any { it.contains("larger than") },
        )
    }

    /** 文件不存在是正常状态：全部取默认值，不算错误。 */
    @Test
    fun `a missing config file yields the defaults without errors`() {
        val repository = TermuxPropertiesRepository(context)
        load(repository)
        assertEquals(TermuxProperties(), repository.configuration.value.properties)
        assertTrue(repository.configuration.value.errors.isEmpty())
    }

    /** 文件在但读不出：沿用上一次成功的结果，不许把覆盖打回全默认。 */
    @Test
    fun `an unreadable colors file keeps the last successful overrides`() {
        val file = writeConfig(".termux/colors.properties", "background = #112233")
        val repository = TermuxPropertiesRepository(context)
        load(repository)
        assertEquals(0x00112233, repository.configuration.value.colors?.background)

        file.writeText("background = #445566\n" + "#".repeat(64 * 1024))
        load(repository)
        assertEquals(
            "读失败必须保留上次覆盖: ${repository.configuration.value.errors}",
            0x00112233,
            repository.configuration.value.colors?.background,
        )
        assertTrue(
            "读失败必须留痕: ${repository.configuration.value.errors}",
            repository.configuration.value.errors.any { it.contains("larger than") },
        )
    }

    /** 文件被清空是「撤销覆盖」，必须生效而不是沿用上次结果。 */
    @Test
    fun `an emptied colors file clears the overrides`() {
        val file = writeConfig(".termux/colors.properties", "background = #112233")
        val repository = TermuxPropertiesRepository(context)
        load(repository)
        assertEquals(0x00112233, repository.configuration.value.colors?.background)

        file.writeText("# only a comment\n")
        load(repository)
        assertNull(repository.configuration.value.colors)
    }
}
