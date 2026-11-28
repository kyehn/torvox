//! 单元帧邮箱：VT 线程写入，渲染线程取走。

use std::sync::Arc;
#[cfg(test)]
use std::sync::atomic::{AtomicUsize, Ordering};

use parking_lot::Mutex;

use super::types::{CellData, CursorInfo};

/// 单元帧邮箱：全程只保留最新一帧。
///
/// 帧是网格全量快照，后到的帧覆盖先到的即可——渲染线程只呈现最新状态。
/// 有界队列在满时丢弃**新**帧，最新的画面反而丢失，终端停在旧视口；
/// 单槽邮箱让生产者永远能覆盖旧帧，消费者取到的必是最新画面。
///
/// 只留一帧也省内存：一份 24×80 网格快照约 75KiB，四帧积压即 300KiB。
///
/// 帧内容用 `Arc` 共享：同一份网格还要交给行缓存（下一帧复用干净行）与去重基线
/// （逐字节比对），三者共用一份即可，不必各存一份整网格拷贝。
pub(crate) struct FrameMailbox {
    frame: Mutex<Option<(Arc<Vec<CellData>>, CursorInfo)>>,
    /// 最近一次发布的帧的内存身份，仅测试使用。
    ///
    /// 发布路径上的一次整网格克隆（24×80 约 184KB）在内容断言里完全看不出来，
    /// 只有比对内存身份才判得出来。
    #[cfg(test)]
    published_identity: AtomicUsize,
}

impl FrameMailbox {
    pub(crate) fn new() -> Arc<Self> {
        Arc::new(Self {
            frame: Mutex::new(None),
            #[cfg(test)]
            published_identity: AtomicUsize::new(0),
        })
    }

    /// 写入最新帧，覆盖尚未被取走的旧帧。
    pub(crate) fn store(&self, frame: (Arc<Vec<CellData>>, CursorInfo)) {
        *self.frame.lock() = Some(frame);
    }

    /// 取走当前帧；已被取走则返回 `None`。
    pub(crate) fn take(&self) -> Option<(Arc<Vec<CellData>>, CursorInfo)> {
        self.frame.lock().take()
    }

    /// 记录刚发布的帧的内存身份，仅测试使用。
    ///
    /// 记的是**交给邮箱之前**那份网格的地址：渲染侧取到的必须是同一块内存，
    /// 而不是发布路径上克隆出来的副本。
    #[cfg(test)]
    pub(crate) fn record_published(&self, frame: &Arc<Vec<CellData>>) {
        self.published_identity
            .store(Arc::as_ptr(frame) as usize, Ordering::Relaxed);
    }

    /// 取最近一次发布的帧的内存身份，仅测试使用。
    #[cfg(test)]
    pub(crate) fn published_identity(&self) -> usize {
        self.published_identity.load(Ordering::Relaxed)
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
        mailbox.store((Arc::new(vec![CellData::zeroed()]), cursor()));
        mailbox.store((Arc::new(vec![CellData::zeroed(); 2]), cursor()));

        let (cells, _) = mailbox.take().expect("最新帧必须可取");
        assert_eq!(cells.len(), 2);
        assert!(mailbox.take().is_none(), "取走后不得残留旧帧");
    }
}
