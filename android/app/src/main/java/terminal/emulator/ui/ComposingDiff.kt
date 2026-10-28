package terminal.emulator.ui

/**
 * 输入法的候选区（拼音、手写）校对：把每次 `setComposingText` 增量翻译为 PTY 编辑，
 * 使终端缓冲区与输入法所见保持一致。
 */
object ComposingDiff {
    /**
     * 把上一个候选区变为 `next` 所需的编辑：追加/回退/全量重写三种情况统一按
     * 「最长公共前缀 + 其后重打」处理。退格数按码点计，删去一个 emoji 只算一次。
     */
    data class Edit(val backspaces: Int, val append: String) {
        val isEmpty: Boolean
            get() = backspaces == 0 && append.isEmpty()
    }

    fun reconcile(previous: String, next: String): Edit {
        if (previous == next) return Edit(0, "")
        // 按码点求最长公共前缀：UTF-16 下标可能落在代理对中间（👍 与 👎 只差低代理位），
        // 那样待追加段是半个码元，编不出合法 UTF-8（会被替换成 U+FFFD），
        // 待擦除段也只覆盖半个字符。切分点必须落在码点边界上。
        var commonPrefixLength = 0
        val maxPrefixLength = minOf(previous.length, next.length)
        while (commonPrefixLength < maxPrefixLength) {
            val previousCodePoint = previous.codePointAt(commonPrefixLength)
            if (previousCodePoint != next.codePointAt(commonPrefixLength)) break
            commonPrefixLength += Character.charCount(previousCodePoint)
        }

        val textToErase = if (previous.length > commonPrefixLength) previous.substring(commonPrefixLength) else ""
        val backspaces = textToErase.codePointCount(0, textToErase.length)
        val toAdd = if (next.length > commonPrefixLength) next.substring(commonPrefixLength) else ""
        return Edit(backspaces, toAdd)
    }

    /**
     * 把组字基线改写为目标基线的重放计划。
     *
     * 终端的输入行由终端自身持有，本仓只能靠「方向键 + 退格 + 重打」复现输入法的编辑。
     * 退格删的是**光标前**的字符，故差分非空时必须先把光标右移到旧组字区末尾——
     * 光标停在组字区中间时直接退格，删掉的是组字区**之前已提交**的内容。
     */
    data class Replay(
        /** 改写前的右移次数（码点）：差分非空时挪到旧组字区末尾，差分为空时是相对前移量。 */
        val rightMovesBeforeEdit: Int,
        /** 差分退格数（码点）。 */
        val backspaces: Int,
        /** 差分追加文本。 */
        val append: String,
        /** 改写后的左移次数（码点）：差分非空时从新组字区末尾挪到目标组字光标，差分为空时是相对后移量。 */
        val leftMovesAfterEdit: Int,
    )

    /**
     * 计算把组字基线（[previous] / [previousCursor]）改写为（[next] / [nextCursor]）的重放计划。
     *
     * [previousCursor] 与 [nextCursor] 是 UTF-16 下标，须落在各自文本范围内。
     */
    fun planReplay(previous: String, previousCursor: Int, next: String, nextCursor: Int): Replay {
        val edit = reconcile(previous, next)
        if (edit.isEmpty) {
            // 组字文本未变：MUST NOT 重放字节，光标按相对位移挪动即可。
            val offset =
                next.codePointCount(0, nextCursor) - previous.codePointCount(0, previousCursor)
            return Replay(
                rightMovesBeforeEdit = if (offset > 0) offset else 0,
                backspaces = 0,
                append = "",
                leftMovesAfterEdit = if (offset < 0) -offset else 0,
            )
        }
        return Replay(
            rightMovesBeforeEdit = previous.codePointCount(previousCursor, previous.length),
            backspaces = edit.backspaces,
            append = edit.append,
            leftMovesAfterEdit = next.codePointCount(nextCursor, next.length),
        )
    }

    /**
     * 计算清空整个组字区的重放计划：先挪到组字区末尾，再退格删净。
     *
     * 退格删的是光标前的字符，故组字光标在中间时必须先右移到末尾，
     * 否则删掉的是组字区之前已提交的内容。
     */
    fun planClear(previous: String, previousCursor: Int): Replay = Replay(
        rightMovesBeforeEdit = previous.codePointCount(previousCursor, previous.length),
        backspaces = previous.codePointCount(0, previous.length),
        append = "",
        leftMovesAfterEdit = 0,
    )
}
