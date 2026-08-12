package terminal.emulator

import com.lemonappdev.konsist.api.Konsist
import com.lemonappdev.konsist.api.ext.list.classes
import com.lemonappdev.konsist.api.ext.list.withNameEndingWith
import com.lemonappdev.konsist.api.verify.assertTrue
import org.junit.Test

/**
 * Architecture consistency tests (Konsist 0.17.3 — uses `assertTrue {}`,
 * the `assert {}` lambda API was introduced in a later version).
 *
 * Guards:
 * - `*ViewModel` classes reside in `terminal.emulator` or sub-packages
 * - `*Repository` classes reside in `terminal.emulator.settings` or
 *   `terminal.emulator`
 *
 * Reference: android-showcase, cortinico/kotlin-android-template,
 * miaowmiaow/fragmject.
 */
class ArchitectureTest {

    @Test
    fun `ViewModel classes reside in terminal emulator package`() {
        Konsist.scopeFromProject()
            .classes()
            .withNameEndingWith("ViewModel")
            .assertTrue { it.resideInPackage("terminal.emulator..") }
    }

    @Test
    fun `Repository classes reside in settings or terminal emulator package`() {
        Konsist.scopeFromProject()
            .classes()
            .withNameEndingWith("Repository")
            .assertTrue {
                it.resideInPackage("terminal.emulator.settings..") ||
                    it.resideInPackage("terminal.emulator..")
            }
    }
}
