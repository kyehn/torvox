//! Ghostty 终端引擎：VT 解析、命令分发与公开 API。
//! 以命令方式在 PTY 读取线程与渲染线程间通信。

use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex, Once};
use std::thread;
use std::time::Duration;

use flume::Sender;

mod commands;
mod internal;
mod public_api;
mod types;
pub(crate) mod unicode_placement;

pub use commands::Command;
pub use commands::Query;
pub use types::*;

/// VT 线程析构等待上限，与会话拆卸宽限一致，避免无界阻塞。
const TERMINAL_THREAD_JOIN_TIMEOUT: Duration = Duration::from_millis(50);

/// 安装进程级 PNG 解码器：Kitty Graphics Protocol 的 PNG 载荷在进程内解码为 RGBA8。
///
/// 上游把解码器回调存为 libghostty 的全局量且不做同步，要求在任何线程进入
/// libghostty 之前设置完毕；各 VT 线程各自安装会与其他会话的 libghostty 调用相撞。
/// 用上游解码器：它按 libghostty 的分配器上限先取缓冲区再解码，未经该上限的尺寸
/// 不会被 PNG 头放大成巨额分配。
#[cfg(any(target_os = "android", target_os = "linux"))]
fn install_png_decoder() {
    static INSTALL: Once = Once::new();
    INSTALL.call_once(|| {
        // SAFETY: 构造函数在动态库加载时执行，此前没有任何线程进入 libghostty。
        if let Err(error) = unsafe {
            libghostty_vt::kitty::graphics::set_png_decoder(Some(Box::new(
                libghostty_vt::kitty::graphics::RustPngDecoder::new(),
            )))
        } {
            log::error!("ghostty_terminal: set_png_decoder failed: {error}");
        }
    });
}

/// 库加载即安装 PNG 解码器，理由见 [`install_png_decoder`]。
#[cfg(any(target_os = "android", target_os = "linux"))]
#[used]
#[unsafe(link_section = ".init_array")]
static PNG_DECODER_INIT: extern "C" fn() = {
    extern "C" fn init() {
        install_png_decoder();
    }
    init
};

pub struct GhosttyTerminal {
    pub(crate) cmd_tx: Sender<Command>,
    pub(crate) query_tx: Sender<Query>,
    pub(crate) cell_data_rx: Option<flume::Receiver<(Vec<CellData>, CursorInfo)>>,
    /// 上游 OSC 52 回调事件接收端（VT 线程经 on_clipboard_write 推送）。
    /// session 在 flush 后收割到锁存槽；BDD 直接轮询断言。
    pub(crate) clipboard_rx: flume::Receiver<(String, String)>,
    pub(crate) handle: Option<thread::JoinHandle<()>>,
    pub(crate) pty_write_responses: Arc<Mutex<Vec<Vec<u8>>>>,
    /// `pty_write()` 末次写入的字节，用于识别 `\r`/`\n` 跨写入块拆分，避免多余的 `\r\r\n`。
    pub(crate) last_pty_write_byte: u8,
    /// `Terminal::active_screen() == Alternate` 的无锁镜像，由 VT 线程每帧更新。
    /// 供 Android 输入路径免阻塞 RPC 检出备用屏（vim/less/htop），使触摸滚动
    /// 转发为滚轮转义序列而非滚动本地回滚（备用屏会吞掉滚轮）。
    pub(crate) alt_screen_active: Arc<AtomicBool>,
    /// 鼠标上报（DECSET 1000/1002/1003）是否启用的无锁镜像，同样由 VT 线程
    /// 每帧刷新。触摸拖动每个事件都要判断一次是否转发滚轮给远端，读原子量
    /// 才能让 UI 线程完全不碰查询通道。
    pub(crate) mouse_tracking_active: Arc<AtomicBool>,
    /// VT 线程上 `Terminal::resize` 失败留下的标记：网格仍是旧尺寸，而
    /// `Session::resize` 已按「命令已入队」把缓存尺寸发布成新值。调用方据此
    /// 让下一次 resize 事件（即使尺寸相同）重试，修复永久分歧。
    pub(crate) resize_failed: Arc<AtomicBool>,
    /// 行级抽取的可观测计数，仅测试使用。
    #[cfg(test)]
    pub(crate) cache_stats: internal::CacheStats,
    /// Kitty 占位符扫描计数（复用行 / 扫描行），仅测试使用。
    #[cfg(test)]
    pub(crate) kitty_stats: internal::CacheStats,
}

/// Terminate 投递的短时等待：try_send 在队列满时失败，但 VT 线程按批排空
/// 有界队列，短等待通常即可送达；超时后放弃，析构继续走限时 join。
const TERMINATE_SEND_TIMEOUT: Duration = Duration::from_millis(50);

/// 投递 `Command::Terminate` 并报告是否送达，`timeout` 为队列满时的等待上限。
///
/// 队列满是常态而非异常：VT 线程按批排空有界队列，一次 `try_send` 失败就放弃
/// 等于把终止推迟到信道彻底断开，`join` 随后超时、线程被 detach，回滚缓冲与
/// 终端存储的回收都被推到不确定的更晚时刻。故失败时给一次有界等待。
fn deliver_terminate(cmd_tx: &Sender<Command>, timeout: Duration) -> bool {
    match cmd_tx.try_send(Command::Terminate) {
        Ok(()) => true,
        Err(error) => {
            log::warn!(
                "ghostty_terminal: try_send Terminate failed ({error}), retrying with timeout"
            );
            cmd_tx.send_timeout(Command::Terminate, timeout).is_ok()
        }
    }
}

impl Drop for GhosttyTerminal {
    fn drop(&mut self) {
        if !deliver_terminate(&self.cmd_tx, TERMINATE_SEND_TIMEOUT) {
            log::error!("ghostty_terminal: failed to deliver Command::Terminate");
        }
        crate::terminal::session::join_with_timeout(&mut self.handle, TERMINAL_THREAD_JOIN_TIMEOUT);
    }
}

#[cfg(test)]
mod kitty_cache_tests;
#[cfg(test)]
mod tests;

#[cfg(test)]
mod input_output_tests;

#[cfg(test)]
mod snapshot_cache_unit_tests;

#[cfg(test)]
mod key_encoding_tests;
