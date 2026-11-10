//! 命令下发：把 VT 写入与状态变更经命令通道交给 VT 线程执行。

use std::sync::atomic::Ordering;

use super::super::GhosttyTerminal;
use super::super::commands::{Command, ViewportTarget};
use super::super::types::CursorStyle;
use super::sanitize_vt_input;

impl GhosttyTerminal {
    /// 命令入队失败时的统一告警。
    ///
    /// 命令通道有界（容量见 [`COMMAND_CHANNEL_CAPACITY`]），VT 线程忙于长命令时
    /// 入队会失败。这是真实异常——被丢弃的命令不会重放——但逐次告警会把 logcat
    /// 刷成洪水（实测一次输出高峰刷出两万余行，淹没其余诊断），故按 500ms 窗口
    /// 限频，并带上窗口内累计丢弃次数：限频吞掉的是条数，不是次数。
    pub(super) fn warn_command_dropped(
        &self,
        operation: &str,
        error: &flume::TrySendError<Command>,
    ) {
        if let Some(count) = self.cmd_drop_log.record_events(1) {
            log::warn!(
                "ghostty_terminal: 命令通道已满，丢弃 {count} 次命令（最近一次 {operation}: {error}）"
            );
        }
    }

    /// 把字节送入 VT 解析：先做输入清洗，再分片直透（不追加闭合序列）。
    pub fn vt_write(&mut self, data: &[u8]) {
        let sanitized = sanitize_vt_input(data);
        let mut buffer = Vec::with_capacity(data.len() + 4);
        buffer.extend_from_slice(&sanitized);
        // 分片直透：上游解析器在同一 Terminal 对象上跨调用保持状态，此处不得追加
        // ST/SGR 提前闭合（会截断合法跨块 OSC 并洗掉颜色）。用 try_send：VT 线程卡住时
        // 不得无限期阻塞调用方（与 pty_write 同策略）。
        if let Err(error) = self.cmd_tx.try_send(Command::Write(buffer)) {
            self.warn_command_dropped("vt_write", &error);
        }
    }

    /// 把一段 PTY 输出按 LF→CRLF 规则成形为命令载荷，返回待入队的字节。
    ///
    /// 只做字节变换、不入队：入队失败时需要把**同一份**载荷原样交还调用方重试，
    /// 再次调用本函数会重复插入 `\r`（`\n` 已变成 `\r\n`，末尾字节也随之改变）。
    fn build_pty_write_buffer(&mut self, data: &[u8]) -> Vec<u8> {
        let mut buffer = Vec::with_capacity(data.len() + 4);
        // 用上次调用的末字节识别跨块拆分的 `\r`/`\n`（Linux PTY 输出常见）；否则
        // LF→CRLF 转换会多插入一个 `\r`，产生 `\r\r\n`。
        let mut prev: u8 = self.last_pty_write_byte;
        for &raw in data {
            // 仅剔除 NUL（与 `vt_write` 同策略，二进制安全）：0xF8 以上字节在合法
            // UTF-8 中不会出现，但在 Kitty 直接 RGB 等二进制载荷中合法，改为空格
            // 等于静默损坏图像，此处直透交由上游解析器处理。
            if raw == 0x00 {
                continue;
            }
            // 裸 LF 转成 CRLF，但仅当其前一个字节不是 CR；否则本已含 CRLF 的输入
            // （PTY 输出常见）会变成 CRCRLF，多出一个回车。
            if raw == b'\n' && prev != b'\r' {
                buffer.push(b'\r');
            }
            buffer.push(raw);
            prev = raw;
        }
        // 分片直透：上游解析器在同一 Terminal 对象上跨调用保持状态，
        // CSI/OSC/DCS 分片由上游增量重组，此处不得提前闭合（ST 自动闭合
        // 会截断合法跨块 OSC，实测分片 OSC 52 被截为空内容）。
        self.last_pty_write_byte = prev;
        buffer
    }

    /// 投递**已成形**的 PTY 载荷；通道满时原样交还调用方重试。
    ///
    /// 重试路径 MUST 用它而非 [`Self::try_pty_write`]：成形已推进过
    /// `last_pty_write_byte`，二次成形会把「上块以 `\r` 结尾、本块以 `\n` 开头」的
    /// 合法 CRLF 重新算一遍并补出一个多余的 `\r`。
    pub fn resend_pty_write(&mut self, buffer: Vec<u8>) -> Result<(), Vec<u8>> {
        match self.cmd_tx.try_send(Command::Write(buffer)) {
            Ok(()) => Ok(()),
            // flume 把未送达的消息原样带回：正是需要重试的那份载荷，无需重新成形。
            Err(flume::TrySendError::Full(Command::Write(buffer))) => Err(buffer),
            Err(flume::TrySendError::Full(_)) => unreachable!("sent Command::Write"),
            Err(flume::TrySendError::Disconnected(_)) => {
                // VT 线程已退出：没有重试对象。此处只留一条错误，由会话层按
                // 「终端已死」处理；静默返回会让调用方误以为数据已送达。
                log::error!("ghostty_terminal: pty_write: VT command channel disconnected");
                Ok(())
            }
        }
    }

    /// 入队一段 PTY 输出；通道满时把**成形后的载荷**原样交还给调用方。
    ///
    /// 这是 PTY → VT 链路的背压点。调用方（会话的输出收割）把交还的块留到下一帧
    /// 重试，并停止继续从 PTY 通道取数据——有界通道逐级向上游传递压力，子进程最终
    /// 因 PTY 缓冲写满而阻塞。绝不丢字节：终端状态与 PTY 流必须逐字节一致，
    /// 丢一块就是屏幕内容与真实输出永久分叉（旧实现如此，且每丢一块刷一行日志）。
    ///
    /// 返回 `Ok(())` 表示已被 VT 线程接收（或通道已断开，重试无意义）。
    pub fn try_pty_write(&mut self, data: &[u8]) -> Result<(), Vec<u8>> {
        let buffer = self.build_pty_write_buffer(data);
        self.resend_pty_write(buffer)
    }

    /// 写入 PTY 输出（无重试槽的调用方）。
    ///
    /// 会话路径 MUST 用 [`Self::try_pty_write`] 把未送达的块留待下一帧重试；本方法
    /// 供测试与一次性馈入使用，通道满时按窗口累计告警后丢弃——逐次告警会在输出
    /// 高峰刷出数万行（见 `logcat-hygiene`）。
    pub fn pty_write(&mut self, data: &[u8]) {
        if self.try_pty_write(data).is_err()
            && let Some(count) = self.cmd_drop_log.record_events(1)
        {
            log::warn!(
                "ghostty_terminal: 命令通道已满，丢弃 {count} 次 pty_write（调用方未提供重试槽）"
            );
        }
    }

    /// 换主题，主题原样下发（无 `colors.properties` 覆盖）。
    pub fn set_theme(&self, theme: &'static crate::terminal::theme::Theme) {
        self.set_theme_palette(theme.name, theme.palette.clone());
    }

    /// 下发**已合并**的调色板。
    ///
    /// 合并（主题 + `colors.properties` 覆盖）在下发前完成：命令只携带一份调色板，
    /// 终端默认色、ANSI 调色板、光标色与搜索高亮色都从它取，不存在第二份颜色副本。
    pub fn set_theme_palette(&self, name: &'static str, palette: crate::terminal::theme::Palette) {
        // try_send：与 resize 同非阻塞策略。
        if let Err(error) = self.cmd_tx.try_send(Command::SetTheme { name, palette }) {
            self.warn_command_dropped("set_theme", &error);
        }
    }

    /// 设置默认光标样式（`termux.properties` 的 `terminal-cursor-style`）。
    ///
    /// 只是默认值：程序用 DECSCUSR 指定过样式时以程序为准。
    pub fn set_cursor_style(&self, style: CursorStyle) {
        if let Err(error) = self.cmd_tx.try_send(Command::SetCursorStyle(style)) {
            self.warn_command_dropped("set_cursor_style", &error);
        }
    }

    /// 下发视口滚动命令。成功返回 true。
    ///
    /// 每次下发分配一个请求序号（进程级单调递增）：VT 处理命令后把序号随帧回传，
    /// 渲染侧据此判断收到的帧是否早于在途的滚动命令——在途期间到达的旧视口帧
    /// 不得呈现（旧视口配新余量会让内容先上跳半行再下跳整行）。
    pub fn scroll_viewport(&self, target: ViewportTarget) -> bool {
        let request_id = super::super::commands::next_scroll_request_id();
        if let Err(error) = self
            .cmd_tx
            .try_send(Command::ScrollViewport { target, request_id })
        {
            self.warn_command_dropped("scroll_viewport", &error);
            return false;
        }
        // 只在确认送达后推进「最新请求序号」：未送达的序号不得让渲染侧把
        // 后续的帧判成陈旧（那会让滚动后的帧永远不被呈现）。
        self.latest_scroll_request_id
            .store(request_id, Ordering::Release);
        true
    }

    /// 最近一次**已送达**的滚动请求序号（0 = 从未下发过滚动命令）。
    ///
    /// 渲染侧拿它与帧携带的「VT 已处理的最后一个序号」比较：帧的序号更小即
    /// 说明该帧早于在途的滚动命令。
    pub fn latest_scroll_request_id(&self) -> u64 {
        self.latest_scroll_request_id.load(Ordering::Acquire)
    }

    /// VT 线程接受 resize 命令时为 true。为 false 表示网格**未**缩放（调用方的
    /// `pty.resize` 可能已更新 PTY）；调用方不得缓存新尺寸，以便下次 resize 事件重试。
    pub fn resize(&mut self, rows: u32, cols: u32) -> bool {
        // 用 try_send 而非 send：VT 线程卡住时不得阻塞调用方（switchSession 在此调用
        // 期间持有 Kotlin sessionLock）。注意：被丢弃的 resize **不会重放**——PTY/网格
        // 保持旧尺寸直到下次 resize 事件（输入法变化、旋转、设置变更、切换会话）。
        if let Err(error) = self.cmd_tx.try_send(Command::Resize { rows, cols }) {
            self.warn_command_dropped("resize", &error);
            return false;
        }
        true
    }

    /// 上一次入队的 resize 是否在 VT 线程上真的失败过（网格仍是旧尺寸）。
    /// 调用方据此让下一次 resize 事件重试，而非按「尺寸未变」短路。
    pub fn resize_failed(&self) -> bool {
        self.resize_failed.load(Ordering::Acquire)
    }

    /// 测试钩子：直接置位/清除 [`Self::resize_failed`]。
    pub fn set_resize_failed_for_test(&self, failed: bool) {
        self.resize_failed.store(failed, Ordering::Release);
    }

    /// 更新单元格像素尺寸：Kitty 图像几何与鼠标像素映射依赖它。
    pub fn set_cell_pixel_size(&self, cell_width: u32, cell_height: u32) {
        if let Err(error) = self.cmd_tx.try_send(Command::SetCellPixelSize {
            cell_width,
            cell_height,
        }) {
            self.warn_command_dropped("set_cell_pixel_size", &error);
        }
    }

    /// 请求 RIS 全重置：恢复初始状态并清空回滚。
    pub fn reset(&self) {
        if let Err(error) = self.cmd_tx.try_send(Command::Reset) {
            self.warn_command_dropped("reset", &error);
        }
    }

    /// 请求按当前状态重推一帧 CellData，不作任何状态改变。内容去重对显式请求不适用，
    /// 故去重基线在 VT 线程上被失效，帧必定到达（邮箱只留最新帧，重推覆盖旧帧）。
    pub fn request_repaint(&self) {
        if let Err(error) = self.cmd_tx.try_send(Command::Repaint) {
            self.warn_command_dropped("request_repaint", &error);
        }
    }

    /// 安装终端持有的线性活动选区（跟踪引用，随滚动/输出/重排跟随文本）。
    /// 坐标为绝对网格行（0 = 回滚顶部）与列。
    /// `try_send` 非阻塞：VT 线程卡住时丢弃而非阻塞调用方（与 resize 同策略）。
    pub fn set_selection(&self, start: (u32, u32), end: (u32, u32)) {
        if let Err(error) = self.cmd_tx.try_send(Command::SetSelection { start, end }) {
            self.warn_command_dropped("set_selection", &error);
        }
    }

    /// 清除当前选区。
    pub fn clear_selection(&self) {
        if let Err(error) = self.cmd_tx.try_send(Command::ClearSelection) {
            self.warn_command_dropped("clear_selection", &error);
        }
    }
}
