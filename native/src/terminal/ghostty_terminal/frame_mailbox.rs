//! 单元帧邮箱：VT 线程写入，渲染线程取走。

use std::sync::Arc;

use parking_lot::Mutex;

use super::types::{CellData, CursorInfo};

/// 单元帧邮箱：全程只保留最新一帧。
///
/// 帧是网格全量快照，后到的帧覆盖先到的即可——渲染线程只呈现最新状态。
/// 有界队列在满时丢弃**新**帧，最新的画面反而丢失，终端停在旧视口；
/// 单槽邮箱让生产者永远能覆盖旧帧，消费者取到的必是最新画面。
///
/// 只留一帧也省内存：一份 24×80 网格快照约 75KiB，四帧积压即 300KiB。
pub(crate) struct FrameMailbox {
    frame: Mutex<Option<(Vec<CellData>, CursorInfo)>>,
}

impl FrameMailbox {
    pub(crate) fn new() -> Arc<Self> {
        Arc::new(Self {
            frame: Mutex::new(None),
        })
    }

    /// 写入最新帧，覆盖尚未被取走的旧帧。
    pub(crate) fn store(&self, frame: (Vec<CellData>, CursorInfo)) {
        *self.frame.lock() = Some(frame);
    }

    /// 取走当前帧；已被取走则返回 `None`。
    pub(crate) fn take(&self) -> Option<(Vec<CellData>, CursorInfo)> {
        self.frame.lock().take()
    }
}
