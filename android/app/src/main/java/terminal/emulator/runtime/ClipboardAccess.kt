package terminal.emulator.runtime

import android.content.ClipData
import android.content.ClipboardManager
import android.content.Context
import android.util.Log

/**
 * Single clipboard access point (K2: round-2 architecture).
 *
 * All clipboard reads/writes in the app previously went through 5 ad-hoc
 * `getSystemService` calls and 2 parallel paste implementations. This
 * wrapper owns the ClipboardManager lookup, the nullability dance and the
 * "clipboard service not available" log so callers get a simple
 * `clipboardText()` / `setClipboardText()` pair.
 */
class ClipboardAccess(
    private val context: Context,
    private val tag: String = "ClipboardAccess",
) {
    private fun manager(): ClipboardManager? {
        val manager = context.getSystemService(Context.CLIPBOARD_SERVICE) as? ClipboardManager
        if (manager == null) {
            Log.w(tag, "Clipboard service not available")
        }
        return manager
    }

    /** Current primary clip text, or null when unavailable/empty. */
    fun clipboardText(): String? {
        val clipboard = manager() ?: return null
        if (!clipboard.hasPrimaryClip()) return null
        return clipboard.primaryClip?.getItemAt(0)?.text?.toString()
    }

    fun setClipboardText(text: String, label: String = "terminal clipboard") {
        val clipboard = manager() ?: return
        clipboard.setPrimaryClip(ClipData.newPlainText(label, text))
    }

    /** True when a primary clip exists (safe against dead-clipboard
     *  exceptions; used by paste-button enablement). */
    fun hasClipboardText(): Boolean {
        val clipboard = manager() ?: return false
        return try {
            clipboard.hasPrimaryClip()
        } catch (_: Exception) {
            false
        }
    }
}
