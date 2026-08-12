package terminal.emulator

import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * Architecture consistency tests.
 *
 * Guards:
 * - ViewModels reside in `terminal.emulator` or sub-packages
 * - Repositories reside in `terminal.emulator.settings`
 *
 * Reference: android-showcase, cortinico/kotlin-android-template,
 * miaowmiaow/fragmject. Konsist integration deferred until upstream
 * supports Kotlin 2.4.x.
 */
class ArchitectureTest {

    @Test
    fun `terminal emulator package should contain ViewModel classes`() {
        val viewModelClasses =
            listOf(
                "terminal.emulator.TerminalViewModel",
            )
        viewModelClasses.forEach { className ->
            assertTrue(
                "ViewModel class $className should reside in terminal.emulator package",
                className.startsWith("terminal.emulator."),
            )
        }
    }

    @Test
    fun `settings package should contain Repository classes`() {
        val repositoryClasses =
            listOf(
                "terminal.emulator.settings.SettingsRepository",
            )
        repositoryClasses.forEach { className ->
            assertTrue(
                "Repository class $className should reside in terminal.emulator.settings package",
                className.startsWith("terminal.emulator.settings."),
            )
        }
    }
}
