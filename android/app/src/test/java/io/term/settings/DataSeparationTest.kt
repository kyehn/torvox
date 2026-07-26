package io.term.settings

import android.content.Context
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.test.StandardTestDispatcher
import kotlinx.coroutines.test.resetMain
import kotlinx.coroutines.test.runTest
import kotlinx.coroutines.test.setMain
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

@OptIn(ExperimentalCoroutinesApi::class)
@RunWith(RobolectricTestRunner::class)
@Config(application = android.app.Application::class)
class DataSeparationTest {
    private val testDispatcher = StandardTestDispatcher()

    @Before
    fun setup() {
        Dispatchers.setMain(testDispatcher)
    }

    @After
    fun tearDown() {
        Dispatchers.resetMain()
    }

    @Test
    fun dataStoreFileNotUnderFilesDir() {
        val context = RuntimeEnvironment.getApplication()
        val provider = SettingsDataStoreProvider(context)

        val filesDir = context.filesDir
        val dataStoreFile = File(provider.prefsDir, "settings.preferences_pb")

        assertFalse(
            "DataStore file must NOT be under files/ (user directory). " +
                "filesDir=$filesDir, dataStoreFile=$dataStoreFile",
            dataStoreFile.absolutePath.startsWith(filesDir.absolutePath),
        )
    }

    @Test
    fun dataStoreFileUnderAppPrefsDir() {
        val context = RuntimeEnvironment.getApplication()
        val provider = SettingsDataStoreProvider(context)

        val expectedDir = context.getDir("prefs", Context.MODE_PRIVATE)
        val actualDir = provider.prefsDir

        assertTrue(
            "DataStore prefsDir must be app_prefs/ under getDir(), " +
                "expected=$expectedDir, actual=$actualDir",
            actualDir.absolutePath == expectedDir.absolutePath,
        )
    }

    @Test
    fun sessionSavePathNotUnderFilesDir() {
        val context = RuntimeEnvironment.getApplication()
        val filesDir = context.filesDir
        val sessionsDir = context.getDir("sessions", Context.MODE_PRIVATE)

        assertFalse(
            "Sessions directory must NOT be under files/ (user directory). " +
                "filesDir=$filesDir, sessionsDir=$sessionsDir",
            sessionsDir.absolutePath.startsWith(filesDir.absolutePath),
        )
    }

    @Test
    fun logsDirNotUnderFilesDir() {
        val context = RuntimeEnvironment.getApplication()
        val filesDir = context.filesDir
        val logsDir = context.getDir("logs", Context.MODE_PRIVATE)

        assertFalse(
            "Logs directory must NOT be under files/ (user directory). " +
                "filesDir=$filesDir, logsDir=$logsDir",
            logsDir.absolutePath.startsWith(filesDir.absolutePath),
        )
    }

    @Test
    fun binDirNotUnderFilesDir() {
        val context = RuntimeEnvironment.getApplication()
        val filesDir = context.filesDir
        val binDir = context.getDir("bin", Context.MODE_PRIVATE)

        assertFalse(
            "Bin directory must NOT be under files/ (user directory). " +
                "filesDir=$filesDir, binDir=$binDir",
            binDir.absolutePath.startsWith(filesDir.absolutePath),
        )
    }

    @Test
    fun allAppDataDirsAreUnderGetDir() {
        val context = RuntimeEnvironment.getApplication()
        val expectedDirs = listOf("prefs", "sessions", "logs", "bin")

        for (dirName in expectedDirs) {
            val dir = context.getDir(dirName, Context.MODE_PRIVATE)
            assertTrue(
                "getDir('$dirName') must exist and be a directory: $dir",
                dir.exists() && dir.isDirectory,
            )
        }
    }

    @Test
    fun settingsPersistAcrossNewRepositoryInstance() = runTest {
        val context = RuntimeEnvironment.getApplication()
        val provider = SettingsDataStoreProvider(context)
        val repositoryOne = SettingsRepository(provider)
        val repositoryTwo = SettingsRepository(provider)

        repositoryOne.setFontSize(42f)
        val fontSize = repositoryTwo.fontSize.first()
        assertTrue(
            "Settings must persist across repository instances using the same provider (got $fontSize, expected 42f)",
            fontSize == 42f,
        )
    }

    @Test
    fun clearAppDataDirsExist() {
        val context = RuntimeEnvironment.getApplication()
        val dirs = listOf("prefs", "sessions", "logs", "bin")

        for (dirName in dirs) {
            val dir = context.getDir(dirName, Context.MODE_PRIVATE)
            assertTrue(
                "getDir('$dirName') must exist: ${dir.absolutePath}",
                dir.exists(),
            )
        }
    }
}
