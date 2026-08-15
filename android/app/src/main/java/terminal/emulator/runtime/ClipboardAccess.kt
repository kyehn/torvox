package terminal.emulator.runtime

import android.annotation.SuppressLint
import android.content.ClipData
import android.content.ClipboardManager
import android.content.Context
import android.util.Log

/**
 * Single clipboard access point (K2:  architecture).
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
    /**
     * Optional smart-copy transformation applied on every
     * [setClipboardText] write, Haven
     * SmartTerminalClipboard:407-430): the terminal selection copy path
     * installs a border-strip / URL-rebuild processor; OSC 52 programmatic
     * writes keep the default null → verbatim passthrough. When the
     * processor returns null/blank the caller's text is kept instead of
     * clobbering the clipboard (Haven's drift guard).
     */
    var smartCopyProcessor: ((text: String) -> String?)? = null

    private fun manager(): ClipboardManager? {
        val manager = context.getSystemService(Context.CLIPBOARD_SERVICE) as? ClipboardManager
        if (manager == null) {
            Log.w(tag, "Clipboard service not available")
        }
        return manager
    }

    /** Current primary clip text, or null when unavailable/empty. */
    @SuppressLint("DeprecatedCall")
    fun clipboardText(): String? {
        val clipboard = manager() ?: return null
        // hasPrimaryClip()/primaryClip: deprecated without replacement
        // (API 36); the platform exposes no other synchronous existence
        // query. slack-lint also flags getPrimaryClip here although it has
        // no @Deprecated annotation in API 37 (rule data lag) — same calls
        // stay, comments document intent.
        if (!clipboard.hasPrimaryClip()) return null
        return clipboard.primaryClip?.getItemAt(0)?.text?.toString()
    }

    @SuppressLint("DeprecatedCall")
    fun setClipboardText(text: String, label: String = "terminal clipboard") {
        val clipboard = manager() ?: return
        val processed =
            smartCopyProcessor?.invoke(text)
                ?.takeIf { it.isNotBlank() }
                ?: text
        // setPrimaryClip(): deprecated without replacement (API 36).
        clipboard.setPrimaryClip(ClipData.newPlainText(label, processed))
    }

    /** True when a primary clip exists (safe against dead-clipboard
     *  exceptions; used by paste-button enablement). */
    @SuppressLint("DeprecatedCall")
    fun hasClipboardText(): Boolean {
        val clipboard = manager() ?: return false
        return try {
            // hasPrimaryClip(): deprecated without replacement (API 36).
            clipboard.hasPrimaryClip()
        } catch (_: Exception) {
            false
        }
    }
}
