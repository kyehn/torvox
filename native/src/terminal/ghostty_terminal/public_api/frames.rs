//! 帧与快照取回：渲染侧按帧取画面与网格快照。

use std::sync::Arc;

use flume::{Sender, bounded};

use super::super::GhosttyTerminal;
use super::super::commands::{Command, Query};
use super::super::types::{
    CellData, CursorInfo, GridSnapshot, KittyGraphicsImageData, KittyPlacementFrame,
    QUERY_TIMEOUT_MS,
};

impl GhosttyTerminal {
    /// 返回当前视口的**最新**网格快照：始终阻塞到 VT 线程处理完请求，调用方不会看到
    /// 陈旧缓存帧。VT 线程仅在网格实际变化时重建快照（见 `snapshot_needs_rebuild`），
    /// 故阻塞代价仅一次通道往返。
    ///
    /// 通道满或超时说明 VT 线程已卡死：记错误后 panic，不回退到空白网格。
    pub fn take_snapshot(&self) -> GridSnapshot {
        let (tx, rx): (Sender<Arc<GridSnapshot>>, _) = bounded(1);
        if let Err(error) = self.cmd_tx.try_send(Command::TakeSnapshot { tx }) {
            log::error!("ghostty_terminal: 快照命令入队失败（VT 线程已卡死）: {error}");
            panic!("ghostty_terminal: 快照命令入队失败: {error}");
        }
        let snapshot = rx
            .recv_timeout(std::time::Duration::from_millis(QUERY_TIMEOUT_MS))
            .unwrap_or_else(|error| {
                log::error!("ghostty_terminal: 等待 VT 线程快照超时: {error}");
                panic!("ghostty_terminal: 等待 VT 线程快照超时: {error}");
            });
        Arc::unwrap_or_clone(snapshot)
    }

    /// 按 id 取回 Kitty 图像的原始像素；id 不存在时为 None。
    pub fn take_kitty_graphics_image(&self, image_id: u32) -> Option<KittyGraphicsImageData> {
        self.query(
            |tx| Query::TakeKittyGraphicsImage { id: image_id, tx },
            None,
            "take_kitty_graphics_image",
        )
    }

    /// 采集全部可见 Kitty 放置（几何 + RGBA），供渲染线程组装图集。
    pub fn take_kitty_placements(&self) -> Vec<KittyPlacementFrame> {
        Self::take_kitty_placements_on(&self.query_tx)
    }

    /// [`Self::take_kitty_placements`] 的通道外置版，理由同 [`Self::title_on`]。
    pub(crate) fn take_kitty_placements_on(query_tx: &Sender<Query>) -> Vec<KittyPlacementFrame> {
        Self::query_on(
            query_tx,
            |tx| Query::TakeKittyPlacements { tx },
            Vec::new(),
            "take_kitty_placements",
        )
    }

    /// 取与渲染同源的光标视口坐标 `(行, 列)`；光标隐藏或构建失败时为 None。
    pub fn render_cursor(&self) -> Option<(u32, u32)> {
        Self::render_cursor_on(&self.query_tx)
    }

    /// [`Self::render_cursor`] 的通道外置版，理由同 [`Self::title_on`]。
    ///
    /// 这一条比其余查询更该走锁外：光标坐标取自 `build_cell_data`，即与渲染同源的
    /// **整网格**构建，耗时随网格面积走（忙时上限两次查询超时）。持着注册表读锁与
    /// 会话锁等它，等于让每次取光标都把按帧取这两把锁的渲染线程堵住。
    pub(crate) fn render_cursor_on(query_tx: &Sender<Query>) -> Option<(u32, u32)> {
        Self::query_on(query_tx, Query::RenderCursor, None, "render_cursor")
    }

    /// 取走邮箱里的最新一帧：邮箱只保留最新帧，故取到的必是当前画面。
    pub fn receive_cell_data(&self) -> Option<(Vec<CellData>, CursorInfo)> {
        self.cell_data_mailbox.take()
    }
}
