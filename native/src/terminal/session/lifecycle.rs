//! 生命周期：退出上报、进程组收尾与线程回收。
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use super::{ExitCodeSlot, OUTPUT_CHANNEL_BOUND, ReportedExit, Session};

/// 拆卸会话时 SIGHUP 与 SIGKILL 之间的宽限期，同时作为读/等待线程的 join 超时。
const TRAILING_EXIT_GRACE: Duration = Duration::from_millis(50);
/// 选择收尾信号的目标：`kill(-pgid)` 覆盖整个前台进程组，使 shell 的子进程一并退出。
/// 组号下界 MUST 大于 1——`-1` 会把信号发给本进程有权发信号的所有进程；组号也 MUST NOT
/// 是本进程自身的组——子进程 fork 后尚未 `setsid` 时与本进程同组，组杀会连带杀死自己
/// （设备实证 SIGKILL 自杀）。本进程组号取不到时一律退化为直杀子进程：确认不了就不组杀。
/// 纯函数，使这条 guard 无需真实子进程即可测试。
pub(super) fn exit_signal_target(
    pid: nix::unistd::Pid,
    child_pgid: Option<nix::unistd::Pid>,
    own_pgid: Option<nix::unistd::Pid>,
) -> nix::unistd::Pid {
    let Some(own_pgid) = own_pgid else {
        return pid;
    };
    child_pgid
        .filter(|pgid| pgid.as_raw() > 1 && *pgid != own_pgid)
        .map_or(pid, |pgid| nix::unistd::Pid::from_raw(-pgid.as_raw()))
}

pub(crate) fn join_with_timeout(
    handle: &mut Option<std::thread::JoinHandle<()>>,
    timeout: Duration,
) {
    let Some(handle) = handle.take() else {
        return;
    };
    if wait_finished(&handle, timeout) {
        join_finished(handle);
        return;
    }
    log::warn!("session: thread did not exit within {timeout:?}, retrying up to 3×");
    for _attempt in 0..3 {
        if wait_finished(&handle, Duration::from_millis(100)) {
            join_finished(handle);
            return;
        }
    }
    log::error!("session: thread failed to exit after retries — DETACHING (resource leak)");
    // 句柄在此被丢弃 → 分离
}

/// 在截止前轮询句柄完成，完成返回 true（不消费句柄，join 留给调用方）。
fn wait_finished(handle: &std::thread::JoinHandle<()>, timeout: Duration) -> bool {
    const POLL_STEP: Duration = Duration::from_millis(10);
    let deadline = std::time::Instant::now() + timeout;
    while std::time::Instant::now() < deadline {
        if handle.is_finished() {
            return true;
        }
        std::thread::sleep(POLL_STEP);
    }
    false
}

/// join 已完成的句柄并报告 panic。
fn join_finished(handle: std::thread::JoinHandle<()>) {
    if let Err(error) = handle.join() {
        log::error!("session: thread panicked: {:?}", error);
    }
}

impl Session {
    pub fn is_exited(&self) -> bool {
        self.exited.load(Ordering::Acquire)
    }

    pub fn exited_flag(&self) -> Arc<AtomicBool> {
        self.exited.clone()
    }

    /// 原子地标记退出事件已上报给 Kotlin 侧，仅首个调用者返回 true——pollEvent 的
    /// 逐帧后台扫描据此保证每次退出只上报一次。
    pub fn mark_exit_reported(&self) -> bool {
        !self.exit_reported.swap(true, Ordering::AcqRel)
    }

    /// 取出「已可上报的退出」：`None` 表示子进程还没退出到能上报的程度，调用方
    /// 每帧再问一次即可（不阻塞、不睡眠）。
    ///
    /// 上报条件与 `mark_exit_reported` 合成一个动作，二者因此不可能不一致。原先 FFI
    /// 层先 `mark_exit_reported()` 再去忙等退出码，等不到就上报一个伪造的 0；而标志
    /// 已锁存、退出事件又不会重发——被 OOM kill（137）之类的会话会永远显示成正常退出 0。
    /// 现在「码到手」才上报：拿到的一定是真码，且退出事件仍恰好发一次。
    pub fn take_reported_exit(&self) -> Option<ReportedExit> {
        if !self.is_exited() {
            return None;
        }
        let reported = {
            let guard = self.exit_code.lock();
            match *guard {
                ExitCodeSlot::Pending => return None,
                ExitCodeSlot::Code(code) => ReportedExit::Code(code),
                ExitCodeSlot::Unknown => ReportedExit::Unknown,
            }
        };
        if self.mark_exit_reported() {
            Some(reported)
        } else {
            None
        }
    }
    pub fn request_exit(&mut self) {
        // 等待线程已回收子进程时不再发信号：PID 可能已被复用，直杀会误伤无关进程。
        let already_exited = self.exited.swap(true, Ordering::AcqRel);
        // 先断开输出通道：flume 旧接收端一析构，阻塞的 `output_tx.send` 即返回
        // Err 走到「output channel closed」退出分支，等待线程也随 exited 标志退出。
        // 换入的是空通道，故 `poll_pty_output` 等后续调用仍安全。自然退出后
        // 也要断开：读取线程可能正卡在满通道的 send 上。
        self.output_rx = flume::bounded::<Vec<u8>>(OUTPUT_CHANNEL_BOUND).1;
        let pid = self.pty.child_pid();
        if already_exited || pid.as_raw() <= 0 {
            return;
        }
        // 优先组杀：`kill(-pgid)` 把信号发给整个前台进程组，使 shell 的子进程
        // （管道、前台组内的后台作业）一并退出。组不可用时退化为直杀子进程，
        // 两者只差信号目标，故把目标选好后走同一条「HUP → CONT → 让步 → KILL」。
        let child_pgid = nix::unistd::getpgid(Some(pid));
        if matches!(child_pgid, Err(nix::errno::Errno::ESRCH)) {
            log::debug!("session exit: child {} already exited", pid.as_raw());
            return;
        }
        let own_pgid = nix::unistd::getpgid(None).ok();
        let group = exit_signal_target(pid, child_pgid.as_ref().ok().copied(), own_pgid);
        match (child_pgid, own_pgid) {
            (Ok(child), Some(own)) if child == own => {
                log::debug!(
                    "session exit: child {} shares our process group",
                    pid.as_raw()
                );
            }
            (Ok(_), Some(_)) => {
                log::debug!("session exit: signalling process group {}", -group.as_raw());
            }
            (Err(error), _) => {
                log::debug!(
                    "session exit: getpgid({}) failed: {error}; killing child directly",
                    pid.as_raw()
                );
            }
            (Ok(_), None) => {
                log::debug!(
                    "session exit: own process group unavailable; killing child {} directly",
                    pid.as_raw()
                );
            }
        }
        Self::terminate_target(pid, group);
    }

    /// 按 `SIGHUP → SIGCONT → 让步 → SIGKILL` 收掉目标：先给前台组一次体面退出，
    /// 让它有机会刷盘与复原终端；宽限期后仍存活即强杀。组信号失败时逐级退到子进程。
    fn terminate_target(pid: nix::unistd::Pid, group: nix::unistd::Pid) {
        // 组不可用时 `group` 就是 `pid`，标签据实切换，否则日志会把直杀子进程写成
        // 「打给了进程组」，排障时会被直接带偏。
        let (first, label) = if group == pid {
            (pid, "child")
        } else {
            (group, "pgid")
        };
        // 退路只在目标确实不同时才发：退化路径下 `first` 就是 `pid`，重发等于把
        // 同一个信号再打一遍（失败时日志会出现两条一模一样的记录）。
        let fallback = |signal: nix::sys::signal::Signal, what: &str| {
            if first != pid {
                deliver_signal(pid, signal, what);
            }
        };
        if !deliver_signal(
            first,
            nix::sys::signal::Signal::SIGHUP,
            &format!("SIGHUP to {label}"),
        ) {
            fallback(nix::sys::signal::Signal::SIGHUP, "SIGHUP to child");
        }
        deliver_signal(
            first,
            nix::sys::signal::Signal::SIGCONT,
            &format!("SIGCONT to {label}"),
        );
        std::thread::sleep(TRAILING_EXIT_GRACE);
        if !deliver_signal(
            first,
            nix::sys::signal::Signal::SIGKILL,
            &format!("SIGKILL to {label}"),
        ) {
            fallback(nix::sys::signal::Signal::SIGKILL, "SIGKILL to child");
        }
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        self.request_exit();
        join_with_timeout(&mut self.reader_handle, TRAILING_EXIT_GRACE);
        join_with_timeout(&mut self.wait_handle, TRAILING_EXIT_GRACE);
    }
}
/// 向进程或进程组发信号并记录失败原因（`Drop` 无法返回错误）。
///
/// `ESRCH` 表示目标已退出，是拆卸过程的正常结局，不记警告；其余 errno 必须留痕，
/// 否则信号丢失会表现为“会话关闭后 shell 仍在后台跑”。返回信号是否送达，供组杀
/// 失败时退化为直杀子进程。
fn deliver_signal(
    target: nix::unistd::Pid,
    signal: nix::sys::signal::Signal,
    description: &str,
) -> bool {
    match nix::sys::signal::kill(target, signal) {
        Ok(()) => true,
        Err(error) => {
            if error != nix::errno::Errno::ESRCH {
                log::warn!("session drop: {description} {}: {error}", target.as_raw());
            }
            false
        }
    }
}
