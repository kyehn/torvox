package terminal.emulator.runtime

// 切会话三个阶段的判据：首帧重试、并发切换守卫与失败恢复。
// 三者只服务 [TerminalRuntime.switchSession]，故与切换流程同处一地。

/**
 * 初始同步渲染重试（switchSession）：只有硬失败（< 0：会话未注册、渲染态异常等）
 * 才重试。**`0` 不是失败**：它是「本帧没有可画的内容」，surface 尚未就绪、目标会话
 * 暂无新帧、缓存帧属于别的会话都会返回 0——切会话后重推请求已由原生渲染路径发出，
 * 该帧到达即呈现，重试只会空转。纯决策，从重试循环中抽出以便测试。
 */
internal fun initialRenderRetryNeeded(result: Int, attempts: Int, maxAttempts: Int): Boolean =
    result < 0 && attempts < maxAttempts

/**
 * switchSession 阶段 3 的并发切换守卫：首帧渲染期间（在 SessionRegistry 的总闸之外），
 * 另一个 switchSession 可能发布了不同的活动会话。返回在发布本次切换之前
 * 必须停止其渲染线程的会话 id；当活动会话未变（或正是本目标）时返回 null。
 */
internal fun concurrentRenderThreadToStop(
    activeSessionIdAfterRender: Long?,
    previousActiveId: Long?,
    targetId: Long,
    concurrentSessionId: Long?,
): Long? {
    if (activeSessionIdAfterRender == previousActiveId) return null
    val concurrentId = concurrentSessionId ?: return null
    if (concurrentId == targetId) return null
    return concurrentId
}

/** switchSession 阶段 1 的失败恢复：spawn 失败后重启前一个活动会话，除非它正是刚刚失败的那个会话。 */
internal fun shouldRestorePreviousSession(previousId: Long?, failedTargetId: Long): Boolean =
    previousId != null && previousId != failedTargetId
