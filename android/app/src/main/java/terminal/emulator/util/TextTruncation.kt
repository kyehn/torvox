package terminal.emulator.util

import terminal.emulator.runtime.LogUtil

/** 截断告警的统一标签：粘贴、复制、分享与安装诊断共用。 */
private const val TRUNCATION_TAG = "TextTruncation"

/**
 * 按 [maxChars] 截断文本，保留**开头**一段（粘贴与安装诊断的口径）。
 *
 * 截断点落在代理对中间时多取一字符保住完整码点：孤立的代理项写进 PTY 或日志后
 * 会变成替换字符，等于在边界处凭空造出一个乱码。
 *
 * 只做截断，不解释内容：空白文本原样返回（空白选区是合法的复制/分享内容）。
 * 截断是降级动作而非正常流程，故一律记警告，不静默丢字。
 */
internal fun truncateTextKeepingHead(text: String, maxChars: Int): String {
    val limit = maxChars.coerceAtLeast(0)
    if (text.length <= limit) return text
    LogUtil.warn(TRUNCATION_TAG, "text too large (${text.length} chars), keeping first $limit")
    val truncated = text.take(limit)
    if (truncated.isNotEmpty() &&
        Character.isHighSurrogate(truncated.last()) &&
        Character.isLowSurrogate(text[limit])
    ) {
        return truncated + text[limit]
    }
    return truncated
}

/**
 * 按 [maxChars] 截断文本，保留**末尾**一段（复制与分享的口径，见 DESIGN「文本选择」节
 * 「复制内容超过 CLIPBOARD_TEXT_MAX_LENGTH 时仅取后段」）。
 *
 * 选区尾端是最新输出，用户要留下的正是这一段；截掉开头才不丢刚看到的内容。
 * 截断点落在代理对中间时多取一字符保住完整码点：起点若是低代理项，它的高代理项
 * 已被截掉，把起点前移一格把整对收回，MUST NOT 产出孤立的低代理项。
 */
internal fun truncateTextKeepingTail(text: String, maxChars: Int): String {
    val limit = maxChars.coerceAtLeast(0)
    if (text.length <= limit) return text
    LogUtil.warn(TRUNCATION_TAG, "text too large (${text.length} chars), keeping last $limit")
    val start = text.length - limit
    val keptStart = if (start in 1 until text.length && Character.isLowSurrogate(text[start])) start - 1 else start
    return text.substring(keptStart)
}
