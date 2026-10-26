//! PTY 主端：可克隆的写入面，与会话解耦。
use parking_lot::Mutex;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use crate::terminal::pty::Pty;

use super::SessionError;

/// PTY 主端：可克隆的写入面，与 [`Session`](super::Session) 解耦。
///
/// 为什么必须能脱离会话锁使用：`Pty::write_all` 在子进程不读 stdin 时最多等
/// `pty::WRITE_DRAIN_TIMEOUT`（5s），且每有部分写入就重置期限，故持锁时间**无上界**；
/// 而会话锁每帧都被渲染与事件收割取得——持锁写入会把整台终端（渲染、事件、输入）
/// 冻住同样长的时间。写入只需主端 fd，故句柄单独克隆、锁外完成。
///
/// 两把锁的分工是刻意的：
/// - **读侧无锁**：`resize` / `set_pixel_size` / `child_pid` 只是 ioctl 或取字段，
///   直接经 `Arc<dyn Pty>` 完成。它们一旦排在无上界的写入后面，UI 线程上的
///   旋转/输入法变化就会跟着一起卡住。
/// - **仅写入之间互斥**：同一 PTY 的字节序要求写入彼此串行，故另设一把只锁写入
///   的锁——它挡不住别的线程，但也不会被只读路径拖住。
#[derive(Clone)]
pub struct PtyMaster {
    /// 只读路径直接经它拿 `&dyn Pty`，不取任何锁（`Pty` 为 `Send + Sync`）。
    pty: Arc<dyn Pty>,
    /// 只串行化写入之间的字节序，与读侧无关。
    write_order: Arc<Mutex<()>>,
    exited: Arc<AtomicBool>,
    /// 子进程 pid，构造期取一次。
    ///
    /// `request_exit` 必须在**不取互斥量**的前提下读到它：那条路经 `destroySession`
    /// 跑在 UI 线程上，而互斥量此刻可能被 `PtyMaster::write` 持住——后者要等子进程
    /// 消费，且每有部分写入就重置期限，故持锁时间**无上界**。子进程退出后该路若被
    /// 堵住，`Session::drop` 的 `join_with_timeout` 随之超时并分离等待线程。
    ///
    /// 进程组在 spawn 之后不再改变，故这个值与经 `Pty::child_pid` 现取完全等价。
    child_pid: nix::unistd::Pid,
}

impl PtyMaster {
    pub(super) fn new(pty: Box<dyn Pty>, exited: Arc<AtomicBool>) -> Self {
        let child_pid = pty.child_pid();
        Self {
            pty: Arc::from(pty),
            write_order: Arc::new(Mutex::new(())),
            exited,
            child_pid,
        }
    }

    /// 子进程 pid：不取互斥量，故可在写入进行中安全调用。
    pub fn child_pid(&self) -> nix::unistd::Pid {
        self.child_pid
    }

    /// 更新 PTY winsize。**不取锁**：只是 ioctl，与无上界的写入无关。
    pub fn resize(&self, rows: u16, cols: u16) -> Result<(), crate::terminal::pty::PtyError> {
        self.pty.resize(rows, cols)
    }

    /// 更新 TIOCSWINSZ 的像素字段。**不取锁**，同上。
    pub fn set_pixel_size(
        &self,
        width: u16,
        height: u16,
    ) -> Result<(), crate::terminal::pty::PtyError> {
        self.pty.set_pixel_size(width, height)
    }

    /// 以给定排空期限写入。**取写入锁**以保字节序——VT 应答与用户输入共用主端。
    pub fn write_with_timeout(
        &self,
        data: &[u8],
        drain_timeout: std::time::Duration,
    ) -> std::io::Result<()> {
        let _order = self.pty_write_order();
        self.pty.write_all(data, drain_timeout, true).1
    }

    /// [`Self::write_with_timeout`] 的带回写条数版：返回**已写出的字节数**（含出错
    /// 时的部分写），调用方据此只把未写出的部分退回队列。VT 应答续写用；用户输入走
    /// 上面那条——它不容忍重发，多写一次就是多打一行命令。
    ///
    /// 排空期限同时是**写入锁**的取锁期限：逐帧续写传进来的是整帧剩余墙钟预算，
    /// 而用户输入传满额。上一条路径的写入锁在别处被等满 `WRITE_DRAIN_TIMEOUT` 时，
    /// 不设限地等锁会让「整段持会话锁以墙钟为界」变成「以条数 × 排空期限为界」。
    ///
    /// `None` 表示期限内没取到写入锁，调用方须把整条退回队列下帧续写。
    pub fn write_with_timeout_counted(
        &self,
        data: &[u8],
        drain_timeout: std::time::Duration,
    ) -> Option<(usize, std::io::Result<()>)> {
        let _order = self.write_order.try_lock_for(drain_timeout)?;
        Some(self.pty.write_all(data, drain_timeout, false))
    }

    pub(super) fn pty_write_order(&self) -> parking_lot::MutexGuard<'_, ()> {
        self.write_order.lock()
    }

    /// 写入子进程 stdin。会话已退出即拒绝（`SessionError::Closed`）——
    /// 退出后写入只会得到 EIO，读起来像「写失败」而非「会话已关」。
    pub fn write(&self, data: &[u8]) -> Result<(), SessionError> {
        if self.exited.load(Ordering::Acquire) {
            return Err(SessionError::Closed);
        }
        self.write_with_timeout(data, crate::terminal::pty::WRITE_DRAIN_TIMEOUT)
            .map_err(SessionError::Io)
    }
}
