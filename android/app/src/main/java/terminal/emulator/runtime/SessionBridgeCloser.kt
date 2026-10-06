package terminal.emulator.runtime

/**
 * 关闭会话的原生 bridge —— 但绝不在仍存活的渲染线程脚下关闭。
 *
 * 渲染线程卡在原生渲染代码里（GPU 挂起、join 超时）时，销毁原生会话即
 * use-after-free：线程继续访问已释放的 wgpu/终端状态。这类会话宁可让原生
 * 侧泄漏、待进程消亡时整体回收，也不能在其下 `close()`。
 *
 * 四条拆卸路径（shell 退出、监视器判死、切换失败回滚、并发移除回滚）都
 * 必须在同一条规则下关闭，否则任何一条漏掉守卫就重新打开这个 UAF。
 *
 * @param reason 进入该路径的原因，仅用于日志定位。
 */
internal fun SessionEntry.closeBridgeUnlessRenderThreadAlive(reason: String) {
    if (renderThreadPossiblyAlive) {
        LogUtil.e(
            "Runtime",
            "session $id render thread possibly alive — skipping bridge close ($reason)",
        )
        return
    }
    try {
        bridge.close()
    } catch (exception: Exception) {
        LogUtil.e("Runtime", "session $id bridge close failed ($reason)", exception)
    }
}
