package terminal.emulator.runtime

import terminal.emulator.bridge.Bridge
import terminal.emulator.bridge.PollEvent
import terminal.emulator.bridge.pollEventJson

/**
 * 每轮排空待投递桌面通知的上限：与事件队列每帧上限（[Bridge.MAX_EVENTS_PER_POLL]）同口径。
 *
 * 两者面对的都是「远端程序可无限触发」的输入，量级相同；上限不同会让两条路径在同一台
 * 设备上表现不一致。
 */
internal const val MAX_NOTIFICATIONS_PER_DRAIN = Bridge.MAX_EVENTS_PER_POLL

/**
 * 排空一轮桌面通知并逐条投递，返回本轮投递的条数。
 *
 * 有界排空：达到 [maxNotifications] 即结束本轮并回到等待点，其余通知留在原生队列里，
 * 由下一轮继续排空——排空不设上限时，通知的产出快于消费就会让循环永不结束，轮询再也
 * 回不到等待点，投递速率随之失控。
 *
 * 解码失败只丢这一条：解码器把出错的片段带进异常消息，而通知正文可能含敏感内容，
 * 故只记异常类名与 JSON 长度。
 *
 * [sessionLabel] 按会话 id 给出序号标签（如「会话 3」）；会话已关闭时给出空串。
 * [maxNotifications] 只供单测缩小规模；生产只用默认值。
 */
internal fun drainDesktopNotifications(
    pollEvent: () -> String?,
    sessionLabel: (Long) -> String,
    deliver: (SessionNotification) -> Unit,
    maxNotifications: Int = MAX_NOTIFICATIONS_PER_DRAIN,
): Int {
    var delivered = 0
    // 计数按「取用次数」而非「投递条数」：解码失败的条目同样消耗取用次数，故一轮的
    // 取用开销有界，不会被一批坏数据拖成无限循环。
    var pollAttempt = 0
    while (pollAttempt < maxNotifications) {
        pollAttempt += 1
        val json = pollEvent() ?: break
        val event =
            try {
                pollEventJson.decodeFromString<PollEvent>(json)
            } catch (exception: Exception) {
                LogUtil.warn(
                    TAG,
                    "desktop notification dropped (${exception.javaClass.simpleName}, ${json.length} chars)",
                )
                continue
            }
        if (event !is PollEvent.DesktopNotification) continue
        deliver(
            SessionNotification(
                sessionId = event.sessionId,
                sessionLabel = sessionLabel(event.sessionId),
                title = event.title,
                body = event.body,
            ),
        )
        delivered += 1
    }
    return delivered
}

/** 日志标签：与 [DesktopNotificationPlayer] 同属一个功能域。 */
private const val TAG = "DesktopNotification"
