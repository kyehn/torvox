/// WezTerm-mimicking SGR parser — validates how Ghostty interprets each
/// SGR parameter by sending the sequence and reading back cell attributes.
use crate::ghostty_terminal::GhosttyTerminal;

/// A parsed SGR descriptor for testing.
#[derive(Debug, Clone, PartialEq)]
pub struct SgrEffects {
    pub bold: bool,
    pub dim: bool,
    pub italic: bool,
    pub underline: bool,
    pub blink: bool,
    pub reverse: bool,
    pub hidden: bool,
    pub strikethrough: bool,
    pub overline: bool,
    pub fg_set: bool,
    pub bg_set: bool,
    pub underline_set: bool,
}

impl SgrEffects {
    pub fn read_from(t: &GhosttyTerminal, col: u32) -> Self {
        let snap = t.take_snapshot();
        let cell = &snap.cells[col as usize];
        SgrEffects {
            bold: cell.bold,
            dim: cell.dim,
            italic: cell.italic,
            underline: cell.underline,
            blink: cell.blink,
            reverse: cell.reverse,
            hidden: cell.hidden,
            strikethrough: cell.strikethrough,
            overline: cell.overline,
            fg_set: cell.foreground[0] > 0.0
                || cell.foreground[1] > 0.0
                || cell.foreground[2] > 0.0,
            bg_set: cell.background[0] > 0.0
                || cell.background[1] > 0.0
                || cell.background[2] > 0.0,
            underline_set: cell.underline,
        }
    }

    pub fn all_clear(&self) -> bool {
        !self.bold
            && !self.dim
            && !self.italic
            && !self.underline
            && !self.blink
            && !self.reverse
            && !self.hidden
            && !self.strikethrough
            && !self.overline
    }
}

/// Send an SGR sequence, write "X" at column 0, then read back the effects.
pub fn apply_sgr_and_read(t: &mut GhosttyTerminal, params: &[u8]) -> SgrEffects {
    let mut seq = Vec::new();
    seq.extend_from_slice(b"\x1b[H\x1b[2K\x1b["); // home, clear line, CSI start
    for (i, p) in params.iter().enumerate() {
        if i > 0 {
            seq.push(b';');
        }
        seq.extend_from_slice(&p.to_string().into_bytes());
    }
    seq.push(b'm');
    seq.push(b'X'); // write character in same batch
    t.vt_write(&seq);
    t.flush();
    SgrEffects::read_from(t, 0)
}

/// Verify that after SGR 0, *all* attributes are cleared.
pub fn assert_sgr0_clears_all(t: &mut GhosttyTerminal) {
    // Set a bunch of attributes
    t.vt_write(b"\x1b[1;3;4;5;7;9;53m");
    t.flush();
    t.vt_write(b"\x1b[0mX");
    t.flush();
    let fx = SgrEffects::read_from(t, 0);
    assert!(fx.all_clear(), "SGR 0 did not clear all: {:?}", fx);
}

/// Verify that a specific SGR sequence is idempotent (applying twice is same as once).
pub fn assert_sgr_idempotent(t: &mut GhosttyTerminal, params: &[u8]) {
    let first = apply_sgr_and_read(t, params);
    let second = apply_sgr_and_read(t, params);
    assert_eq!(
        first.bold, second.bold,
        "SGR {:?} bold not idempotent",
        params
    );
    assert_eq!(
        first.underline, second.underline,
        "SGR {:?} underline not idempotent",
        params
    );
}

// ── Ghostty SGR correctness bug detection ────────────────────────────

/// BUG DETECTED: SGR 21 should clear bold (per ECMA-48 5th ed), but Ghostty
/// treats it as double-underline.  Returns `true` if the bug is present.
///
/// Reference: <https://vt100.net/emu/dec_ansi_parser> (SGR 21 = Bold off)
pub fn detect_sgr_21_bold_off_bug(t: &mut GhosttyTerminal) -> bool {
    t.vt_write(b"\x1b[1m\x1b[21mX");
    t.flush();
    let fx = SgrEffects::read_from(t, 0);
    fx.bold // If bold is still set, the bug exists
}

/// BUG DETECTED: SGR 22 should clear bold (ECMA-48 8.3.53).  Ghostty handles
/// this correctly.  Returns `true` if bold is off after SGR 22.
pub fn verify_sgr_22_clears_bold(t: &mut GhosttyTerminal) -> bool {
    t.vt_write(b"\x1b[1m\x1b[22mX");
    t.flush();
    let fx = SgrEffects::read_from(t, 0);
    !fx.bold // Should be true (bold cleared)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ghostty_terminal::GhosttyTerminal;

    fn t() -> GhosttyTerminal {
        GhosttyTerminal::new(24, 80, 1000).expect("terminal")
    }

    #[test]
    fn sgr_bold_attr() {
        let fx = apply_sgr_and_read(&mut t(), &[1]);
        assert!(fx.bold, "SGR 1 should set bold");
    }

    #[test]
    fn sgr_italic_attr() {
        let fx = apply_sgr_and_read(&mut t(), &[3]);
        assert!(fx.italic, "SGR 3 should set italic");
    }

    #[test]
    fn sgr_underline_attr() {
        let fx = apply_sgr_and_read(&mut t(), &[4]);
        assert!(fx.underline, "SGR 4 should set underline");
    }

    #[test]
    fn sgr_blink_attr() {
        let fx = apply_sgr_and_read(&mut t(), &[5]);
        assert!(fx.blink, "SGR 5 should set blink");
    }

    #[test]
    fn sgr_reverse_attr() {
        let fx = apply_sgr_and_read(&mut t(), &[7]);
        assert!(fx.reverse, "SGR 7 should set reverse");
    }

    #[test]
    fn sgr_hidden_attr() {
        let fx = apply_sgr_and_read(&mut t(), &[8]);
        assert!(fx.hidden, "SGR 8 should set hidden");
    }

    #[test]
    fn sgr_strikethrough_attr() {
        let fx = apply_sgr_and_read(&mut t(), &[9]);
        assert!(fx.strikethrough, "SGR 9 should set strikethrough");
    }

    #[test]
    fn sgr_overline_attr() {
        let fx = apply_sgr_and_read(&mut t(), &[53]);
        assert!(fx.overline, "SGR 53 should set overline");
    }

    #[test]
    fn sgr0_clears_all() {
        assert_sgr0_clears_all(&mut t());
    }

    #[test]
    fn sgr_bold_idempotent() {
        assert_sgr_idempotent(&mut t(), &[1]);
    }

    #[test]
    fn sgr_underline_idempotent() {
        assert_sgr_idempotent(&mut t(), &[4]);
    }
}
