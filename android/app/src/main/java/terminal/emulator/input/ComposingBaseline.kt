package terminal.emulator.input

/**
 * 输入法组字基线：输入法「已写入但未提交」的那段文本与组字光标。
 *
 * 为何必须独立于 `InputConnection` 存活：
 * `BaseInputConnection` 的默认 `getEditable()` 自持一份 editable，而本仓从不读它。
 * 于是取文类接口（`getTextBeforeCursor` 等）若不覆写，返回的是那份**空** editable
 * ——输入法据此判定字段没有内容，提交后看不到自己刚写的字，随即整词重发，
 * 终端里出现重复文本。让基线成为可单测的纯数据持有者，取文接口才有一致的事实来源。
 *
 * [cursor] 按 UTF-16 下标计，与 `newCursorPosition` 同系。
 */
class ComposingBaseline {
    /** 进行中的组字文本；空表示无组字。 */
    var text: String = ""
        private set

    /** 组字光标在 [text] 中的 UTF-16 下标，范围 `0..text.length`。 */
    var cursor: Int = 0
        private set

    val isComposing: Boolean
        get() = text.isNotEmpty()

    /**
     * 记录一次组字：[text] 为空表示组字结束。
     *
     * 光标按 [newCursorPosition] 推出，与平台同口径：
     * - `> 0`：相对组字文本末尾的新偏移（`text.length + newCursorPosition`）；
     * - `<= 0`：绝对位置 `newCursorPosition`（通常 0 即回到组字开头）。
     *
     * 两种语义都钳位在 `0..text.length`：输入法可以给出越界值（诚实降级而非
     * 抛异常——越界光标会让输入法的后续 diff 全部错位）。
     */
    fun compose(text: String?, newCursorPosition: Int) {
        val next = text.orEmpty()
        this.text = next
        cursor = resolveCursor(next, newCursorPosition)
    }

    /** 把组字文本与光标重设为绝对位置；越界光标钳入文本范围。 */
    internal fun composeAt(text: String?, cursor: Int) {
        val next = text.orEmpty()
        this.text = next
        this.cursor = cursor.coerceIn(0, next.length)
    }

    /** 计算 `setComposingText` 的绝对光标位置。 */
    internal fun resolveCursor(text: String, newCursorPosition: Int): Int {
        val absolute =
            if (newCursorPosition > 0) {
                text.length + newCursorPosition
            } else {
                newCursorPosition
            }
        return absolute.coerceIn(0, text.length)
    }

    /** 提交/结束组字：文本与光标一并清空。 */
    fun clear() {
        text = ""
        cursor = 0
    }

    /**
     * 光标前的 [length] 个 UTF-16 单位；越界时只返回已有的部分。
     *
     * 该接口只回答「输入法自己写进来的组字内容」，绝不掺入屏幕或回滚文本——
     * 否则输入法会把终端上任意位置的字符当成自己的候选区来改写。
     */
    fun textBefore(length: Int): String {
        if (length <= 0) return ""
        val from = (cursor - length).coerceAtLeast(0)
        return text.substring(from, cursor)
    }

    /** 光标后的 [length] 个 UTF-16 单位；越界时只返回已有的部分。 */
    fun textAfter(length: Int): String {
        if (length <= 0) return ""
        val endIndex = (cursor + length).coerceAtMost(text.length)
        return text.substring(cursor, endIndex)
    }

    /**
     * 按「头 + 替换文本 + 尾」构造把 `[start, end)` 换成 [replacement] 后的目标组字串。
     *
     * 返回 null 表示范围不落在基线内——调用方必须**拒绝并发出零字节**。
     * 这是刻意的：输入法的下标来自它自己的模型，越界时无从判断该改写终端的哪里，
     * 猜测会让已提交内容被错误删改。
     */
    fun replaceRange(start: Int, end: Int, replacement: String): String? {
        if (!addressable(start, end)) return null
        return buildString {
            append(text, 0, start)
            append(replacement)
            append(text, end, text.length)
        }
    }

    /**
     * 把组字区域重锚到 `[start, end)`，返回该区间内的目标组字串；
     * 范围不落在基线内时返回 null（调用方须拒绝并发出零字节）。
     *
     * 与 [replaceRange] 的区别是本操作不引入新文本：输入法只是把「哪一段正在组字」
     * 重新标定，区间外的已提交内容必须原样保留。
     */
    fun reanchorRegion(start: Int, end: Int): String? {
        if (!addressable(start, end)) return null
        return text.substring(start, end)
    }

    private fun addressable(start: Int, end: Int): Boolean = start >= 0 && end >= start && end <= text.length

    /**
     * 删除光标前后各若干码点后的组字内容与绝对光标。
     *
     * 返回实际删除数：输入法可给出超出基线的长度，按已有内容截断。
     */
    internal fun deletionTarget(beforeLength: Int, afterLength: Int): Deletion {
        var start = cursor
        var beforeRemoved = 0
        while (beforeRemoved < beforeLength && start > 0) {
            start -= Character.charCount(text.codePointBefore(start))
            beforeRemoved++
        }
        var end = cursor
        var afterRemoved = 0
        while (afterRemoved < afterLength && end < text.length) {
            end += Character.charCount(text.codePointAt(end))
            afterRemoved++
        }
        return Deletion(text.removeRange(start, end), start, beforeRemoved, afterRemoved)
    }

    internal data class Deletion(
        val text: String,
        val cursor: Int,
        val beforeCount: Int,
        val afterCount: Int,
    )
}
