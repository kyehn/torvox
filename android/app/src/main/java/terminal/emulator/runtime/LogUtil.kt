package terminal.emulator.runtime

import android.util.Log
import terminal.emulator.BuildConfig

/**
 * 仅写入 logcat 的日志器，对标 termux-kotlin 的 Logger。
 *
 * 长消息被切分，使任何单条 logcat 条目都不超过平台负载上限（4068 字节）
 * ——超出部分会被 logcat 静默截断，丢失消息尾部。
 * 切分算法与原生 [`log_chunk`] 模块一致：`maxEntrySize = 4068 - 32 - tagLen - 4`
 * （32 字节 = logd 的每条头部，实测所得），续块带 `(i/n)` 前缀。
 */
object LogUtil {
    fun d(tag: String, message: String, throwable: Throwable? = null) {
        // Logcat 在 debug 构建中仍由 DEBUG 门控。
        if (BuildConfig.DEBUG) {
            logChunked(Log.DEBUG, tag, message, throwable)
        }
    }

    fun i(tag: String, message: String, throwable: Throwable? = null) {
        logChunked(Log.INFO, tag, message, throwable)
    }

    fun w(tag: String, message: String, throwable: Throwable? = null) {
        logChunked(Log.WARN, tag, message, throwable)
    }

    fun e(tag: String, message: String, throwable: Throwable? = null) {
        logChunked(Log.ERROR, tag, message, throwable)
    }

    /**
     * 以 VERBOSE 优先级记录敏感数据（如用户输入），前缀为 `[PRIVATE]`。
     * 仅在 DEBUG 构建中生效——release 构建中为空操作，机密绝不会抵达该处的 logcat。
     */
    fun logPrivate(tag: String, message: String, throwable: Throwable? = null) {
        if (BuildConfig.DEBUG) {
            logChunked(Log.VERBOSE, tag, "[PRIVATE] $message", throwable)
        }
    }

    private fun logChunked(priority: Int, tag: String, message: String, throwable: Throwable?) {
        for (chunk in chunkMessage(tag, message)) {
            Log.println(priority, tag, chunk)
        }
        if (throwable != null) {
            // 异常单独作为一条记录，使堆栈不与切分后的消息片段交错。
            Log.println(priority, tag, throwable.toString())
            for (line in throwable.stackTrace) {
                Log.println(priority, tag, "    at $line")
            }
        }
    }

    /**
     * 把 [message] 切分为 logcat 大小的块。暴露供单元测试，对标原生
     * `log_chunk::chunk_message`。
     *
     * 预算以 UTF-8 字节计（logcat 计字节而非 UTF-16 码元），
     * 且切分只落在码点边界，使多字节 CJK 字符与 emoji 代理对绝不被切开。
     */
    internal fun chunkMessage(tag: String, message: String): List<String> {
        val budget = maxEntrySize(tag.toByteArray(Charsets.UTF_8).size)
        if (utf8Length(message) <= budget) {
            return listOf(message)
        }
        // 续块前缀会占用预算。「(N/N)\n」在 N >= 100 时超过 8 字节；
        // 先按 8 估算，若变长则用真实前缀长度重新切分。
        var prefixLen = 8
        var chunks = splitIntoChunks(message, budget, prefixLen)
        if (chunks.size > 1) {
            // 反复重切直到「(N/N)\n」前缀长度收敛（N >= 100 时超过 8 字节；
            // 极大 N 下重切后可能再次变长）。
            while (chunks.size > 1) {
                val actualPrefixLen = "(${chunks.size}/${chunks.size})\n".length
                if (actualPrefixLen <= prefixLen) break
                prefixLen = actualPrefixLen
                chunks = splitIntoChunks(message, budget, prefixLen)
            }
            val total = chunks.size
            for (i in 1 until total) {
                chunks[i] = "(${i + 1}/$total)\n${chunks[i]}"
            }
        }
        return chunks
    }

    /** 把 [message] 切分为 UTF-8 字节长度符合 `budget - prefixLen` 的块，优先在换行处切开。 */
    private fun splitIntoChunks(message: String, budget: Int, prefixLen: Int): MutableList<String> {
        val effective = (budget - prefixLen).coerceAtLeast(16)
        val chunks = mutableListOf<String>()
        var start = 0
        while (start < message.length) {
            var end = start
            var windowBytes = 0
            while (end < message.length) {
                val cp = message.codePointAt(end)
                val cpBytes = utf8CharLength(cp)
                if (windowBytes + cpBytes > effective) {
                    break
                }
                windowBytes += cpBytes
                end += Character.charCount(cp)
            }
            if (end == start) {
                // 病态情形：单个码点比整个窗口还长。仍然包含它以保证推进。
                val cp = message.codePointAt(start)
                end = start + Character.charCount(cp)
                windowBytes = utf8CharLength(cp)
            }
            // 优先在窗口内最后一个换行处切开，使多行消息保持整行。
            if (end < message.length) {
                val lastNl = message.lastIndexOf('\n', end - 1)
                if (lastNl > start && lastNl < end) {
                    end = lastNl + 1
                    windowBytes = utf8Length(message.substring(start, end))
                }
            }
            chunks.add(message.substring(start, end))
            start = end
        }
        return chunks
    }

    /** [value] 占用的 UTF-8 字节数。 */
    private fun utf8Length(value: String): Int {
        var length = 0
        var i = 0
        while (i < value.length) {
            length += utf8CharLength(value.codePointAt(i))
            i += Character.charCount(value.codePointAt(i))
        }
        return length
    }

    /** 单个码点占用的 UTF-8 字节数。 */
    private fun utf8CharLength(codePoint: Int): Int = when {
        codePoint < 0x80 -> 1
        codePoint < 0x800 -> 2
        codePoint < 0x10000 -> 3
        else -> 4
    }

    internal fun maxEntrySize(tagLength: Int): Int {
        val budget =
            LOGGER_ENTRY_MAX_PAYLOAD - LOGGER_PREFIX_OVERHEAD - tagLength - LOGGER_SAFETY_MARGIN
        return budget.coerceAtLeast(64)
    }

    private const val LOGGER_ENTRY_MAX_PAYLOAD = 4068

    // logd 的每条头部（logger_entry 结构 + tag 长度字段）。
    // 实测：4036 字节负载配 12 字节 tag 时在 4022 字节处被截断
    // ——旧的 8 字节估算会让 logd 静默切掉块。
    private const val LOGGER_PREFIX_OVERHEAD = 32
    private const val LOGGER_SAFETY_MARGIN = 4
}
