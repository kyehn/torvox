package terminal.emulator.ui

/**
 * 渲染暂停**记账**。
 *
 * 各来源（设置页、切后台、Surface 销毁、交换链防抖、输入法网格重排）各自领一次
 * 暂停、到期归还一次；运行期的 `setRenderPaused` 是非计数布尔，而这些窗口彼此
 * 可能重叠，故须计数。两个不变量：
 *
 * - 只有归属自己的持有才会归还，且只能归还一次（重复归还会减掉别人的）。
 * - 集合从空变为非空时暂停、从非空变为空时恢复。
 *
 * 独立于 [android.view.View] 是为了能在 JVM 单测里跑完整条记账序列——这个记账
 * 出过两次错（被丢弃的防抖不归还、归还时动到别人的持有），两次都表现为共享
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
     * 凭证不在持有集合里就是陈旧的（早已归还过），此时
     * MUST NOT 改动计数。
     *
     * 被 `removeCallbacks` 丢弃的防抖同样走这里归还：它永不执行，归还也永不自行
     * 发生，持有就永久留在集合里（渲染器永不恢复）。
     *
     * 调用顺序无约束：先归还后领取会让持有者数经过 0，多发一对恢复/暂停；先领取后
     * 归还则全程非空。推荐后者。
     */
    fun release(token: Long) {
        if (!holders.remove(token)) return
        if (holders.isEmpty()) onPauseChanged(false)
    }
}
