//! 输出收割：PTY 输出处理、VT 副作用与剪贴板应答。
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use super::{Session, SessionError};

/// 待上报 OSC 52 剪贴板写入的上限。渲染暂停期间（设置页/输入法弹出）`poll_clipboard`
/// 停调，而程序可继续发 OSC 52；超出即丢最旧，使内存不随「上游发得多快」增长。
pub(super) const MAX_CLIPBOARD_TEXT_QUEUE: usize = 8;

/// 单帧最多回写多少条 VT 应答。
///
/// 上界而非下界：正常终端一帧内至多一两条（DSR 一次、DA 一次），而病态输出
/// （`printf '\033[6n'` 循环）一帧能产生上万条。写完即止，剩余的留在
/// [`Session::pty_write_back_queue`] 下帧续写。它与
/// `WRITE_BACK_TIMEOUT` 的墙钟闸各管一件事：条数封顶内存与单帧工作量，
/// 墙钟封顶**持会话锁**的时间——主端满时每条都会等满排空期限，只靠条数封顶
/// 仍能按住渲染与看门狗近一秒。
pub(super) const MAX_PTY_WRITES_PER_FRAME: usize = 16;

/// 待回写 VT 应答的队列上限。
///
/// 每帧只写走 [`MAX_PTY_WRITES_PER_FRAME`] 条，生产速率远高于它时（子进程不读
/// stdin，病态程序又持续查询）队列净增长，故显式封顶丢最旧——应答是请求-响应
/// 语义，不被消费的应答不会被回头读，无限保留只是白占内存。取值给足突发余量：
/// 一整帧的输出块（10 × 8KiB）能产生的 DSR 应答不足其四分之一。
const MAX_PENDING_PTY_WRITES: usize = 256;

impl Session {
    /// 每会话帧处理的最大 VT 输出块数，用于 PTY 输出洪水时限制渲染线程延迟。
    const MAX_CHUNKS_PER_FRAME: u32 = 10;

    /// [`Self::focus_event`] 中 DECSET 1004 模式查询的超时。调用方持会话锁，
    /// 而该锁每帧都被渲染与事件收割取得，故 VT 线程卡住时必须快速失败。
    /// PTY 写入在同一把锁内，其排空上限是 `pty::WRITE_DRAIN_TIMEOUT`。
    pub(super) const FOCUS_MODE_QUERY_TIMEOUT_MS: u64 =
        4 * crate::terminal::ghostty_terminal::VT_IDLE_TICK_MS;

    /// 处理来自 PTY 读取线程的终端输出：读取 VT 输出、更新终端状态并排空回写应答。
    /// 处理过任何 VT 数据时返回 true。
    pub(crate) fn poll_pty_output(&mut self, max_chunks: u32) -> bool {
        let mut count = 0u32;
        while let Ok(data) = self.output_rx.try_recv() {
            let snap = self.output_processor.process(&data);

            for selection in snap.clipboard_reads {
                self.clipboard_read.lock().push_back(selection);
            }
            self.terminal.pty_write(&snap.filtered);
            count += 1;
            // 限制每帧处理量，避免单次渲染调用长时间持有会话锁。剩余块在下一渲染帧处理，
            // 无正确性代价——VT 线程按 FIFO 处理命令。
            if count >= max_chunks {
                log::trace!(
                    "poll_pty_output: hit cap of {} chunks, {} remain",
                    max_chunks,
                    self.output_rx.len(),
                );
                // 即使走到封顶路径也排空回写应答：输出洪水不得饿死 DECRPM/DSR/DA 应答，
                // 否则子应用会无限期等待。
                self.harvest_vt_side_effects();
                return true;
            }
        }
        if count > 0 {
            log::trace!("poll_pty_output: processed {count} chunks");
            self.harvest_vt_side_effects();
            true
        } else {
            // 本帧没有新输出，但 VT 线程可能仍在处理此前投递的命令：其回调事件
            // （OSC 52 写入）与回写应答此刻可能刚好就绪。仍然收割一次，否则
            // 这些副作用要一直等到下一批输出到来才上报——终端安静时（恰恰是用户
            // 复制完内容的那一刻）就永远不发生。
            self.harvest_vt_side_effects();
            false
        }
    }

    /// 收割 VT 侧的副作用：回调事件（OSC 52 写入）与 PTY 回写应答（DECRPM/DSR/DA）。
    ///
    /// 两者都是非阻塞取（`try_recv` 与加锁后 `take`），故**不**以 VT 线程的 flush ack
    /// 为条件。原实现把收割挂在 `flush_with_timeout(Duration::ZERO)` 的成功分支上，
    /// 而零期限的 `recv_timeout` 要在 ack 已被 VT 线程送出的一瞬才可能命中，实测 32 次
    /// 连续尝试 0 次成功（`zero_timeout_flush_observes_the_ack`）——于是这条分支实际是
    /// 死代码：OSC 52 写入永远不收割（用户复制内容后剪贴板不更新），
    /// DECRPM/DSR/DA 应答也永不写回 PTY（子应用无限期等待设备状态查询）。
    ///
    /// 丢弃 flush 的代价是无 Synchronization：VT 线程尚未处理完时取到空，下次取到。
    /// 收割是每帧重复的拉取式操作，事件不会因此丢失，只是可能晚一帧。
    fn harvest_vt_side_effects(&mut self) {
        self.drain_callback_events();
        self.drain_pty_write_back();
    }

    /// 把待处理的 VT 应答（DECRPM、DSR、DA 等）回写到子 PTY。VT 引擎负责缓冲，子进程在等它们。
    ///
    /// 每帧至多写 [`MAX_PTY_WRITES_PER_FRAME`] 条，写不完的（含失败的那条）留在
    /// [`Self::pty_write_back_queue`] 下帧续写。**首次写失败即停**：一次失败已说明
    /// 子进程此刻不读 stdin，继续按同一条件逐条重试只会把本帧的持锁时间乘以条数。
    /// 队列封顶丢最旧——应答是请求-响应语义，子进程不消费就永远不会回头读，
    /// 无限保留只是白占内存；丢弃时记错误，不静默。
    pub(super) fn drain_pty_write_back(&mut self) {
        // 新到的应答排在既有队列之后：先前的应答先出去，顺序与引擎产生顺序一致。
        self.pty_write_back_queue
            .extend(self.terminal.drain_pty_write_responses());
        let overflow = self
            .pty_write_back_queue
            .len()
            .saturating_sub(MAX_PENDING_PTY_WRITES);
        if overflow > 0 {
            self.pty_write_back_queue.drain(..overflow);
            log::error!(
                "session: dropped {overflow} of {} VT responses, child is not reading stdin",
                self.pty_write_back_queue.len() + overflow
            );
        }
        // 整段持会话锁的时间以**墙钟**为界而非以条数计：每条各自等满
        // `WRITE_BACK_TIMEOUT` 的话，`条数 × 期限` 在主端满时仍能按住渲染与看门狗
        // 近一秒。逐帧收割且全程持会话锁，故到期即止，余下留队下帧。
        let deadline = std::time::Instant::now() + crate::terminal::pty::WRITE_BACK_TIMEOUT;
        for _ in 0..MAX_PTY_WRITES_PER_FRAME {
            // 把**剩余**预算交给这一条，而不是整份期限：否则每条都能再等满一个
            // WRITE_BACK_TIMEOUT，最坏持锁时间变成两倍，与「以墙钟为界」的说法不符。
            let Some(budget) = deadline.checked_duration_since(std::time::Instant::now()) else {
                return;
            };
            let Some(response) = self.pty_write_back_queue.pop_front() else {
                return;
            };
            log::trace!("poll_pty_output: pty write-back {} bytes", response.len());
            let Some((written, result)) = self.pty.write_with_timeout_counted(&response, budget)
            else {
                // 写入锁被等满剩余预算（用户输入正占着主端）：本条退回队首下帧续写，
                // 与墙钟闸同一语义——到期即止，不拖住会话锁。
                self.pty_write_back_queue.push_front(response);
                return;
            };
            if let Err(error) = result {
                log::error!(
                    "session: PTY write-back failed ({} bytes, {} written): {}",
                    response.len(),
                    written,
                    error
                );
                // 只退回**未写出**的部分：整条退回会让下一帧把已发出的前缀再发一遍，
                // 子应用于是收到两条 DECRPM/DSR 应答；而全部丢掉又让子应用永远等不到
                // 一个完整回答。
                if written < response.len() {
                    self.pty_write_back_queue
                        .push_front(response[written..].to_vec());
                }
                return;
            }
        }
    }

    /// 处理本帧所有可用输出与用户输入；处理过任何 VT 输出时返回 `true`
    /// （调用方应重建显示快照）。
    pub fn process_output(&mut self) -> bool {
        self.poll_pty_output(Self::MAX_CHUNKS_PER_FRAME)
    }

    /// 会话注册表持有的 `new_output` 共享标志：渲染线程直接读它，
    /// 不抢会话锁（PTY 洪流下锁可能被 VT 解析按住数十毫秒）。
    pub fn new_output_flag(&self) -> Arc<AtomicBool> {
        self.output_processor.new_output_flag()
    }

    /// 备用屏激活的无锁镜像：与 `new_output` 同批上报给宿主，
    /// 同样不经过会话锁。
    pub fn alt_screen_active_flag(&self) -> Arc<AtomicBool> {
        Arc::clone(&self.terminal.alt_screen_active)
    }

    /// 收割 VT 线程经上游回调上报的事件（剪贴板写入）到队列/标志位。
    /// 拉取式：每帧重复调用，VT 线程尚未处理完时取到空、下帧再取（见
    /// `harvest_vt_side_effects` 的说明——本路径不 flush）。
    fn drain_callback_events(&self) {
        let mut dropped = 0usize;
        while let Some((_, text)) = self.terminal.poll_clipboard_event() {
            // FIFO 而非单槽：渲染暂停（pollEvent 停调）期间的连续写入必须全部保留，
            // 单槽会让后一次覆盖前一次，用户丢剪贴板内容。
            let mut queue = self.clipboard_text.lock();
            queue.push_back(text);
            // 丢最旧而非拒收最新：队首才是用户已等待最久、即将被上报的那条。
            // 与 clipboard_read 的洪泛削减同口径（见 ffi 的
            // MAX_UNANSWERED_CLIPBOARD_READS_PER_SESSION），只是那边答空串、这边丢弃。
            if queue.len() > MAX_CLIPBOARD_TEXT_QUEUE {
                queue.pop_front();
                dropped += 1;
            }
        }
        // 逐条记没有任何诊断价值（每条都只是同一个「队列满」），却能让病态程序
        // 每秒刷出数十条 logcat——与渲染洪泛、事件队列溢出同款限频。
        if dropped > 0 {
            log::warn!(
                "clipboard queue overflow (>{} pending); dropped {dropped} oldest writes",
                MAX_CLIPBOARD_TEXT_QUEUE,
            );
        }
    }

    /// 轮询 OSC 52 转义序列写入的剪贴板文本：每次取走最早的一条。
    pub fn poll_clipboard(&self) -> Option<String> {
        self.clipboard_text.lock().pop_front()
    }

    /// 取走全部待处理的 OSC 52 剪贴板读取请求（selection 名，按序）。
    /// JNI 层将其转发给宿主应用，并经 [`Session::answer_clipboard_read`] 回传结果。
    pub fn take_clipboard_reads(&self) -> Vec<String> {
        let mut guard = self.clipboard_read.lock();
        guard.drain(..).collect()
    }

    /// 向 PTY 写入 `ESC ] 52 ; <selection> ; <base64> ESC \\` 应答待处理的 OSC 52
    /// 剪贴板读取请求。selection 为空串（惯例默认 `c`）时原样应答，含义由应用自行解释。
    pub fn answer_clipboard_read(
        &mut self,
        selection: &str,
        text: &str,
    ) -> Result<(), SessionError> {
        use base64::Engine;
        let encoded = base64::engine::general_purpose::STANDARD.encode(text.as_bytes());
        let mut response = Vec::with_capacity(selection.len() + encoded.len() + 8);
        response.extend_from_slice(b"\x1b]52;");
        response.extend_from_slice(selection.as_bytes());
        response.push(b';');
        response.extend_from_slice(encoded.as_bytes());
        response.push(0x07); // BEL 终止符（兼容 xterm）
        if self.is_exited() {
            return Err(SessionError::Closed);
        }
        self.pty
            .write_with_timeout(&response, crate::terminal::pty::WRITE_BACK_TIMEOUT)
            .map_err(SessionError::Io)?;
        Ok(())
    }
    /// DECSET 1004 焦点上报：返回待直写子 PTY 的序列，`None` 表示子进程未启用 1004。
    ///
    /// 序列必须**直接**写入子 PTY 而非进入 VT 引擎——引擎的输出流解析器把 `CSI I` 当作
    /// CHT（光标水平制表）、`CSI O` 当作非法 CSI，送入引擎只会把光标移到下一个制表位而非
    /// 通知应用。仅当子进程确实启用了 1004 时才发送（xterm 语义）。查询窗口须短于用户输入
    /// 的排空上限：调用方持会话锁，VT 线程卡住时不得拖满通用查询超时。
    ///
    /// 写入刻意留给调用方在**释放会话锁之后**做：主端满时排空上限是 5s，而会话锁每帧被
    /// 渲染与事件收割取得（理由同 [`PtyMaster`]）。
    pub fn focus_event_bytes(&mut self, focused: bool) -> Option<&'static [u8]> {
        if !self.terminal.mode_get_with_timeout(
            libghostty_vt::terminal::Mode::FOCUS_EVENT,
            std::time::Duration::from_millis(Self::FOCUS_MODE_QUERY_TIMEOUT_MS),
        ) {
            return None;
        }
        Some(if focused { b"\x1b[I" } else { b"\x1b[O" })
    }
}
