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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::terminal::ghostty_terminal::CursorStyle;
    use bytemuck::Zeroable;

    fn cursor() -> CursorInfo {
        CursorInfo {
            row: 0,
            col: 0,
            at_wide_tail: false,
            visible: true,
            style: CursorStyle::Block,
            scrollback_length: 0,
            viewport_row: 0,
            scroll_request_id: 0,
            rows: 24,
            cols: 80,
            kitty_generation: 0,
            kitty_placement_generation: 0,
        }
    }

    /// 后来的帧覆盖未被取走的旧帧：若像有界队列那样满时丢新帧，终端会停在旧视口。
    #[test]
    fn last_stored_frame_wins_and_is_taken_once() {
        let mailbox = FrameMailbox::new();
        mailbox.store((vec![CellData::zeroed()], cursor()));
        mailbox.store((vec![CellData::zeroed(); 2], cursor()));

        let (cells, _) = mailbox.take().expect("最新帧必须可取");
        assert_eq!(cells.len(), 2);
        assert!(mailbox.take().is_none(), "取走后不得残留旧帧");
    }
}
