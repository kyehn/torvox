package terminal.emulator.runtime

import android.os.SystemClock

/**
 * 开孔状态的一条转换记录（供闪烁取证）。
 *
 * [painted] 是「视图已把开孔涂满（遮罩画面或底色）并交给窗口」的事实记录，
 * 只与某一份遮罩一起出现一次：重复重绘不再记流水账。
 *
 * [sequence] 是该条记录的序号：状态相同时也要靠它区分先后，故相等判断只比状态三元组。
 *
 * [stampMs] 是记录时刻（进程内单调时钟，毫秒）：「遮罩发布 → 遮罩上屏」的时延只能
 * 从这里量——外部采样在切后台期间会被平台拆掉窗口缓冲而整段丢失，量到的时延是
 * 采样空档而非真实时延。
 */
internal data class HoleStateEntry(
    val bufferPresent: Boolean,
    val coverPresent: Boolean,
    val painted: Boolean = false,
    val sequence: Long = 0L,
    val stampMs: Long = 0L,
)

/**
 * 开孔状态转换日志：每次「缓冲区在位 / 遮罩在位 / 已上屏」变化记一条，只留最近
 * [DEFAULT_CAPACITY] 条。
 *
 * 闪烁是一两帧的瞬态：截图来不及，轮询采样在后台会被进程冻结吞掉，回前台的关键
 * 路径又会压过采样线程。产品自己的状态机是唯一不丢事件的来源，故在标志的写入点
 * 各记一条；仪器化测试据此验证「可见时开孔永远有内容」。
 *
 * 光有「遮罩在位」还不够：遮罩得真的进了窗口缓冲区才有用。标志翻转只说明运行期
 * 有了遮罩，视图未必被要求重绘——切后台那一刻没有别的失效源，漏一次重绘，窗口
 * 留着上一帧的**空开孔**，回前台首帧就是用户看到的那一下闪。故视图每画一次覆盖
 * 开孔的内容就回记一条（[recordPainted]）。
 *
 * 日志是环形缓冲，满了就挤掉最早的条目，故「某段时间内新增了哪些条目」不能按
 * 条数差分——前后都满时差分恒为空，整段往返会被算成「什么都没发生」。序号单调
 * 递增且不受绕回影响，差分改按序号取（见 [holeStateCycle]）。
 */
internal class HoleStateJournal(private val capacity: Int = DEFAULT_CAPACITY) {
    private val lock = Any()
    private val entries = ArrayDeque<HoleStateEntry>()
    private var sequence = 0L

    /** 当前这份遮罩是否已被视图画进窗口（见 [recordPainted]）。 */
    private var paintedForCover = false

    /** 记录一次状态；与上一条相同（且尚未标上屏）则忽略。 */
    fun record(bufferPresent: Boolean, coverPresent: Boolean) {
        synchronized(lock) {
            val last = entries.lastOrNull()
            if (
                last != null &&
                last.bufferPresent == bufferPresent &&
                last.coverPresent == coverPresent &&
                !last.painted
            ) {
                return
            }
            append(HoleStateEntry(bufferPresent, coverPresent))
        }
    }

    /**
     * 记下「视图已把开孔涂满并重绘了窗口」（视图在主线程绘制路径上调用）。
     *
     * 这是遮罩**真的上屏**的唯一证据：运行期发布遮罩不会自己让窗口重绘，而闪烁
     * 恰恰发生在「遮罩已发布、窗口没重绘」的那一刻。每份遮罩只记一次。
     */
    fun recordPainted(bufferPresent: Boolean, coverPresent: Boolean) {
        synchronized(lock) {
            if (paintedForCover) return
            paintedForCover = true
            append(HoleStateEntry(bufferPresent, coverPresent, painted = true))
        }
    }

    /** 新一份遮罩发布或撤下：上一份的「已上屏」记录随之作废。 */
    fun resetPainted() {
        synchronized(lock) { paintedForCover = false }
    }

    /** 转换快照（旧 → 新）。 */
    fun snapshot(): List<HoleStateEntry> = synchronized(lock) { entries.toList() }

    /** 追加一条记录并盖上序号与时刻（调用方须持 [lock]）。 */
    private fun append(entry: HoleStateEntry) {
        entries.addLast(
            entry.copy(sequence = ++sequence, stampMs = SystemClock.uptimeMillis()),
        )
        if (entries.size > capacity) entries.removeFirst()
    }

    private companion object {
        /** 只留最近若干条，足够覆盖一次后台往返。 */
        const val DEFAULT_CAPACITY = 32
    }
}
