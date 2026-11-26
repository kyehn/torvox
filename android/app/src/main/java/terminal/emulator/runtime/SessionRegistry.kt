package terminal.emulator.runtime

import terminal.emulator.bridge.Bridge
import java.util.concurrent.ConcurrentHashMap
import kotlin.contracts.ExperimentalContracts
import kotlin.contracts.InvocationKind
import kotlin.contracts.contract

/**
 * 会话表与活动会话 id 的唯一所有者。
 *
 * 会话表的增删与活动会话的切换必须在同一把闸下完成：分两次锁外读会拼出
 * 「活动会话已关闭却仍留在列表里」的组合——活动会话 id 与会话列表必须同源。
 * 闸、表与活动 id 都收在本模块内，调用方拿到的只是条目视图与闸本身。
 */
internal class SessionRegistry {
    /**
     * 会话总闸。
     *
     * 可见性为 internal 而非 private：[withLock] 是 inline 函数，inline 函数只能
     * 访问与自身可见性相同或更高的成员。
     */
    internal val lock = Any()

    /** 会话表。无锁读取供「看一眼当前条目」的路径使用。 */
    private val entries = ConcurrentHashMap<Long, SessionEntry>()

    /**
     * 活动会话 id。
     *
     * volatile 而非普通字段：渲染线程与主线程都在闸外读它，闸外的可见性只能由
     * volatile 保证（普通字段的写可能停在写线程的寄存器里，读线程永远看不到）。
     */
    @Volatile
    private var activeId: Long = 0L

    /** 活动会话 id。0 表示没有活动会话。 */
    val activeSessionId: Long
        get() = activeId

    /**
     * 在会话总闸内执行 [block]。
     *
     * 与 `synchronized` 同义（可非局部返回），但调用点不必知道闸的实现：
     * 总闸若换成别的协调机制，改动只落在这一个模块里。
     *
     * `callsInPlace` 契约不可省：它告诉编译器闸内的块恰好执行一次，闸外声明、
     * 闸内赋值的局部变量才能通过「恰好赋值一次」的定值分析。
     */
    @OptIn(ExperimentalContracts::class)
    inline fun <T> withLock(block: () -> T): T {
        contract { callsInPlace(block, InvocationKind.EXACTLY_ONCE) }
        return synchronized(lock, block)
    }

    /** 取指定会话条目。 */
    fun entry(sessionId: Long): SessionEntry? = entries[sessionId]

    /** 取活动会话条目。 */
    fun activeEntry(): SessionEntry? = entries[activeId]

    /** 会话是否仍在表中。 */
    fun contains(sessionId: Long): Boolean = entries.containsKey(sessionId)

    /** 会话数。 */
    val size: Int
        get() = entries.size

    /** 是否没有会话。 */
    fun isEmpty(): Boolean = entries.isEmpty()

    /**
     * 全部会话 id，**升序**。
     *
     * 升序是契约而非实现细节：抽屉里的标签序号按它换算，顺序随哈希表桶序漂移
     * 会让同一个会话在不同帧换序号。
     */
    fun ids(): List<Long> = entries.keys.sorted()

    /** 全部会话条目。 */
    fun values(): Collection<SessionEntry> = entries.values

    /** 登记会话。 */
    fun register(entry: SessionEntry) {
        entries[entry.sessionId] = entry
    }

    /** 移除会话，返回被移除的条目。 */
    fun remove(sessionId: Long): SessionEntry? = entries.remove(sessionId)

    /** 移除挂在指定 bridge 上的全部会话，返回移除条数。 */
    fun removeByBridge(bridge: Bridge): Int {
        val before = entries.size
        entries.entries.removeIf { it.value.bridge === bridge }
        return before - entries.size
    }

    /** 切换活动会话。 */
    fun setActive(sessionId: Long) {
        activeId = sessionId
    }
}
