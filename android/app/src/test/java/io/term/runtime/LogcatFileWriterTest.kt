package io.term.runtime

import android.content.Context
import android.content.ContextWrapper
import org.junit.After
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.RuntimeEnvironment
import org.robolectric.annotation.Config
import java.io.File

@RunWith(RobolectricTestRunner::class)
@Config(application = android.app.Application::class)
class LogcatFileWriterTest {
    private lateinit var context: Context

    private fun noExternalFilesContext(): Context = object : ContextWrapper(context) {
        override fun getExternalFilesDir(type: String?): File? = null
    }

    @Before
    fun setup() {
        context = RuntimeEnvironment.getApplication()
        LogcatFileWriter.resetForTest()
        // wrap to force fallback to getDir
        context = noExternalFilesContext()
        File(context.getDir("logs_root", Context.MODE_PRIVATE), "logs").deleteRecursively()
    }

    @After
    fun tearDown() {
        LogcatFileWriter.resetForTest()
    }

    @Test
    fun init_createsLogFileInGetDir() {
        LogcatFileWriter.init(context)
        val logFile = File(File(context.getDir("logs_root", Context.MODE_PRIVATE), "logs"), "debug.log")
        assertTrue("log file should exist after init: ${logFile.absolutePath}", logFile.exists())
    }

    @Test
    fun init_fallsBackToGetDirWhenExternalReturnsNull() {
        LogcatFileWriter.init(context)
        val logFile = File(File(context.getDir("logs_root", Context.MODE_PRIVATE), "logs"), "debug.log")
        assertTrue("log file should exist in getDir fallback: ${logFile.absolutePath}", logFile.exists())
    }

    @Test
    fun write_appendsLineToFile() {
        LogcatFileWriter.init(context)
        LogcatFileWriter.write("TestTag", "hello-world")
        LogcatFileWriter.flush()
        val logFile = File(File(context.getDir("logs_root", Context.MODE_PRIVATE), "logs"), "debug.log")
        val contents = logFile.readText()
        assertTrue("expected tag in log, got: $contents", contents.contains("TestTag"))
        assertTrue("expected message in log, got: $contents", contents.contains("hello-world"))
    }

    @Test
    fun write_appendsToExistingContent() {
        LogcatFileWriter.init(context)
        LogcatFileWriter.write("First", "alpha")
        LogcatFileWriter.write("Second", "beta")
        LogcatFileWriter.flush()
        val logFile = File(File(context.getDir("logs_root", Context.MODE_PRIVATE), "logs"), "debug.log")
        val contents = logFile.readText()
        assertTrue(contents.contains("alpha"))
        assertTrue(contents.contains("beta"))
    }

    @Test
    fun write_afterClose_isNoop() {
        LogcatFileWriter.init(context)
        LogcatFileWriter.write("Before", "kept")
        LogcatFileWriter.close()
        LogcatFileWriter.write("After", "dropped")
        val logFile = File(File(context.getDir("logs_root", Context.MODE_PRIVATE), "logs"), "debug.log")
        val contents = logFile.readText()
        assertTrue(contents.contains("kept"))
        assertFalse("expected no 'dropped' line, got: $contents", contents.contains("dropped"))
    }
}
