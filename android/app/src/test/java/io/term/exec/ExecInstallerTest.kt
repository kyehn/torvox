package io.term.exec

import android.content.Context
import org.junit.Assert.assertNotEquals
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.RuntimeEnvironment
import org.robolectric.annotation.Config

@RunWith(RobolectricTestRunner::class)
@Config(application = android.app.Application::class)
class ExecInstallerTest {
    @Test
    fun binDirReturnsExpectedPath() {
        val context: Context = RuntimeEnvironment.getApplication()
        val binDir = ExecInstaller.binDir(context)
        assertNotNull(binDir)
        assertTrue("binDir should end with app_bin: ${binDir.absolutePath}", binDir.absolutePath.endsWith("/app_bin"))
    }

    @Test
    fun binDirDoesNotUseFilesDir() {
        val context: Context = RuntimeEnvironment.getApplication()
        val binDir = ExecInstaller.binDir(context)
        assertNotEquals("binDir must not be under filesDir", context.filesDir, binDir.parentFile)
    }
}
