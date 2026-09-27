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
        // 按 UTF-16 下标求最长公共前缀；前缀两侧相同，故码点切分点一致。
        var i = 0
        val maxI = minOf(previous.length, next.length)
        while (i < maxI && previous[i] == next[i]) i++

        val textToErase = if (previous.length > i) previous.substring(i) else ""
        val backspaces = textToErase.codePointCount(0, textToErase.length)
        val toAdd = if (next.length > i) next.substring(i) else ""
        return Edit(backspaces, toAdd)
    }
}
