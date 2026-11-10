//! 输入编码：把键鼠与粘贴事件编码为终端转义序列。

use flume::Sender;
use libghostty_vt::key;

use super::super::GhosttyTerminal;
use super::super::commands::Query;

impl GhosttyTerminal {
    /// 用 Ghostty 鼠标编码器把鼠标事件（像素位置、动作、按键）编码为终端转义序列。
    /// `cell_width`/`cell_height` 取渲染器的实时单元格尺寸，使像素→单元映射与实际显示一致。
    /// `modifiers` 是上游 `key.Mods` 原始位（Shift/Ctrl/Alt/Super）。
    ///
    /// 鼠标上报未启用（无 DECSET 1000/1002/1003）或编码失败时返回 `Some(空)`，
    /// 由调用方丢弃该事件；仅查询通道卡死时返回 `None`。
    pub fn encode_mouse_event(
        &self,
        position: (f32, f32),
        action: u8,
        button: u8,
        modifiers: u16,
        cell_width: f32,
        cell_height: f32,
    ) -> Option<Vec<u8>> {
        Self::encode_mouse_event_on(
            &self.query_tx,
            position,
            action,
            button,
            modifiers,
            cell_width,
            cell_height,
        )
    }

    /// [`Self::encode_mouse_event`] 的通道外置版：调用方在会话锁内克隆查询通道，
    /// 随后在锁外编码，使每次鼠标移动不持有会话锁与注册表锁等待 VT 应答。
    pub(crate) fn encode_mouse_event_on(
        query_tx: &Sender<Query>,
        position: (f32, f32),
        action: u8,
        button: u8,
        modifiers: u16,
        cell_width: f32,
        cell_height: f32,
    ) -> Option<Vec<u8>> {
        Self::query_on(
            query_tx,
            |tx| Query::EncodeMouseEvent {
                position,
                action,
                button,
                modifiers,
                cell_width,
                cell_height,
                tx,
            },
            Vec::new(),
            "encode_mouse_event",
        )
        .into()
    }

    /// 把 `repeat` 档滚轮编码为一次应答：每档一个 press + release 对。
    ///
    /// 触摸拖动一个事件可能滚过数行，逐档往返会把 UI 线程按在 VT 线程上
    /// （每档两次应答）；合并后一次事件只有一次往返，PTY 也只写一次。
    /// `button` 3 为向上（更旧）、4 为向下（更新）。鼠标上报未启用时返回空。
    pub fn encode_mouse_wheel(
        &self,
        position: (f32, f32),
        button: u8,
        modifiers: u16,
        cell_width: f32,
        cell_height: f32,
        repeat: u32,
    ) -> Option<Vec<u8>> {
        Self::encode_mouse_wheel_on(
            &self.query_tx,
            position,
            button,
            modifiers,
            cell_width,
            cell_height,
            repeat,
        )
    }

    /// [`Self::encode_mouse_wheel`] 的通道外置版，语义同
    /// [`Self::encode_mouse_event_on`]。
    pub(crate) fn encode_mouse_wheel_on(
        query_tx: &Sender<Query>,
        position: (f32, f32),
        button: u8,
        modifiers: u16,
        cell_width: f32,
        cell_height: f32,
        repeat: u32,
    ) -> Option<Vec<u8>> {
        Self::query_on(
            query_tx,
            |tx| Query::EncodeMouseWheel {
                position,
                button,
                modifiers,
                cell_width,
                cell_height,
                repeat: repeat.max(1),
                tx,
            },
            Vec::new(),
            "encode_mouse_wheel",
        )
        .into()
    }

    /// 用上游 `key::Encoder` 把按键事件编码为终端转义序列。
    ///
    /// 编码器的全部选项（DECCKM、keypad 应用模式、modifyOtherKeys、kitty 键盘
    /// 协议、backarrow key mode）在每次按键前从终端状态重取，故调用方无需查询
    /// 任何模式：应用改了 DECSET，本函数下一次调用即按新模式编码。
    ///
    /// `text` 是平台按当前布局与修饰生成的文本（Shift+A 的 `A`），无文本的键传
    /// `None`；`unshifted` 是该键无修饰时的码点，kitty 键盘协议据此区分档位。
    /// 单独的修饰键等不产生序列的键返回 `Some(空)`，由调用方丢弃。
    /// 仅查询通道卡死时返回 `None`。
    pub fn encode_key_event(
        &self,
        key: key::Key,
        action: key::Action,
        modifiers: u16,
        text: Option<char>,
        unshifted: Option<char>,
    ) -> Option<Vec<u8>> {
        Self::encode_key_event_on(&self.query_tx, key, action, modifiers, text, unshifted)
    }

    /// [`Self::encode_key_event`] 的通道外置版：调用方在会话锁内克隆查询通道，
    /// 随后在锁外编码，使每次击键不持有会话锁与注册表锁等待 VT 应答——否则重负载
    /// VT 线程下每次按键会连同按帧取这两把锁的渲染一起卡住一个查询超时。
    pub(crate) fn encode_key_event_on(
        query_tx: &Sender<Query>,
        key: key::Key,
        action: key::Action,
        modifiers: u16,
        text: Option<char>,
        unshifted: Option<char>,
    ) -> Option<Vec<u8>> {
        Self::query_on(
            query_tx,
            |tx| Query::EncodeKeyEvent {
                key,
                action,
                modifiers,
                text,
                unshifted,
                tx,
            },
            Vec::new(),
            "encode_key_event",
        )
        .into()
    }

    /// 用上游 `paste::encode` 把剪贴板文本编码为 PTY 写入字节。
    ///
    /// bracketed 标志取自终端模式 2004 的活体状态：启用时包裹并保留换行，
    /// 未启用时换行转回车；控制字节由上游中化。不取 `Terminal::paste`
    /// （其拒绝流程要求确认对话框，PROHIBITED 明禁）。
    ///
    /// 编码失败返回 `Some(空)`，由调用方丢弃本次粘贴；
    /// 仅查询通道卡死时返回 `None`。
    pub fn encode_paste(&self, data: &[u8]) -> Option<Vec<u8>> {
        Self::encode_paste_on(&self.query_tx, data)
    }

    /// [`Self::encode_paste`] 的通道外置版，理由同 [`Self::encode_key_event_on`]。
    pub(crate) fn encode_paste_on(query_tx: &Sender<Query>, data: &[u8]) -> Option<Vec<u8>> {
        Self::query_on(
            query_tx,
            |tx| Query::EncodePaste {
                data: data.to_vec(),
                tx,
            },
            Vec::new(),
            "encode_paste",
        )
        .into()
    }
}
