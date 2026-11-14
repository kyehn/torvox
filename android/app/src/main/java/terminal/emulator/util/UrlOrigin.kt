package terminal.emulator.util

import androidx.core.net.toUri

/**
 * URL 的来源部分（`scheme://host`），供日志使用。
 *
 * 完整 URL 可能带 token 与查询参数，logcat 无差别记录；来源部分足以定位来源。
 * 无法解析时给占位串，MUST NOT 把原串回填——否则「不记完整 URL」就形同虚设。
 */
internal fun urlOrigin(url: String): String = runCatchingCancellable {
    val uri = url.toUri()
    val scheme = uri.scheme ?: return@runCatchingCancellable "<no-scheme>"
    val host = uri.host
    if (host.isNullOrBlank()) return@runCatchingCancellable "<no-host>"
    "$scheme://$host"
}.getOrNull() ?: "<unparsable>"
