//! VT sequence types — CSI, OSC, ESC definitions.
//!
//! # Requirements
//! - [FR-021](crate) — Terminal: state machine
use alloc::string::String;
use alloc::vec::Vec;

use crate::sgr::SgrAttribute;

/// CSI sequence parsed from input
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CsiSequence {
    pub params: Vec<u16>,
    pub intermediate: Option<u8>,
    pub final_byte: u8,
}

impl CsiSequence {
    /// Create a new CSI sequence with the given final byte and no parameters.
    ///
    /// ```
    /// use torvox_core::vt_types::CsiSequence;
    ///
    /// let seq = CsiSequence::new('m' as u8);
    /// assert!(seq.params.is_empty());
    /// assert_eq!(seq.final_byte, 'm' as u8);
    /// assert!(seq.intermediate.is_none());
    /// ```
    pub fn new(final_byte: u8) -> Self {
        Self {
            params: Vec::new(),
            intermediate: None,
            final_byte,
        }
    }

    pub fn with_params(params: Vec<u16>, final_byte: u8) -> Self {
        Self {
            params,
            intermediate: None,
            final_byte,
        }
    }

    /// Return the first parameter, or the default if no parameter is present
    /// or the first parameter is zero (VT convention: 0 means default).
    ///
    /// ```
    /// use torvox_core::vt_types::CsiSequence;
    ///
    /// // No params — returns default (clamped to at least 1)
    /// let seq = CsiSequence::new('A' as u8);
    /// assert_eq!(seq.first_param_or(1), 1);
    ///
    /// // Explicit param — returns the param value
    /// let seq = CsiSequence::with_params(vec![5], 'A' as u8);
    /// assert_eq!(seq.first_param_or(1), 5);
    ///
    /// // Zero param — treated as default per VT spec
    /// let seq = CsiSequence::with_params(vec![0], 'A' as u8);
    /// assert_eq!(seq.first_param_or(1), 1);
    /// ```
    pub fn first_param_or(&self, default: u16) -> u16 {
        match self.params.first() {
            Some(&0) | None => default,
            Some(&param) => param,
        }
    }

    pub fn second_param_or(&self, default: u16) -> u16 {
        match self.params.get(1) {
            Some(&0) | None => default,
            Some(&param) => param,
        }
    }
}

/// Parsed OSC (Operating System Command) sequence.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OscSequence {
    pub params: Vec<String>,
}

impl OscSequence {
    pub fn new() -> Self {
        Self { params: Vec::new() }
    }
}

impl Default for OscSequence {
    fn default() -> Self {
        Self::new()
    }
}

/// Parsed ESC (Escape) sequence with optional intermediate byte.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EscSequence {
    pub intermediate: Option<u8>,
    pub final_byte: u8,
}

impl EscSequence {
    pub fn new(final_byte: u8) -> Self {
        Self {
            intermediate: None,
            final_byte,
        }
    }

    pub fn with_intermediate(intermediate: u8, final_byte: u8) -> Self {
        Self {
            intermediate: Some(intermediate),
            final_byte,
        }
    }
}

/// Any parsed VT sequence — CSI, OSC, ESC, SGR, or control code.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VtSequence {
    Csi(CsiSequence),
    Osc(OscSequence),
    Esc(EscSequence),
    Sgr(Vec<SgrAttribute>),
    Control(u8),
}

/// CSI sequence final byte identifiers
pub mod csi_final {
    pub const CUU: u8 = b'A'; // Cursor Up
    pub const CUD: u8 = b'B'; // Cursor Down
    pub const CUF: u8 = b'C'; // Cursor Forward
    pub const CUB: u8 = b'D'; // Cursor Back
    pub const CNL: u8 = b'E'; // Cursor Next Line
    pub const CPL: u8 = b'F'; // Cursor Previous Line
    pub const CHA: u8 = b'G'; // Cursor Horizontal Absolute
    pub const CUP: u8 = b'H'; // Cursor Position
    pub const CHT: u8 = b'I'; // Cursor Horizontal Tab
    pub const ED: u8 = b'J'; // Erase in Display
    pub const EL: u8 = b'K'; // Erase in Line
    pub const IL: u8 = b'L'; // Insert Lines
    pub const DL: u8 = b'M'; // Delete Lines
    pub const DCH: u8 = b'P'; // Delete Characters
    pub const SU: u8 = b'S'; // Scroll Up
    pub const SD: u8 = b'T'; // Scroll Down
    pub const ECH: u8 = b'X'; // Erase Characters
    pub const CBT: u8 = b'Z'; // Cursor Backward Tab
    pub const HPA: u8 = b'`'; // Horizontal Position Absolute
    pub const REP: u8 = b'b'; // Repeat
    pub const DA: u8 = b'c'; // Device Attributes
    pub const VPA: u8 = b'd'; // Vertical Position Absolute
    pub const HVP: u8 = b'f'; // Horizontal and Vertical Position
    pub const SM: u8 = b'h'; // Set Mode
    pub const RM: u8 = b'l'; // Reset Mode
    pub const SGR: u8 = b'm'; // Select Graphic Rendition
    pub const DSR: u8 = b'n'; // Device Status Report
    pub const DECSTBM: u8 = b'r'; // Set Scrolling Region
    pub const SCP: u8 = b's'; // Save Cursor Position
    pub const RCP: u8 = b'u'; // Restore Cursor Position
}

/// DEC private mode identifiers
pub mod dec_mode {
    pub const DECCKM: u16 = 1; // Cursor keys mode
    pub const DECANM: u16 = 2; // ANSI/VT52 mode
    pub const DECCOLM: u16 = 3; // 132 column mode
    pub const DECSCLM: u16 = 4; // Smooth scroll
    pub const DECSCNM: u16 = 5; // Reverse video
    pub const DECOM: u16 = 6; // Origin mode
    pub const DECAWM: u16 = 7; // Auto-wrap mode
    pub const DECARM: u16 = 8; // Auto-repeat keys
    pub const DECINLM: u16 = 9; // Interlace scrolling
    pub const LNM: u16 = 20; // Line feed / newline mode
    pub const DECTCEM: u16 = 25; // Text cursor enable
    pub const DECCOLM_ALLOW: u16 = 40; // Allow 132 columns
    pub const DECCOLM_SET: u16 = 40; // Allow 80→132 column switching
    pub const DEC_ALT_47: u16 = 47; // Use alternate screen buffer
    pub const DEC_ALT_1047: u16 = 1047; // Use alternate screen buffer (no saved cursor)
    pub const DECSCOSC: u16 = 1048; // Save/restore cursor
    pub const DEC_ALT_1049: u16 = 1049; // Use alternate screen + save cursor
    pub const BRACKETED_PASTE: u16 = 2004; // Bracketed paste mode
    pub const MOUSE_X10: u16 = 9; // X10 mouse
    pub const MOUSE_BTN: u16 = 1000; // Button-event mouse tracking
    pub const MOUSE_DRAG: u16 = 1002; // Drag-event mouse tracking
    pub const MOUSE_ANY: u16 = 1003; // Any-event mouse tracking
    pub const MOUSE_FOCUS: u16 = 1004; // Focus event mouse
    pub const MOUSE_SGR: u16 = 1006; // SGR mouse encoding
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec;

    #[test]
    fn csi_sequence_new() {
        let seq = CsiSequence::new(b'A');
        assert_eq!(seq.final_byte, b'A');
        assert!(seq.params.is_empty());
        assert!(seq.intermediate.is_none());
    }

    #[test]
    fn csi_sequence_with_params() {
        let seq = CsiSequence::with_params(vec![5, 10], b'H');
        assert_eq!(seq.params, vec![5, 10]);
        assert_eq!(seq.final_byte, b'H');
    }

    #[test]
    fn csi_first_param_or_default() {
        let seq = CsiSequence::new(b'A');
        assert_eq!(seq.first_param_or(1), 1);

        let seq = CsiSequence::with_params(vec![0], b'A');
        assert_eq!(seq.first_param_or(1), 1); // 0 -> 1

        let seq = CsiSequence::with_params(vec![5], b'A');
        assert_eq!(seq.first_param_or(1), 5);

        let seq = CsiSequence::with_params(vec![100], b'A');
        assert_eq!(seq.first_param_or(1), 100);
    }

    #[test]
    fn csi_second_param_or_default() {
        let seq = CsiSequence::new(b'H');
        assert_eq!(seq.second_param_or(1), 1);

        let seq = CsiSequence::with_params(vec![5], b'H');
        assert_eq!(seq.second_param_or(1), 1);

        let seq = CsiSequence::with_params(vec![5, 10], b'H');
        assert_eq!(seq.second_param_or(1), 10);
    }

    #[test]
    fn csi_first_param_zero_means_use_default() {
        let seq = CsiSequence::with_params(vec![0], b'A');
        assert_eq!(seq.first_param_or(1), 1, "param 0 should trigger default");
        assert_eq!(
            seq.first_param_or(5),
            5,
            "param 0 should use caller's default"
        );
    }

    #[test]
    fn csi_params_with_many_values() {
        let seq = CsiSequence::with_params(vec![1, 2, 3, 4, 5], b'm');
        assert_eq!(seq.params.len(), 5);
        assert_eq!(seq.first_param_or(0), 1);
        assert_eq!(seq.second_param_or(0), 2);
    }

    #[test]
    fn csi_first_param_zero_default_is_honored() {
        // ED/EL use `first_param_or(0)`: a zero param must yield the default 0,
        // not be coerced to 1 (VT erase mode 0 is a distinct, valid mode).
        let seq = CsiSequence::with_params(vec![0], b'J');
        assert_eq!(seq.first_param_or(0), 0);
        let seq = CsiSequence::with_params(vec![0], b'K');
        assert_eq!(seq.first_param_or(0), 0);
        // A present nonzero param is returned as-is regardless of default.
        let seq = CsiSequence::with_params(vec![2], b'J');
        assert_eq!(seq.first_param_or(0), 2);
    }

    #[test]
    fn osc_sequence_new() {
        let seq = OscSequence::new();
        assert!(seq.params.is_empty());
    }

    #[test]
    fn esc_sequence_new() {
        let seq = EscSequence::new(b'7');
        assert_eq!(seq.final_byte, b'7');
        assert!(seq.intermediate.is_none());
    }

    #[test]
    fn esc_sequence_with_intermediate() {
        let seq = EscSequence::with_intermediate(b'(', b'B');
        assert_eq!(seq.intermediate, Some(b'('));
        assert_eq!(seq.final_byte, b'B');
    }

    #[test]
    fn csi_final_constants_are_ascii() {
        assert_eq!(csi_final::CUU, b'A');
        assert_eq!(csi_final::CUD, b'B');
        assert_eq!(csi_final::CUF, b'C');
        assert_eq!(csi_final::CUB, b'D');
        assert_eq!(csi_final::SGR, b'm');
        assert_eq!(csi_final::CUP, b'H');
        assert_eq!(csi_final::ED, b'J');
        assert_eq!(csi_final::EL, b'K');
    }

    #[test]
    fn dec_mode_constants_are_unique_and_in_expected_range() {
        let modes = [
            dec_mode::DECCKM,
            dec_mode::DECOM,
            dec_mode::DECAWM,
            dec_mode::DECTCEM,
            dec_mode::BRACKETED_PASTE,
        ];
        assert!(modes.iter().all(|&m| m > 0), "all modes should be positive");
        assert_eq!(
            dec_mode::BRACKETED_PASTE,
            2004,
            "bracketed paste is DEC mode 2004"
        );
    }

    #[test]
    fn vt_sequence_clone_preserves_data() {
        let seq = VtSequence::Csi(CsiSequence::with_params(vec![1], b'm'));
        let seq2 = seq.clone();
        assert_eq!(seq, seq2);
        match seq2 {
            VtSequence::Csi(inner) => assert_eq!(inner.params, vec![1]),
            _ => panic!("clone should produce Csi variant"),
        }
    }

    #[test]
    fn csi_sequence_equality_requires_same_params_and_byte() {
        let a = CsiSequence::with_params(vec![1], b'm');
        let b = CsiSequence::with_params(vec![1], b'm');
        assert_eq!(a, b);

        let c = CsiSequence::with_params(vec![2], b'm');
        assert_ne!(a, c, "different params should be unequal");

        let d = CsiSequence::with_params(vec![1], b'H');
        assert_ne!(a, d, "different final byte should be unequal");
    }

    #[test]
    fn esc_sequence_equality_requires_same_byte() {
        let a = EscSequence::new(b'7');
        let b = EscSequence::new(b'7');
        assert_eq!(a, b);

        let c = EscSequence::new(b'8');
        assert_ne!(a, c);
    }

    #[test]
    fn dec_mode_constants_are_all_distinct() {
        let modes = [
            dec_mode::DECCKM,
            dec_mode::DECANM,
            dec_mode::DECCOLM,
            dec_mode::DECSCLM,
            dec_mode::DECSCNM,
            dec_mode::DECOM,
            dec_mode::DECAWM,
            dec_mode::DECARM,
            dec_mode::DECTCEM,
            dec_mode::LNM,
            dec_mode::BRACKETED_PASTE,
        ];
        let mut sorted = modes.to_vec();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(sorted.len(), modes.len());
    }
}
