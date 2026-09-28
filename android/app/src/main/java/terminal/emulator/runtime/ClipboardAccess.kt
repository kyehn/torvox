package terminal.emulator.runtime

import android.annotation.SuppressLint
import android.content.ClipData
import android.content.ClipboardManager
import android.content.Context

/**
 * 全应用唯一的剪贴板访问点，封装 ClipboardManager 查找、可空处理
 * 与「剪贴板服务不可用」的日志，使调用方只需一对
 * `clipboardText()` / `setClipboardText()`。
 */
class ClipboardAccess(private val context: Context, private val tag: String = "ClipboardAccess") {
    private fun manager(): ClipboardManager? {
        val manager = context.getSystemService(Context.CLIPBOARD_SERVICE) as? ClipboardManager
        if (manager == null) {
            LogUtil.w(tag, "Clipboard service not available")
        }
        return manager
    }

    /** 当前主剪贴板文本，不可用或为空时为 null。 */
    @SuppressLint("DeprecatedCall")
    fun clipboardText(): String? {
        val clipboard = manager() ?: return null
        // hasPrimaryClip()/primaryClip：无替代方案的弃用 API（API 36），
        // 平台未提供其他同步存在性查询。slack-lint 在此也标记 getPrimaryClip，
        // 尽管它在 API 37 中没有 @Deprecated 注解（规则数据滞后）——保留同样的调用，
        // 以注释说明意图。
        if (!clipboard.hasPrimaryClip()) return null
        return clipboard.primaryClip?.getItemAt(0)?.text?.toString()
    }

    @SuppressLint("DeprecatedCall")
    fun setClipboardText(text: String, label: String = "terminal clipboard") {
        val clipboard = manager() ?: return
        // setPrimaryClip()：无替代方案的弃用 API（API 36）。
        clipboard.setPrimaryClip(ClipData.newPlainText(label, text))
    }

    /** 是否存在主剪贴板（能防范剪贴板服务死掉时的异常；用于粘贴按钮的可用性判断）。 */
    @SuppressLint("DeprecatedCall")
    fun hasClipboardText(): Boolean {
        val clipboard = manager() ?: return false
        return try {
            // hasPrimaryClip()：无替代方案的弃用 API（API 36）。
            clipboard.hasPrimaryClip()
        } catch (_: Exception) {
            false
        }
    }
}
