package terminal.emulator.installer

import android.content.Context
import android.os.UserManager

/**
 * Guards bootstrap installation against secondary (non-primary) Android
 * users (round-231, termux-app TermuxInstaller pattern). The bootstrap
 * writes into the primary user's private app-data directory; a secondary
 * user cannot access it (silent SELinux denials, broken exec), so the
 * install must fail fast with a clear message instead of a mysterious
 * download/install error.
 *
 * The primary-user check is injectable so the logic is unit-testable on
 * the JVM without a real [UserManager].
 */
class UserGuard(
    private val isPrimaryUserCheck: () -> Boolean,
) {
    /**
     * @return null when the current user may install; otherwise a
     * user-facing error message describing the restriction.
     */
    fun primaryUserError(): String? = if (isPrimaryUserCheck()) null else PRIMARY_USER_ERROR_MESSAGE

    companion object {
        const val PRIMARY_USER_ERROR_MESSAGE =
            "Bootstrap can only be installed by the primary user (owner profile)"

        /**
         * Device default check: [UserManager.isSystemUser] (API 24+;
         * minSdk 33) with an UID-arithmetic fallback (userId = uid /
         * 100_000, a stable Android ABI contract) if the service is
         * unavailable.
         */
        fun fromContext(context: Context): UserGuard = UserGuard {
            val userManager = context.getSystemService(Context.USER_SERVICE) as? UserManager
            if (userManager != null) {
                userManager.isSystemUser
            } else {
                android.os.Process.myUid() / 100_000 == 0
            }
        }

        /** Context-free fallback used when no Context is available. */
        fun fromUid(): UserGuard = UserGuard { android.os.Process.myUid() / 100_000 == 0 }
    }
}
