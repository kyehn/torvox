package terminal.emulator.util

/*
 * Port of DrJava's ArgumentTokenizer (BSD-2, Rice University JavaPLT),
 * via termux-kotlin-app's `termux-shared/shell/ArgumentTokenizer.kt`
 * (itself a 1:1 Kotlin port). Splits a user-visible command string into a
 * safe argv array WITHOUT `sh -c` semantics: no variable expansion, no
 * `;` / `|` / `&&` / redirection, no globbing — only quoting and escaping.
 *
 * Rules (four-state machine):
 *  - SINGLE_QUOTE: everything is literal, no escapes at all.
 *  - DOUBLE_QUOTE: only `\"` and `\\` escape; `\n` stays as literal
 *    backslash-n (POSIX divergence, kept for DrJava compatibility).
 *  - Backslash outside quotes escapes the next character.
 *  - Unclosed quote is silently kept as part of the final token.
 */
object ArgumentTokenizer {
    private const val NO_TOKEN_STATE = 0
    private const val NORMAL_TOKEN_STATE = 1
    private const val SINGLE_QUOTE_STATE = 2
    private const val DOUBLE_QUOTE_STATE = 3

    fun tokenize(arguments: String): List<String> = tokenize(arguments, false)

    fun tokenize(
        arguments: String,
        stringify: Boolean,
    ): List<String> {
        var currArg = StringBuilder()
        var escaped = false
        var state = NO_TOKEN_STATE
        val argList = mutableListOf<String>()

        var i = 0
        while (i < arguments.length) {
            val c = arguments[i]
            when {
                escaped -> {
                    currArg.append(c)
                    escaped = false
                }
                state == SINGLE_QUOTE_STATE -> {
                    if (c == '\'') {
                        state = NORMAL_TOKEN_STATE
                    } else {
                        currArg.append(c)
                    }
                }
                state == DOUBLE_QUOTE_STATE -> {
                    if (c == '"') {
                        state = NORMAL_TOKEN_STATE
                    } else if (c == '\\') {
                        // Look-ahead: only `"` and `\` are real escapes
                        // inside double quotes; anything else keeps both
                        // the backslash and the next character literally.
                        if (i + 1 < arguments.length) {
                            val next = arguments[i + 1]
                            if (next == '"' || next == '\\') {
                                currArg.append(next)
                                i++
                            } else {
                                currArg.append(c)
                            }
                        } else {
                            currArg.append(c)
                        }
                    } else {
                        currArg.append(c)
                    }
                }
                state == NO_TOKEN_STATE -> {
                    when {
                        c == '\\' -> {
                            escaped = true
                            state = NORMAL_TOKEN_STATE
                        }
                        c == '\'' -> state = SINGLE_QUOTE_STATE
                        c == '"' -> state = DOUBLE_QUOTE_STATE
                        !c.isWhitespace() -> {
                            currArg.append(c)
                            state = NORMAL_TOKEN_STATE
                        }
                    }
                }
                // NORMAL_TOKEN_STATE
                else -> {
                    when {
                        c == '\\' -> {
                            escaped = true
                            state = NORMAL_TOKEN_STATE
                        }
                        c == '\'' -> state = SINGLE_QUOTE_STATE
                        c == '"' -> state = DOUBLE_QUOTE_STATE
                        c.isWhitespace() -> {
                            argList.add(currArg.toString())
                            currArg = StringBuilder()
                            state = NO_TOKEN_STATE
                        }
                        else -> currArg.append(c)
                    }
                }
            }
            i++
        }

        // Trailing escape: keep the backslash literally.
        if (escaped) {
            currArg.append('\\')
        }
        // Trailing unclosed quote: keep the token as-is.
        if (state != NO_TOKEN_STATE) {
            argList.add(currArg.toString())
        }
        return if (stringify) {
            argList.map { "\"" + escapeQuotesAndBackslashes(it) + "\"" }
        } else {
            argList
        }
    }

    private fun escapeQuotesAndBackslashes(s: String): String {
        val buf = StringBuilder()
        // Iterate in reverse so that insert() indices stay valid.
        for (i in s.indices.reversed()) {
            buf.insert(0, s[i])
            when (s[i]) {
                '\\', '"' -> buf.insert(0, '\\')
                '\n' -> {
                    buf.deleteCharAt(1)
                    buf.insert(0, "\\n")
                }
                '\t' -> {
                    buf.deleteCharAt(1)
                    buf.insert(0, "\\t")
                }
                '\r' -> {
                    buf.deleteCharAt(1)
                    buf.insert(0, "\\r")
                }
                '\u0008' -> {
                    buf.deleteCharAt(1)
                    buf.insert(0, "\\b")
                }
                '\u000C' -> {
                    buf.deleteCharAt(1)
                    buf.insert(0, "\\f")
                }
            }
        }
        return buf.toString()
    }
}