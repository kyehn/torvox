package terminal.emulator.ui

/**
 * 防抖期间的渲染暂停**记账**。
 *
 * 两个防抖（交换链重配、输入法网格重排）各自领一次暂停、到期归还一次；运行期的
 * `setRenderPaused` 是非计数布尔，而两者窗口可能重叠，故须计数。三个不变量：
 *
 * - 只有归属自己的持有才会归还，且只能归还一次（重复归还会减掉别人的）。
 * - [reset] 之后，任何**陈旧**持有者的归还都是空操作——否则它会去减 reset 之后
 *   新领的那一次，等于提前放开了仍在等稳定尺寸的那个防抖。
 * - 集合从空变为非空时暂停、从非空变为空时恢复。
 *
 * 独立于 [android.view.View] 是为了能在 JVM 单测里跑完整条记账序列——这个记账
 * 出过两次错（被丢弃的防抖不归还、reset 不作废旧持有者），两次都表现为共享
 * 渲染器被永久暂停即终端全黑，而当时没有任何测试能覆盖到它。
 */
internal class RenderPauseLedger(private val onPauseChanged: (Boolean) -> Unit) {

    private val holders = mutableSetOf<Long>()
    private var nextToken = 0L

    /** 当前持有者数量；仅供诊断与断言使用。 */
    val holderCount: Int
        get() = holders.size

    /** 领一次暂停，返回本次持有的凭证。 */
    fun acquire(): Long {
        val token = nextToken++
        if (holders.isEmpty()) onPauseChanged(true)
        holders += token
        return token
    }

    /**
     * 归还 [acquire] 领到的那一次。
     *
     * 凭证不在持有集合里就是陈旧的（已被 [releaseAndCancel] 或 [reset] 注销），
     * 此时 MUST NOT 改动计数。
     */
    fun release(token: Long) {
        if (!holders.remove(token)) return
        if (holders.isEmpty()) onPauseChanged(false)
    }

    /**
     * 注销一个被取消的防抖所持有的暂停。
     *
     * 取消必须在重新领取**之前**调用：被 `removeCallbacks` 丢弃的防抖永不执行，
     * 它的 `release` 也永不发生，持有就永久留在集合里（渲染器永不恢复）。
     */
    fun releaseAndCancel(token: Long) = release(token)

    /**
     * 作废全部持有并无条件恢复渲染。
     *
     * 用于「我们等的那件事已经发生，不必再等」的入口（Surface 重建成功、交换链
     * 重配完成）。这些入口本就必须立刻出一帧，此时继续挂着防抖暂停只会让已就绪的
     * 缓冲不显示；而清空集合让之后到达的陈旧 `release` 自动成为空操作。
     */
    fun reset() {
        if (holders.isEmpty()) {
            onPauseChanged(false)
            return
        }
        holders.clear()
        onPauseChanged(false)
    }
}
