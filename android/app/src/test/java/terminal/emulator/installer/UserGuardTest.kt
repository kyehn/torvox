package terminal.emulator.installer

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test

/**
 * UserGuard gates bootstrap installs to the primary Android user. The
 * check is injected, so the JVM test pins both branches without a real
 * UserManager.
 */
class UserGuardTest {

    @Test
    fun `primary user allows install`() {
        val guard = UserGuard { true }
        assertNull(guard.primaryUserError())
    }

    @Test
    fun `secondary user blocks install with clear message`() {
        val guard = UserGuard { false }
        assertEquals(
            UserGuard.PRIMARY_USER_ERROR_MESSAGE,
            guard.primaryUserError(),
        )
    }

    @Test
    fun `error message is stable and user-facing`() {
        assertEquals(
            "Bootstrap can only be installed by the primary user (owner profile)",
            UserGuard.PRIMARY_USER_ERROR_MESSAGE,
        )
    }
}
