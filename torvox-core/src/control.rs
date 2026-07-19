//! C0 and C1 control code definitions.
//!
//! # Requirements
//! - [FR-008](crate) — Tab stops: set/reset
use serde::{Deserialize, Serialize};

/// C0 control codes (0x00-0x1F)
/// C0 control codes (0x00-0x1F) — basic ASCII control characters.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(
    feature = "rkyv",
    derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize)
)]
pub enum C0 {
    Nul,  // 0x00
    Enq,  // 0x05
    Bel,  // 0x07
    Bksp, // 0x08
    Tab,  // 0x09
    Lf,   // 0x0A
    Vt,   // 0x0B
    Ff,   // 0x0C
    Cr,   // 0x0D
    So,   // 0x0E - Shift Out (G1)
    Si,   // 0x0F - Shift In (G0)
    Xon,  // 0x11
    Xoff, // 0x13
    Esc,  // 0x1B
    Other(u8),
}

impl C0 {
    /// Parse a single byte into the corresponding C0 control code variant.
    pub fn from_byte(byte: u8) -> Self {
        match byte {
            0x00 => Self::Nul,
            0x05 => Self::Enq,
            0x07 => Self::Bel,
            0x08 => Self::Bksp,
            0x09 => Self::Tab,
            0x0A => Self::Lf,
            0x0B => Self::Vt,
            0x0C => Self::Ff,
            0x0D => Self::Cr,
            0x0E => Self::So,
            0x0F => Self::Si,
            0x11 => Self::Xon,
            0x13 => Self::Xoff,
            0x1B => Self::Esc,
            _ => Self::Other(byte),
        }
    }
}

/// C1 control codes (0x80-0x9F) - represented as ESC + byte
/// C1 control codes (0x80-0x9F) — 8-bit control characters sent as ESC + 2-byte sequence.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(
    feature = "rkyv",
    derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize)
)]
pub enum C1 {
    Hts, // ESC H - Horizontal Tab Set
    Ri,  // ESC M - Reverse Index
    Dcs, // ESC P - Device Control String
    Osc, // ESC ] - Operating System Command
    Sos, // ESC X - Start of String
    St,  // ESC \ - String Terminator
    Csi, // ESC [ - Control Sequence Introducer
    Nel, // ESC E - Next Line
    Ind, // ESC D - Index (line feed)
}

impl C1 {
    pub fn from_byte(byte: u8) -> Option<Self> {
        match byte {
            b'H' => Some(Self::Hts),
            b'M' => Some(Self::Ri),
            b'P' => Some(Self::Dcs),
            b']' => Some(Self::Osc),
            b'X' => Some(Self::Sos),
            b'\\' => Some(Self::St),
            b'[' => Some(Self::Csi),
            b'E' => Some(Self::Nel),
            b'D' => Some(Self::Ind),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn c0_from_byte_all_defined() {
        assert_eq!(C0::from_byte(0x00), C0::Nul);
        assert_eq!(C0::from_byte(0x05), C0::Enq);
        assert_eq!(C0::from_byte(0x07), C0::Bel);
        assert_eq!(C0::from_byte(0x08), C0::Bksp);
        assert_eq!(C0::from_byte(0x09), C0::Tab);
        assert_eq!(C0::from_byte(0x0A), C0::Lf);
        assert_eq!(C0::from_byte(0x0B), C0::Vt);
        assert_eq!(C0::from_byte(0x0C), C0::Ff);
        assert_eq!(C0::from_byte(0x0D), C0::Cr);
        assert_eq!(C0::from_byte(0x0E), C0::So);
        assert_eq!(C0::from_byte(0x0F), C0::Si);
        assert_eq!(C0::from_byte(0x11), C0::Xon);
        assert_eq!(C0::from_byte(0x13), C0::Xoff);
        assert_eq!(C0::from_byte(0x1B), C0::Esc);
    }

    #[test]
    fn c0_from_byte_other() {
        assert_eq!(C0::from_byte(0x01), C0::Other(0x01));
        assert_eq!(C0::from_byte(0x1F), C0::Other(0x1F));
    }

    #[test]
    fn c1_from_byte_all_defined() {
        assert_eq!(C1::from_byte(b'H'), Some(C1::Hts));
        assert_eq!(C1::from_byte(b'M'), Some(C1::Ri));
        assert_eq!(C1::from_byte(b'P'), Some(C1::Dcs));
        assert_eq!(C1::from_byte(b']'), Some(C1::Osc));
        assert_eq!(C1::from_byte(b'X'), Some(C1::Sos));
        assert_eq!(C1::from_byte(b'\\'), Some(C1::St));
        assert_eq!(C1::from_byte(b'['), Some(C1::Csi));
        assert_eq!(C1::from_byte(b'E'), Some(C1::Nel));
        assert_eq!(C1::from_byte(b'D'), Some(C1::Ind));
    }

    #[test]
    fn c1_from_byte_unknown() {
        assert_eq!(C1::from_byte(b'A'), None);
        assert_eq!(C1::from_byte(b'z'), None);
        assert_eq!(C1::from_byte(b'0'), None);
    }

    #[test]
    fn c0_bel_is_distinguishable_from_tab() {
        let bel = C0::from_byte(0x07);
        let tab = C0::from_byte(0x09);
        assert_ne!(bel, tab, "BEL and TAB should be distinct C0 controls");
    }

    #[test]
    fn c1_hts_is_distinguishable_from_ri() {
        let hts = C1::from_byte(b'H');
        let ri = C1::from_byte(b'M');
        assert_ne!(hts, ri, "HTS and RI should be distinct C1 controls");
    }

    #[test]
    fn c0_copy_preserves_variant() {
        let original = C0::Bel;
        let copied = original;
        assert_eq!(original, copied);
        if let C0::Other(_) = copied {
            panic!("Copy of Bel should not produce Other variant");
        }
    }

    #[test]
    fn c1_copy_preserves_variant() {
        let original = C1::Hts;
        let copied = original;
        assert_eq!(original, copied);
    }

    #[test]
    fn c0_other_preserves_value() {
        if let C0::Other(v) = C0::from_byte(0x42) {
            assert_eq!(v, 0x42);
        } else {
            panic!("Expected Other variant");
        }
    }

    #[test]
    fn c1_from_byte_boundary() {
        assert_eq!(C1::from_byte(b'@'), None);
        assert_eq!(C1::from_byte(b'_'), None);
    }
}
