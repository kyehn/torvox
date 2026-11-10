//! 回调事件收割：取走 VT 线程侧产生的剪贴板、通知与响铃事件。

use super::super::GhosttyTerminal;

impl GhosttyTerminal {
    /// 取走待回写 PTY 的应答字节块（VT 线程已产生、宿主尚未写回的部分）。
    pub fn drain_pty_write_responses(&self) -> Vec<Vec<u8>> {
        let mut guard = self
            .pty_write_responses
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        std::mem::take(&mut *guard)
    }

    /// 取走一个待上报的剪贴板事件 `(选择器名, 文本)`；无事件时为 None。
    pub fn poll_clipboard_event(&self) -> Option<(String, String)> {
        self.clipboard_rx.try_recv().ok()
    }

    /// 取走一个待上报的桌面通知 `(标题, 正文)`；无事件时为 None。
    ///
    /// OSC 9 不带标题，故标题常为空串；OSC 777 的 `notify` 形式带标题。
    pub fn poll_notification_event(&self) -> Option<(String, String)> {
        self.notification_queue.pop()
    }

    /// 取走响铃标志：自上次取走后终端收到过 BEL 返回 `true`，同时复位。
    /// 收割方每帧调用一次，取走即消费——重复取不会重放。
    pub(crate) fn take_bell(&self) -> bool {
        self.bell_flag
            .swap(false, std::sync::atomic::Ordering::AcqRel)
    }
}
