package terminal.emulator.runtime

import terminal.emulator.bridge.Bridge

/**
 * 关闭会话的原生 bridge —— 但绝不在仍存活的渲染线程脚下关闭。
 *
 * 渲染线程卡在原生渲染代码里（GPU 挂起、join 超时）时，销毁原生会话即
 * use-after-free：线程继续访问已释放的 wgpu/终端状态。这类会话宁可让原生
 * 侧泄漏、待进程消亡时整体回收，也不能在其下 `close()`。
 *
 * 五条拆卸路径（shell 退出、监视器判死、切换失败回滚、并发移除回滚、用户关闭）
 * 都必须在同一条规则下关闭，否则任何一条漏掉守卫就重新打开这个 UAF。
 *
 * @param reason 进入该路径的原因，仅用于日志定位。
 * @param close 真正执行销毁的动作，默认只 `close()`。用户关闭那条路径还要先
 *   释放 GPU surface，它经此参数进入同一守卫——守卫只有一处，故不存在「某条
 *   路径顺带多释放一次 surface」或「某条路径漏掉守卫」的分叉。
 */
internal fun SessionEntry.closeBridgeUnlessRenderThreadAlive(
    reason: String,
    close: (Bridge) -> Unit = { it.close() },
) {
    if (renderThreadPossiblyAlive) {
        LogUtil.error(
            "Runtime",
            "session $sessionId render thread possibly alive — skipping bridge close ($reason)",
        )
        return
    }
    try {
        close(bridge)
    } catch (exception: Exception) {
        LogUtil.error("Runtime", "session $sessionId bridge close failed ($reason)", exception)
    }
}
