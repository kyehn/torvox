package terminal.emulator.runtime

import android.util.Log
import terminal.emulator.BuildConfig

/**
 * Logcat-only logger, mirrors termux-kotlin Logger).
 *
 * Long messages are chunked so no single logcat entry exceeds the
 * platform payload cap (4068 bytes) — logcat silently truncates anything
 * past that, losing the tail of the message. The chunk math matches the
 * native [`log_chunk`] module: `maxEntrySize = 4068 - 32 - tagLen - 4`
 * (32 bytes = logd per-entry header, measured on-device),
 * continuation chunks carry a `(i/n)` prefix.
 *
 * The previous file sink (`LogcatFileWriter`) is removed: logs go to
 * logcat only, exactly like termux-kotlin's Logger.
 */
object LogUtil {
    fun d(
        tag: String,
        message: String,
        throwable: Throwable? = null,
    ) {
        // Logcat remains gated by DEBUG in debug builds.
        if (BuildConfig.DEBUG) {
            logChunked(Log.DEBUG, tag, message, throwable)
        }
    }

    fun i(
        tag: String,
        message: String,
        throwable: Throwable? = null,
    ) {
        logChunked(Log.INFO, tag, message, throwable)
    }

    fun w(
        tag: String,
        message: String,
        throwable: Throwable? = null,
    ) {
        logChunked(Log.WARN, tag, message, throwable)
    }

    fun e(
        tag: String,
        message: String,
        throwable: Throwable? = null,
    ) {
        logChunked(Log.ERROR, tag, message, throwable)
    }

    /**
     * Log sensitive data (e.g. MCP payloads, user input) at VERBOSE
     * priority with a `[PRIVATE]` prefix. Only active in DEBUG builds —
     * no-op in release builds, so secrets never reach logcat there.
     */
    fun logPrivate(
        tag: String,
        message: String,
        throwable: Throwable? = null,
    ) {
        if (BuildConfig.DEBUG) {
            logChunked(Log.VERBOSE, tag, "[PRIVATE] $message", throwable)
        }
    }

    private fun logChunked(
        priority: Int,
        tag: String,
        message: String,
        throwable: Throwable?,
    ) {
        for (chunk in chunkMessage(tag, message)) {
            Log.println(priority, tag, chunk)
        }
        if (throwable != null) {
            // Log the throwable once, on its own entry, so the stack trace
            // is not interleaved with chunked message parts.
            Log.println(priority, tag, throwable.toString())
            for (line in throwable.stackTrace) {
                Log.println(priority, tag, "    at $line")
            }
        }
    }

    /**
     * Split [message] into logcat-sized chunks. Exposed for unit tests.
     * Mirrors native `log_chunk::chunk_message`.
     *
     * The budget is measured in UTF-8 bytes (logcat counts bytes, not
     * UTF-16 code units) and chunks are only ever cut at code point
     * boundaries, so multi-byte CJK characters and emoji surrogate pairs
     * are never split audit fix).
     */
    internal fun chunkMessage(tag: String, message: String): List<String> {
        val budget = maxEntrySize(tag.toByteArray(Charsets.UTF_8).size)
        if (utf8Length(message) <= budget) {
            return listOf(message)
        }
        // Continuation prefixes eat into the budget. "(N/N)\n" grows past
        // 8 bytes once N >= 100; estimate with 8 and re-split with the
        // real prefix length if it grew.
        var prefixLen = 8
        var chunks = splitIntoChunks(message, budget, prefixLen)
        if (chunks.size > 1) {
            // Re-split until the "(N/N)\n" prefix length converges (it
            // grows past 8 bytes once N >= 100; for absurdly large N it
            // could grow again after re-splitting).
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

    /** Split [message] into chunks whose UTF-8 byte length fits
     *  `budget - prefixLen`, preferring cuts at newlines. */
    private fun splitIntoChunks(
        message: String,
        budget: Int,
        prefixLen: Int,
    ): MutableList<String> {
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
                // Pathological: a single code point longer than the whole
                // window. Include it anyway so we make progress.
                val cp = message.codePointAt(start)
                end = start + Character.charCount(cp)
                windowBytes = utf8CharLength(cp)
            }
            // Prefer cutting at the last newline inside the window so
            // multi-line messages keep their lines intact.
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

    /** Number of UTF-8 bytes [value] occupies. */
    private fun utf8Length(value: String): Int {
        var length = 0
        var i = 0
        while (i < value.length) {
            length += utf8CharLength(value.codePointAt(i))
            i += Character.charCount(value.codePointAt(i))
        }
        return length
    }

    /** Number of UTF-8 bytes a single code point occupies. */
    private fun utf8CharLength(codePoint: Int): Int = when {
        codePoint < 0x80 -> 1
        codePoint < 0x800 -> 2
        codePoint < 0x10000 -> 3
        else -> 4
    }

    internal fun maxEntrySize(tagLength: Int): Int {
        val budget = LOGGER_ENTRY_MAX_PAYLOAD - LOGGER_PREFIX_OVERHEAD - tagLength - LOGGER_SAFETY_MARGIN
        return budget.coerceAtLeast(64)
    }

    private const val LOGGER_ENTRY_MAX_PAYLOAD = 4068

    // logd per-entry header (logger_entry struct + tag length field).
    // Measured on-device: a 4036-byte payload with a
    // 12-byte tag surfaces truncated at 4022 bytes — the old 8-byte
    // estimate let logd silently cut chunks.
    private const val LOGGER_PREFIX_OVERHEAD = 32
    private const val LOGGER_SAFETY_MARGIN = 4
}
