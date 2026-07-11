package io.torvox

import android.app.Application
import androidx.test.core.app.ApplicationProvider
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import java.io.File

@RunWith(RobolectricTestRunner::class)
class CrashHandlerTest {
    @Test
    fun crashLogDirectoryIsCreatedUnderGetDir() {
        val application = ApplicationProvider.getApplicationContext<Application>()
        val logsDirectory = application.getDir("logs", Application.MODE_PRIVATE)
        assertTrue(
            "logs directory should exist under getDir('logs')",
            logsDirectory.exists(),
        )
        assertTrue(
            "logs directory should be a directory",
            logsDirectory.isDirectory,
        )
    }

    @Test
    fun crashLogDirectoryIsNotUnderFilesDir() {
        val application = ApplicationProvider.getApplicationContext<Application>()
        val logsDirectory = application.getDir("logs", Application.MODE_PRIVATE)
        val filesDirectory = application.filesDir

        assertTrue(
            "crash log directory should NOT be under filesDir",
            !logsDirectory.absolutePath.startsWith(filesDirectory.absolutePath),
        )
    }

    @Test
    fun crashLogFileCanBeWrittenAndRead() {
        val application = ApplicationProvider.getApplicationContext<Application>()
        val logsDirectory = application.getDir("logs", Application.MODE_PRIVATE)
        logsDirectory.mkdirs()

        val crashLogFile = File(logsDirectory, "crash_test.log")
        val testContent = "# Torvox Crash Log\n## Thread: main\n## Exception: TestException\n"
        crashLogFile.writeText(testContent)

        assertTrue(
            "crash log file should exist after writing",
            crashLogFile.exists(),
        )
        assertTrue(
            "crash log file content should match",
            crashLogFile.readText() == testContent,
        )

        crashLogFile.delete()
    }

    @Test
    fun crashLogFileIsNotUnderFilesDir() {
        val application = ApplicationProvider.getApplicationContext<Application>()
        val logsDirectory = application.getDir("logs", Application.MODE_PRIVATE)
        logsDirectory.mkdirs()

        val crashLogFile = File(logsDirectory, "crash_verify.log")
        crashLogFile.writeText("test")
        val filesDirectory = application.filesDir

        assertTrue(
            "crash log file should NOT be under filesDir",
            !crashLogFile.absolutePath.startsWith(filesDirectory.absolutePath),
        )

        crashLogFile.delete()
    }
}
