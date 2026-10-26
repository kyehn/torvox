use super::*;

// ── 鼠标事件编码 ─────────────────────────────────────────────

impl super::GhosttyTerminal {
    /// 鼠标事件里的按钮编号 → 上游按钮。1/2 为右键/中键，3/4 为滚轮上/下，
    /// 其余（含 0）按左键处理：与 Android 的 `BUTTON_PRIMARY` 约定一致。
    pub(crate) fn mouse_button(button: u8) -> mouse::Button {
        match button {
            1 => mouse::Button::Right,
            2 => mouse::Button::Middle,
            3 => mouse::Button::Four,
            4 => mouse::Button::Five,
            _ => mouse::Button::Left,
        }
    }

    /// 把一个鼠标事件编码进 `out`。
    ///
    /// 编码器与事件跨事件复用：选项按终端当前状态重新同步，按钮/位置/修饰键
    /// 逐字段覆写。漏写任一字段都会让本次事件沿用上一次的取值。
    pub(crate) fn encode_mouse_event_into(
        encoders: &mut Encoders<'_>,
        terminal: &Terminal,
        out: &mut Vec<u8>,
        position: (f32, f32),
        action: mouse::Action,
        button: Option<mouse::Button>,
        modifiers: u16,
        cell_width: f32,
        cell_height: f32,
    ) {
        let Some((encoder, event)) = encoders.mouse.as_mut() else {
            log::warn!("ghostty_terminal: mouse encoder/event unavailable — dropping mouse event");
            return;
        };
        encoder.set_options_from_terminal(terminal);
        // 视口外 motion 只在有键按下时上报（上游 mouse_encode 口径）：
        // 按下/移动恒有接触（触摸 onTouchEvent 与持键鼠标移动），
        // 释放后无接触。缺省 false 会把滑出视口的拖动静默丢弃。
        encoder.set_any_button_pressed(!matches!(action, mouse::Action::Release));
        let cols = grid_cols(terminal);
        let rows = grid_rows(terminal);
        encoder.set_size(mouse::EncoderSize {
            screen_width: cols.saturating_mul(cell_width.max(1.0) as u32),
            screen_height: rows.saturating_mul(cell_height.max(1.0) as u32),
            cell_width: cell_width.max(1.0) as u32,
            cell_height: cell_height.max(1.0) as u32,
            padding_top: 0,
            padding_bottom: 0,
            padding_right: 0,
            padding_left: 0,
        });
        event.set_position(mouse::Position {
            x: position.0,
            y: position.1,
        });
        event.set_action(action);
        event.set_button(button);
        // 修饰键进编码器：Shift/Ctrl 点击在远端（vim/tmux/htop）必须与
        // 普通左键可区分，否则无法粘贴选择、无法扩展选区。
        event.set_mods(Mods::from_bits_retain(modifiers));
        if let Err(error) = encoder.encode_to_vec(event, out) {
            log::warn!("ghostty_terminal: mouse encode failed: {error}");
        }
    }
}
