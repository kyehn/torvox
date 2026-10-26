package terminal.emulator.runtime

import android.annotation.SuppressLint
import android.content.ClipData
import android.content.ClipboardManager
import android.content.Context
import terminal.emulator.util.runCatchingCancellable

/**
 * 全应用唯一的剪贴板访问点，封装 ClipboardManager 查找、可空处理
 * 与「剪贴板服务不可用」的日志，使调用方只需一对
 * `clipboardText()` / `setClipboardText()`。
 */
class ClipboardAccess(private val context: Context, private val tag: String = "ClipboardAccess") {
    private fun manager(): ClipboardManager? {
        val manager = context.getSystemService(Context.CLIPBOARD_SERVICE) as? ClipboardManager
        if (manager == null) {
            LogUtil.warn(tag, "Clipboard service not available")
        }
        return manager
    }

    /**
     * 当前主剪贴板文本：无文本内容为 `Ok(null)`（空剪贴板是合法状态）。
     *
     * 剪贴板服务不可用或读取抛异常为失败——必须与空剪贴板可区分：
     * OSC 52 读取应答把两者都写成空串会让远端 tmux/ssh 粘出空白，
     * 且看上去像是用户清空了剪贴板。
     */
    @SuppressLint("DeprecatedCall")
    fun clipboardText(): Result<String?> = runCatchingCancellable {
        val clipboard = manager() ?: error("clipboard service unavailable")
        // hasPrimaryClip()/primaryClip：无替代方案的弃用 API（API 36），
        // 平台未提供其他同步存在性查询。slack-lint 在此也标记 getPrimaryClip，
        // 尽管它在 API 37 中没有 @Deprecated 注解（规则数据滞后）——保留同样的调用，
        // 以注释说明意图。
        if (!clipboard.hasPrimaryClip()) {
            null
        } else {
            clipboard.primaryClip
                ?.getItemAt(0)
                ?.text
                ?.toString()
        }
    }.onFailure { failure ->
        // 只记异常类名：异常消息可能嵌入剪贴板文本。
        LogUtil.error(tag, "剪贴板读取失败: ${failure.javaClass.simpleName}")
    }

    @SuppressLint("DeprecatedCall")
    fun setClipboardText(text: String, label: String = "terminal clipboard") {
        val clipboard = manager() ?: return
        // setPrimaryClip()：无替代方案的弃用 API（API 36）。
        clipboard.setPrimaryClip(ClipData.newPlainText(label, text))
    }

    /**
     * 剪贴板里是否有可粘贴的文本；无主剪贴板、非文本项（URI/Intent 剪贴）或读取失败
     * 均为 false。复用 [clipboardText]：粘贴菜单据此决定是否提供 PASTE，口径与真正
     * 粘贴出去的必须是同一份内容。
     */
    fun hasClipboardText(): Boolean = clipboardText().getOrNull() != null
}
