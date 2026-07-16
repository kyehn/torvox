package io.torvox.installer

import android.system.Os
import io.mockk.every
import io.mockk.mockkStatic
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.test.StandardTestDispatcher
import kotlinx.coroutines.test.resetMain
import kotlinx.coroutines.test.runTest
import kotlinx.coroutines.test.setMain
import org.junit.After
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import java.io.File
import java.nio.file.Files

@OptIn(ExperimentalCoroutinesApi::class)
@RunWith(RobolectricTestRunner::class)
class SecondStageRunnerTest {
    private val testDispatcher = StandardTestDispatcher()
    private lateinit var prefixDir: File
    private lateinit var homeDir: File

    @Before
    fun setup() {
        Dispatchers.setMain(testDispatcher)
        prefixDir = Files.createTempDirectory("prefix").toFile()
        homeDir = Files.createTempDirectory("home").toFile()
        mockkStatic(Os::class)
        every { Os.symlink(any(), any()) } returns Unit
    }

    @After
    fun teardown() {
        Dispatchers.resetMain()
        prefixDir.deleteRecursively()
        homeDir.deleteRecursively()
    }

    @Test
    fun `missing lock file creates symlink and returns success`() = runTest(testDispatcher) {
        val runner = SecondStageRunner(prefixDir, homeDir)
        val result = runner.run()
        assertTrue(result.success)
        assertTrue(result.errors.isEmpty())
    }

    @Test
    fun `empty postinst dir returns success with no errors`() = runTest(testDispatcher) {
        val postinstDir = File(prefixDir, "var/lib/dpkg/info")
        postinstDir.mkdirs()

        val runner = SecondStageRunner(prefixDir, homeDir)
        val result = runner.run()
        assertTrue(result.success)
        assertTrue(result.errors.isEmpty())
    }

    @Test
    fun `lock file already exists returns success immediately`() = runTest(testDispatcher) {
        val lockFile = File(prefixDir, "bin/termux-bootstrap-second-stage.sh.lock")
        lockFile.parentFile?.mkdirs()
        lockFile.writeText("lock")

        val runner = SecondStageRunner(prefixDir, homeDir)
        val result = runner.run()
        assertTrue(result.success)
        assertTrue(result.errors.isEmpty())
    }
}
