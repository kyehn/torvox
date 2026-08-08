/*BEGIN_COPYRIGHT_BLOCK
 *
 * Copyright (c) 2001-2010, JavaPLT group at Rice University (drjava@rice.edu)
 * All rights reserved.
 *
 * Redistribution and use in source and binary forms, with or without
 * modification, are permitted provided that the following conditions are met:
 *    * Redistributions of source code must retain the above copyright
 *      notice, this list of conditions and the following disclaimer.
 *    * Redistributions in binary form must reproduce the above copyright
 *      notice, this list of conditions and the following disclaimer in the
 *      documentation and/or other materials provided with the distribution.
 *    * Neither the names of DrJava, the JavaPLT group, Rice University, nor the
 *      names of its contributors may be used to endorse or promote products
 *      derived from this software without specific prior written permission.
 *
 * THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND CONTRIBUTORS
 * "AS IS" AND ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT
 * LIMITED TO, THE IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR
 * A PARTICULAR PURPOSE ARE DISCLAIMED. IN NO EVENT SHALL THE COPYRIGHT OWNER OR
 * CONTRIBUTORS BE LIABLE FOR ANY DIRECT, INDIRECT, INCIDENTAL, SPECIAL,
 * EXEMPLARY, OR CONSEQUENTIAL DAMAGES (INCLUDING, BUT NOT LIMITED TO,
 * PROCUREMENT OF SUBSTITUTE GOODS OR SERVICES; LOSS OF USE, DATA, OR
 * PROFITS; OR BUSINESS INTERRUPTION) HOWEVER CAUSED AND ON ANY THEORY OF
 * LIABILITY, WHETHER IN CONTRACT, STRICT LIABILITY, OR TORT (INCLUDING
 * NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF THE USE OF THIS
 * SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.
 *
 * This software is Open Source Initiative approved Open Source Software.
 * Open Source Initative Approved is a trademark of the Open Source Initiative.
 *
 * This file is part of DrJava.  Download the current version of this project
 * from http://www.drjava.org/ or http://sourceforge.net/projects/drjava/
 *
 * END_COPYRIGHT_BLOCK*/

package terminal.emulator.util

/*
 * Kotlin port of DrJava's ArgumentTokenizer (3-clause BSD, Rice University
 * JavaPLT — license block above retained verbatim as BSD requires),
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

    // 1:1 port of termux-app's ArgumentTokenizer.java state machine — the
    // branching is inherent to the four-state parser; suppressing keeps the
    // port byte-for-byte comparable with upstream (TerminalViewModel.kt:534
    // precedent).
    @Suppress("LongMethod", "CyclomaticComplexMethod", "CognitiveComplexMethod", "NestedBlockDepth")
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
        // Iterate in reverse so that insert() indices stay valid. The
        // character just inserted sits at index 0, so control-char
        // replacement deletes index 0 before inserting the two-char escape
        // (round-227 T1b: this was deleteCharAt(1), which dropped the NEXT
        // already-processed character and corrupted \t/\r/\b/\f/\n output).
        for (i in s.indices.reversed()) {
            buf.insert(0, s[i])
            when (s[i]) {
                '\\', '"' -> buf.insert(0, '\\')

                '\n' -> {
                    buf.deleteCharAt(0)
                    buf.insert(0, "\\n")
                }

                '\t' -> {
                    buf.deleteCharAt(0)
                    buf.insert(0, "\\t")
                }

                '\r' -> {
                    buf.deleteCharAt(0)
                    buf.insert(0, "\\r")
                }

                '\u0008' -> {
                    buf.deleteCharAt(0)
                    buf.insert(0, "\\b")
                }

                '\u000C' -> {
                    buf.deleteCharAt(0)
                    buf.insert(0, "\\f")
                }
            }
        }
        return buf.toString()
    }
}
