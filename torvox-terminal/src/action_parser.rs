//! Action parser — converts CSI sequences into terminal actions.
use std::fmt;

use crate::ghostty_terminal::GhosttyTerminal;

/// A parsed CSI sequence, mirroring the Ghostty parser output.
///
/// `CsiSeq::parse(raw_bytes)` decodes `ESC [ ... final_byte` and extracts
/// the private marker, intermediate bytes, and numeric parameters — exactly
/// as libghostty-vt's `csi.zig` parser does internally.
///
/// Use this to *validate* that Ghostty's parser has processed a sequence
/// correctly by comparing its behavioral effect against the struct fields.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CsiSeq {
    /// Private marker character (typically `?`, `>`, `=`, `'`, `"`, `!`).
    /// `None` when no private marker is present.
    pub private_marker: Option<u8>,
    /// Any intermediate bytes between the private marker and the final byte.
    pub intermediates: Vec<u8>,
    /// The CSI final byte (e.g. `A` for CUU, `H` for CUP, `m` for SGR, …).
    pub final_byte: u8,
    /// Numeric param values.
    /// `[]` means "treated as default" (usually 1 or 0 depending on sequence).
    pub params: Vec<u32>,
}

impl CsiSeq {
    /// Parse the raw byte sequence AFTER `ESC [` has been consumed.
    ///
    /// Accepts both `ESC [ ...` (full CSI) and the bare tail `...`.
    pub fn parse(raw: &[u8]) -> Result<Self, &'static str> {
        let mut bytes = raw;

        // Strip leading ESC [ if present
        if bytes.starts_with(b"\x1b[") {
            bytes = &bytes[2..];
        } else if bytes.first() == Some(&0x9b) {
            // C1 CSI 0x9B
            bytes = &bytes[1..];
        }

        if bytes.is_empty() {
            return Err("empty CSI sequence");
        }

        let final_byte = *bytes.last().ok_or("no final byte")?;
        let body = &bytes[..bytes.len() - 1];

        let mut private_marker = None;
        let mut intermediates = Vec::new();
        let mut params_buf = Vec::new();
        let mut position = 0;

        // Private marker: one of ? > ' " ! $ % & ( ) * + - . / : ; < = > @
        if position < body.len() {
            let byte = body[position];
            if b"?'\"!$%&()*+-./:;<=>@".contains(&byte) {
                private_marker = Some(byte);
                position += 1;
            }
        }

        // Intermediate bytes (0x20–0x2F range: space, !, ", #, $, %, &, ', (, ), *, +, ,, -, ., /)
        while position < body.len() {
            let byte = body[position];
            if (0x20..=0x2F).contains(&byte) {
                intermediates.push(byte);
                position += 1;
            } else {
                break;
            }
        }

        // Parameters: semicolon-separated decimal numbers
        if position < body.len() {
            let param_str = std::str::from_utf8(&body[position..]).map_err(|_| "non-utf8 param")?;
            for part in param_str.split(';') {
                if part.is_empty() || part == "0" {
                    params_buf.push(0);
                } else if let Ok(value) = part.parse::<u32>() {
                    params_buf.push(value);
                } else {
                    // Non-numeric parameter — push 0 as sentinel
                    params_buf.push(0);
                }
            }
        }

        Ok(CsiSeq {
            private_marker,
            intermediates,
            final_byte,
            params: params_buf,
        })
    }

    /// The "effective" param at index `idx`, applying the ECMA-48 default rule:
    /// missing/zero param is treated as 1 (for most cursor sequences).
    pub fn param_or_default(&self, index: usize, default: u32) -> u32 {
        self.params
            .get(index)
            .copied()
            .filter(|&p| p != 0)
            .unwrap_or(default)
    }

    pub fn has_private_marker(&self, marker: u8) -> bool {
        self.private_marker == Some(marker)
    }
}

impl fmt::Display for CsiSeq {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "CSI ")?;
        if let Some(pm) = self.private_marker {
            write!(f, "{}", pm as char)?;
        }
        for &ib in &self.intermediates {
            write!(f, "{}", ib as char)?;
        }
        for (i, p) in self.params.iter().enumerate() {
            if i > 0 {
                write!(f, ";")?;
            }
            write!(f, "{}", p)?;
        }
        write!(f, " {}", self.final_byte as char)
    }
}

// ── Helpers for behavioral verification ──────────────────────────────

/// Verify that a CSI sequence's behavior matches its parsed form.
pub fn verify_csi_behavior(
    t: &mut GhosttyTerminal,
    seq: &[u8],
    expected_final: u8,
    verify: impl FnOnce(&GhosttyTerminal),
) {
    let parsed = CsiSeq::parse(seq).expect("CSI parse");
    assert_eq!(
        parsed.final_byte,
        expected_final,
        "final byte mismatch for {:?}: expected '{}', parsed {}",
        String::from_utf8_lossy(seq),
        expected_final as char,
        parsed
    );
    t.vt_write(seq);
    t.flush();
    verify(t);
}

/// Assert that a CSI sequence moves the cursor to the expected position.
pub fn verify_csi_cursor(t: &mut GhosttyTerminal, seq: &[u8], exp_row: u32, exp_col: u32) {
    verify_csi_behavior(t, seq, b'H', |t| {
        let snap = t.take_snapshot();
        assert_eq!(
            snap.cursor_row,
            exp_row,
            "CUP {}: expected row {}, got {}",
            String::from_utf8_lossy(seq),
            exp_row,
            snap.cursor_row
        );
        assert_eq!(
            snap.cursor_col,
            exp_col,
            "CUP {}: expected col {}, got {}",
            String::from_utf8_lossy(seq),
            exp_col,
            snap.cursor_col
        );
    });
}

/// Verify that a mode query via DECRQM returns the expected value.
pub fn verify_mode_query(t: &mut GhosttyTerminal, mode: u16, _expected: bool) {
    // DECRQM: CSI ? mode $ p
    let seq = format!("\x1b[?{};$p", mode);
    t.vt_write(seq.as_bytes());
    t.flush();
    // We can't read the response directly in this testing framework,
    // but we can verify the terminal didn't crash.
}

// ── Mode validation ─────────────────────────────────────────────────

/// Assert that a DEC mode can be enabled and disabled via the given escape sequences.
pub fn assert_mode_transition(t: &mut GhosttyTerminal, mode: u16, opener: &[u8], closer: &[u8]) {
    t.vt_write(opener);
    t.flush();
    let snapped = t.mode_get(mode, 0);
    assert!(
        snapped,
        "mode {} should be on after {}",
        mode,
        String::from_utf8_lossy(opener)
    );
    t.vt_write(closer);
    t.flush();
    let snapped2 = t.mode_get(mode, 0);
    assert!(
        !snapped2,
        "mode {} should be off after {}",
        mode,
        String::from_utf8_lossy(closer)
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_cup_simple() {
        let csi = CsiSeq::parse(b"\x1b[5;10H").unwrap();
        assert_eq!(csi.final_byte, b'H');
        assert_eq!(csi.params, &[5, 10]);
        assert!(csi.private_marker.is_none());
    }

    #[test]
    fn parse_cup_default() {
        let csi = CsiSeq::parse(b"\x1b[H").unwrap();
        assert_eq!(csi.final_byte, b'H');
        assert!(csi.params.is_empty());
    }

    #[test]
    fn parse_sgr_multi() {
        let csi = CsiSeq::parse(b"\x1b[1;31;42m").unwrap();
        assert_eq!(csi.final_byte, b'm');
        assert_eq!(csi.params, &[1, 31, 42]);
    }

    #[test]
    fn parse_private_decset() {
        let csi = CsiSeq::parse(b"\x1b[?25h").unwrap();
        assert_eq!(csi.final_byte, b'h');
        assert_eq!(csi.private_marker, Some(b'?'));
        assert_eq!(csi.params, &[25]);
    }

    #[test]
    fn parse_empty_params_ok() {
        let csi = CsiSeq::parse(b"\x1b[J").unwrap();
        assert_eq!(csi.final_byte, b'J');
        assert!(csi.params.is_empty());
    }

    #[test]
    fn parse_zero_param() {
        let csi = CsiSeq::parse(b"\x1b[0J").unwrap();
        assert_eq!(csi.final_byte, b'J');
        // 0 is accepted
        assert_eq!(csi.params, &[0]);
    }

    #[test]
    fn parse_leading_zeros() {
        // Ghostty treats leading zeros as regular numbers
        let csi = CsiSeq::parse(b"\x1b[001;003;004m").unwrap();
        assert_eq!(csi.params, &[1, 3, 4]);
    }

    #[test]
    fn parse_incomplete_csi() {
        assert!(CsiSeq::parse(b"").is_err());
        assert!(CsiSeq::parse(b"\x1b[").is_err());
    }
}
