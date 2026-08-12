package terminal.emulator.ui

/**
 * Schedules a delayed unit of work with cancellation support, abstracted
 * so debounce logic can be unit-tested on the JVM without a Looper.
 */
interface DebounceScheduler {
    /** Run [action] after [delayMillis], replacing any pending scheduled action. */
    fun postDelayed(delayMillis: Long, action: () -> Unit)

    /** Drop the currently scheduled action, if any. */
    fun cancelPending()
}

/**
 * [DebounceScheduler] backed by a main-thread [android.os.Handler]; used
 * in production Compose code. Not exercised by JVM unit tests (they use
 * a fake scheduler).
 */
class HandlerDebounceScheduler(
    private val handler: android.os.Handler,
) : DebounceScheduler {
    private var pendingRunnable: Runnable? = null

    override fun postDelayed(delayMillis: Long, action: () -> Unit) {
        val runnable = Runnable { action() }
        pendingRunnable = runnable
        handler.postDelayed(runnable, delayMillis)
    }

    override fun cancelPending() {
        pendingRunnable?.let { handler.removeCallbacks(it) }
        pendingRunnable = null
    }
}

/**
 * Debounces rapid successive [submit] calls: only the action submitted
 * last within [debounceMillis] actually runs, once, after the quiet
 * period. [flush] cancels the pending action and runs it immediately —
 * used by the IME Search action so pressing enter searches without
 * waiting out the debounce.
 *
 * Pure Kotlin (no Android dependencies): unit-tested on the JVM with a
 * fake [DebounceScheduler].
 */
class SearchDebouncer(
    private val debounceMillis: Long,
    private val scheduler: DebounceScheduler,
) {
    private var pendingAction: (() -> Unit)? = null

    /** Schedule [action]; a previous pending action is replaced, not run. */
    fun submit(action: () -> Unit) {
        pendingAction = action
        scheduler.cancelPending()
        scheduler.postDelayed(debounceMillis) {
            // Identity check: a stale scheduled runnable whose action was
            // superseded by a later submit must not run the new action.
            if (pendingAction === action) {
                pendingAction = null
                action()
            }
        }
    }

    /**
     * Run the pending action immediately, if any.
     * @return true when an action was flushed, false when nothing was pending.
     */
    fun flush(): Boolean {
        val action = pendingAction ?: return false
        pendingAction = null
        scheduler.cancelPending()
        action()
        return true
    }

    /** Drop the pending action without running it. */
    fun cancel() {
        pendingAction = null
        scheduler.cancelPending()
    }
}
