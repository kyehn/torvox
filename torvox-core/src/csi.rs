//! CSI (Control Sequence Introducer) handlers.
use crate::{sgr::parse_sgr, terminal::TerminalState};

/// CSI (Control Sequence Introducer) sequence handler
pub struct CsiHandler;

impl CsiHandler {
    /// Process a CSI sequence
    pub fn process_csi(
        sequence: &crate::vt_types::CsiSequence,
        terminal: &mut TerminalState,
        rows: u16,
        cols: u16,
    ) {
        let final_byte = sequence.final_byte;
        let params = &sequence.params;

        match final_byte {
            // Cursor movement
            b'A' => terminal.cursor_up(sequence.first_param_or(1), rows),
            b'B' => terminal.cursor_down(sequence.first_param_or(1), rows),
            b'C' => terminal.cursor_forward(sequence.first_param_or(1), cols),
            b'D' => terminal.cursor_back(sequence.first_param_or(1)),
            b'E' => terminal.cursor_next_line(sequence.first_param_or(1), rows),
            b'F' => terminal.cursor_prev_line(sequence.first_param_or(1), rows),
            b'G' => terminal.cursor_horizontal_absolute(sequence.first_param_or(1), cols),
            b'H' => terminal.cursor_position(
                sequence.first_param_or(1),
                sequence.second_param_or(1),
                rows,
                cols,
            ),
            b'I' => terminal.cursor_horizontal_tab(cols),
            b'J' => terminal.erase_in_display(sequence.first_param_or(0) as u8, rows, cols),
            b'K' => terminal.erase_in_line(sequence.first_param_or(0) as u8),
            b'L' => terminal.insert_lines(sequence.first_param_or(1), rows),
            b'M' => terminal.delete_lines(sequence.first_param_or(1), rows),
            b'P' => terminal.delete_characters(sequence.first_param_or(1)),
            b'S' => terminal.scroll_up(sequence.first_param_or(1), rows),
            b'T' => terminal.scroll_down(sequence.first_param_or(1), rows),
            b'X' => terminal.erase_characters(sequence.first_param_or(1)),
            b'Z' => terminal.cursor_horizontal_tab_back(cols),
            b'`' => terminal.cursor_horizontal_absolute(sequence.first_param_or(1), cols),
            b'b' => terminal.repeat_character(sequence.first_param_or(1)),
            b'c' => {
                // Device attributes - DA
                // In a real implementation, this would send a response
            }
            b'd' => terminal.cursor_vertical_absolute(sequence.first_param_or(1), rows),
            b'f' => terminal.cursor_position(
                sequence.first_param_or(1),
                sequence.second_param_or(1),
                rows,
                cols,
            ),
            b'h' => {
                if let Some(param) = params.first() {
                    CsiHandler::process_dec_mode(*param, true, terminal);
                }
            }
            b'l' => {
                if let Some(param) = params.first() {
                    CsiHandler::process_dec_mode(*param, false, terminal);
                }
            }
            b'm' => {
                // Select Graphic Rendition (SGR)
                let attrs = parse_sgr(params);
                terminal.apply_sgr(&attrs);
            }
            b'n' => {
                // Device status report - DSR
                // In a real implementation, this would send a response
            }
            b'r' if params.len() >= 2 => {
                let top = params[0].max(1);
                let bottom = params[1].max(top);
                terminal.set_scrolling_region(top - 1, bottom - 1);
            }
            b's' => {
                // Save cursor position (ANSI)
                terminal.save_cursor();
            }
            b'u' => {
                // Restore cursor position (ANSI)
                terminal.restore_cursor();
            }
            _ => {}
        }
    }

    /// Process DEC private mode
    pub fn process_dec_mode(mode: u16, enabled: bool, terminal: &mut TerminalState) {
        match mode {
            1u16 => terminal.set_dec_mode(mode, enabled),
            2u16 => terminal.set_dec_mode(mode, enabled),
            3u16 => terminal.set_dec_mode(mode, enabled),
            4u16 => terminal.set_dec_mode(mode, enabled),
            5u16 => terminal.set_dec_mode(mode, enabled),
            6u16 => terminal.set_origin_mode(enabled),
            7u16 => terminal.set_auto_wrap(enabled),
            8u16 => terminal.set_dec_mode(mode, enabled),
            9u16 => terminal.set_dec_mode(mode, enabled),
            20u16 => terminal.set_dec_mode(mode, enabled),
            25u16 => terminal.set_cursor_visible(enabled),
            40u16 => terminal.set_dec_mode(mode, enabled),
            47u16 => terminal.set_alternate_screen(enabled),
            2004u16 => terminal.set_bracketed_paste(enabled),
            1007u16 => terminal.set_dec_mode(mode, enabled),
            1008u16 => terminal.set_dec_mode(mode, enabled),
            1009u16 => terminal.set_dec_mode(mode, enabled),
            10010u16 => terminal.set_dec_mode(mode, enabled),
            10011u16 => terminal.set_dec_mode(mode, enabled),
            _ => {}
        }
    }
}

/// OSC (Operating System Command) sequence handler
pub struct OscHandler;

impl OscHandler {
    /// Process an OSC sequence
    pub fn process_osc(sequence: &crate::vt_types::OscSequence, terminal: &mut TerminalState) {
        if sequence.params.is_empty() {
            return;
        }

        match sequence.params[0].as_str() {
            "0" | "1" | "2" if sequence.params.len() >= 2 => {
                terminal.set_title(&sequence.params[1]);
                if sequence.params[0] == "0" {
                    terminal.set_icon_title(&sequence.params[1]);
                }
            }
            "4" => {}
            "10" | "11" | "12" => {}
            "52" => {}
            "104" | "110" | "111" | "112" => {}
            "8" => {}
            _ => {}
        }
    }
}

/// ESC (Escape) sequence handler
pub struct EscHandler;

impl EscHandler {
    /// Process an ESC sequence
    pub fn process_esc(sequence: &crate::vt_types::EscSequence, terminal: &mut TerminalState) {
        match sequence.final_byte {
            b'7' => terminal.save_cursor(),
            b'8' => terminal.restore_cursor(),
            b'D' => {}
            b'E' => {}
            b'M' => {}
            b'c' => {}
            b'(' => {}
            b')' => {}
            b'*' => {}
            b'+' => {}
            b'~' => {}
            _ => {}
        }
    }
}

/// Complete VT sequence processing
pub fn process_vt_sequence(
    sequence: &crate::vt_types::VtSequence,
    terminal: &mut TerminalState,
    rows: u16,
    cols: u16,
) {
    match sequence {
        crate::vt_types::VtSequence::Csi(csi) => {
            CsiHandler::process_csi(csi, terminal, rows, cols);
        }
        crate::vt_types::VtSequence::Osc(osc) => {
            OscHandler::process_osc(osc, terminal);
        }
        crate::vt_types::VtSequence::Esc(esc) => {
            EscHandler::process_esc(esc, terminal);
        }
        crate::vt_types::VtSequence::Sgr(attrs) => {
            terminal.apply_sgr(attrs);
        }
        crate::vt_types::VtSequence::Control(byte) => match byte {
            0x05 => {}     // ENQ — handled by caller
            0x07 => {}     // BEL — handled by caller
            0x08 => {}     // BS — handled by caller
            0x09 => {}     // TAB — handled by caller
            0x0A => {}     // LF — handled by caller
            0x0B => {}     // VT — handled by caller
            0x0C => {}     // FF — handled by caller
            0x0D => {}     // CR — handled by caller
            0x0E => {}     // SO — handled by caller
            0x0F => {}     // SI — handled by caller
            0x11 => {}     // XON — handled by caller
            0x13 => {}     // XOFF — handled by caller
            0x1B => {}     // ESC — handled by caller
            _unknown => {} // Handled by caller
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vt_types::{CsiSequence, EscSequence, OscSequence, VtSequence};
    use alloc::string::ToString;
    use alloc::vec;
    use alloc::vec::Vec;

    fn csi(final_byte: u8, params: Vec<u16>) -> CsiSequence {
        CsiSequence {
            final_byte,
            params,
            intermediate: None,
        }
    }

    fn osc(params: Vec<&str>) -> OscSequence {
        OscSequence {
            params: params.into_iter().map(ToString::to_string).collect(),
        }
    }

    fn esc(final_byte: u8) -> EscSequence {
        EscSequence {
            final_byte,
            intermediate: None,
        }
    }

    #[test]
    fn cursor_up_moves_cursor() {
        let mut terminal = TerminalState::new(24, 80);
        terminal.cursor_row = 10;
        let sequence = csi(b'A', vec![5]);
        CsiHandler::process_csi(&sequence, &mut terminal, 24, 80);
        assert_eq!(terminal.cursor_row, 5);
    }

    #[test]
    fn cursor_down_moves_cursor() {
        let mut terminal = TerminalState::new(24, 80);
        terminal.cursor_row = 10;
        let sequence = csi(b'B', vec![5]);
        CsiHandler::process_csi(&sequence, &mut terminal, 24, 80);
        assert_eq!(terminal.cursor_row, 15);
    }

    #[test]
    fn cursor_forward_moves_cursor() {
        let mut terminal = TerminalState::new(24, 80);
        terminal.cursor_col = 10;
        let sequence = csi(b'C', vec![5]);
        CsiHandler::process_csi(&sequence, &mut terminal, 24, 80);
        assert_eq!(terminal.cursor_col, 15);
    }

    #[test]
    fn cursor_back_moves_cursor() {
        let mut terminal = TerminalState::new(24, 80);
        terminal.cursor_col = 10;
        let sequence = csi(b'D', vec![5]);
        CsiHandler::process_csi(&sequence, &mut terminal, 24, 80);
        assert_eq!(terminal.cursor_col, 5);
    }

    #[test]
    fn cursor_back_clamps_to_zero() {
        let mut terminal = TerminalState::new(24, 80);
        terminal.cursor_col = 3;
        let sequence = csi(b'D', vec![10]);
        CsiHandler::process_csi(&sequence, &mut terminal, 24, 80);
        assert_eq!(terminal.cursor_col, 0);
    }

    #[test]
    fn cursor_position_absolute() {
        let mut terminal = TerminalState::new(24, 80);
        let sequence = csi(b'H', vec![5, 10]);
        CsiHandler::process_csi(&sequence, &mut terminal, 24, 80);
        assert_eq!(terminal.cursor_row, 4);
        assert_eq!(terminal.cursor_col, 9);
    }

    #[test]
    fn cursor_position_defaults_to_one_one() {
        let mut terminal = TerminalState::new(24, 80);
        let sequence = csi(b'H', vec![]);
        CsiHandler::process_csi(&sequence, &mut terminal, 24, 80);
        assert_eq!(terminal.cursor_row, 0);
        assert_eq!(terminal.cursor_col, 0);
    }

    #[test]
    fn erase_line_does_not_move_cursor_row() {
        let mut terminal = TerminalState::new(24, 80);
        terminal.cursor_row = 5;
        terminal.cursor_col = 10;
        let sequence = csi(b'K', vec![0]);
        CsiHandler::process_csi(&sequence, &mut terminal, 24, 80);
        assert_eq!(terminal.cursor_row, 5);
        assert_eq!(terminal.cursor_col, 10);
    }

    #[test]
    fn sgr_bold_and_reset() {
        let mut terminal = TerminalState::new(24, 80);
        let sequence = csi(b'm', vec![1]);
        CsiHandler::process_csi(&sequence, &mut terminal, 24, 80);
        assert!(
            terminal
                .sgr_attributes
                .contains(&crate::sgr::SgrAttribute::Bold(true))
        );

        let reset_sequence = csi(b'm', vec![0]);
        CsiHandler::process_csi(&reset_sequence, &mut terminal, 24, 80);
        assert!(
            !terminal
                .sgr_attributes
                .contains(&crate::sgr::SgrAttribute::Bold(true))
        );
    }

    #[test]
    fn dec_mode_cursor_visibility() {
        let mut terminal = TerminalState::new(24, 80);
        let sequence = csi(b'h', vec![25]);
        CsiHandler::process_csi(&sequence, &mut terminal, 24, 80);
        assert!(terminal.cursor_visible);

        let reset_sequence = csi(b'l', vec![25]);
        CsiHandler::process_csi(&reset_sequence, &mut terminal, 24, 80);
        assert!(!terminal.cursor_visible);
    }

    #[test]
    fn dec_mode_bracketed_paste() {
        let mut terminal = TerminalState::new(24, 80);
        let sequence = csi(b'h', vec![2004]);
        CsiHandler::process_csi(&sequence, &mut terminal, 24, 80);
        assert!(terminal.bracketed_paste);

        let reset_sequence = csi(b'l', vec![2004]);
        CsiHandler::process_csi(&reset_sequence, &mut terminal, 24, 80);
        assert!(!terminal.bracketed_paste);
    }

    #[test]
    fn save_restore_cursor_roundtrip() {
        let mut terminal = TerminalState::new(24, 80);
        terminal.cursor_row = 10;
        terminal.cursor_col = 20;

        let save_sequence = csi(b's', vec![]);
        CsiHandler::process_csi(&save_sequence, &mut terminal, 24, 80);
        terminal.cursor_row = 0;
        terminal.cursor_col = 0;

        let restore_sequence = csi(b'u', vec![]);
        CsiHandler::process_csi(&restore_sequence, &mut terminal, 24, 80);
        assert_eq!(terminal.cursor_row, 10);
        assert_eq!(terminal.cursor_col, 20);
    }

    #[test]
    fn scrolling_region_set() {
        let mut terminal = TerminalState::new(24, 80);
        let sequence = csi(b'r', vec![5, 20]);
        CsiHandler::process_csi(&sequence, &mut terminal, 24, 80);
        assert_eq!(terminal.scrolling_region, Some((4, 19)));
    }

    #[test]
    fn vt_sequence_routes_csi_cursor_up() {
        let mut terminal = TerminalState::new(24, 80);
        terminal.cursor_row = 10;
        let sequence = VtSequence::Csi(csi(b'A', vec![3]));
        process_vt_sequence(&sequence, &mut terminal, 24, 80);
        assert_eq!(terminal.cursor_row, 7);
    }

    #[test]
    fn vt_sequence_routes_osc_title() {
        let mut terminal = TerminalState::new(24, 80);
        let sequence = VtSequence::Osc(osc(vec!["0", "My Terminal"]));
        process_vt_sequence(&sequence, &mut terminal, 24, 80);
        assert_eq!(terminal.title.as_deref(), Some("My Terminal"));
    }

    #[test]
    fn vt_sequence_routes_esc_save_restore() {
        let mut terminal = TerminalState::new(24, 80);
        terminal.cursor_row = 15;
        terminal.cursor_col = 25;

        let save_sequence = VtSequence::Esc(esc(b'7'));
        process_vt_sequence(&save_sequence, &mut terminal, 24, 80);

        terminal.cursor_row = 0;
        terminal.cursor_col = 0;

        let restore_sequence = VtSequence::Esc(esc(b'8'));
        process_vt_sequence(&restore_sequence, &mut terminal, 24, 80);
        assert_eq!(terminal.cursor_row, 15);
        assert_eq!(terminal.cursor_col, 25);
    }

    #[test]
    fn osc_title_zero_sets_both_title_and_icon() {
        let mut terminal = TerminalState::new(24, 80);
        OscHandler::process_osc(&osc(vec!["0", "Both"]), &mut terminal);
        assert_eq!(terminal.title.as_deref(), Some("Both"));
        assert_eq!(terminal.icon_title.as_deref(), Some("Both"));
    }

    #[test]
    fn osc_title_one_sets_title_only() {
        let mut terminal = TerminalState::new(24, 80);
        OscHandler::process_osc(&osc(vec!["1", "TitleOnly"]), &mut terminal);
        assert_eq!(terminal.title.as_deref(), Some("TitleOnly"));
        assert!(terminal.icon_title.is_none());
    }

    #[test]
    fn osc_title_two_sets_title_only() {
        let mut terminal = TerminalState::new(24, 80);
        OscHandler::process_osc(&osc(vec!["2", "TitleTwo"]), &mut terminal);
        assert_eq!(terminal.title.as_deref(), Some("TitleTwo"));
        assert!(terminal.icon_title.is_none());
    }

    #[test]
    fn osc_empty_params_noop() {
        let mut terminal = TerminalState::new(24, 80);
        OscHandler::process_osc(&OscSequence { params: vec![] }, &mut terminal);
        assert!(terminal.title.is_none());
    }

    #[test]
    fn dec_mode_origin_mode_toggle() {
        let mut terminal = TerminalState::new(24, 80);
        assert!(!terminal.origin_mode);

        let set_sequence = csi(b'h', vec![6]);
        CsiHandler::process_csi(&set_sequence, &mut terminal, 24, 80);
        assert!(terminal.origin_mode);

        let reset_sequence = csi(b'l', vec![6]);
        CsiHandler::process_csi(&reset_sequence, &mut terminal, 24, 80);
        assert!(!terminal.origin_mode);
    }

    #[test]
    fn dec_mode_auto_wrap_disable() {
        let mut terminal = TerminalState::new(24, 80);
        assert!(terminal.auto_wrap);

        let disable_sequence = csi(b'l', vec![7]);
        CsiHandler::process_csi(&disable_sequence, &mut terminal, 24, 80);
        assert!(!terminal.auto_wrap);
    }

    #[test]
    fn dec_mode_alternate_screen() {
        let mut terminal = TerminalState::new(24, 80);
        assert!(!terminal.alternate_screen);

        let enable_sequence = csi(b'h', vec![47]);
        CsiHandler::process_csi(&enable_sequence, &mut terminal, 24, 80);
        assert!(terminal.alternate_screen);

        let disable_sequence = csi(b'l', vec![47]);
        CsiHandler::process_csi(&disable_sequence, &mut terminal, 24, 80);
        assert!(!terminal.alternate_screen);
    }
}
