//! Select Graphic Rendition (SGR) attribute parsing.
//!
//! # Requirements
//! - [FR-003](crate) — SGR: bold, italic, underline, blink, inverse, strikethrough
use alloc::vec::Vec;
use serde::{Deserialize, Serialize};

use crate::cell::Color;

/// Underline styles for SGR (Select Graphic Rendition) escape sequences.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[cfg_attr(
    feature = "rkyv",
    derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize)
)]
pub enum UnderlineStyle {
    #[default]
    None,
    Single,
    Double,
    Curly,
    Dotted,
    Dashed,
}

/// Blink styles for SGR (Select Graphic Rendition) escape sequences.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[cfg_attr(
    feature = "rkyv",
    derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize)
)]
pub enum BlinkStyle {
    #[default]
    None,
    Slow,
    Rapid,
}

/// Color specification for SGR sequences — named (0-15), indexed (0-255), or 24-bit RGB.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(
    feature = "rkyv",
    derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize)
)]
pub enum ColorSpec {
    /// Named color (0-15)
    Named(u8),
    /// Indexed color (0-255)
    Indexed(u8),
    /// 24-bit RGB color
    Rgb { r: u8, g: u8, b: u8 },
}

/// SGR (Select Graphic Rendition) attribute — one of the parameters in a CSI m sequence.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(
    feature = "rkyv",
    derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize)
)]
pub enum SgrAttribute {
    Reset,
    Bold(bool),
    Faint(bool),
    Italic(bool),
    Underline(UnderlineStyle),
    Blink(BlinkStyle),
    Reverse(bool),
    Conceal(bool),
    Strikethrough(bool),
    Overline(bool),
    NormalIntensity,
    ForegroundColor(ColorSpec),
    BackgroundColor(ColorSpec),
    DefaultForegroundColor,
    DefaultBackgroundColor,
}

impl SgrAttribute {
    /// Parse a single SGR parameter code.
    ///
    /// ```
    /// use terminal_core::sgr::SgrAttribute;
    ///
    /// assert_eq!(SgrAttribute::from_code(0), Some(SgrAttribute::Reset));
    /// assert_eq!(SgrAttribute::from_code(1), Some(SgrAttribute::Bold(true)));
    /// assert_eq!(SgrAttribute::from_code(3), Some(SgrAttribute::Italic(true)));
    /// assert_eq!(SgrAttribute::from_code(24), Some(SgrAttribute::Underline(terminal_core::sgr::UnderlineStyle::None)));
    /// assert_eq!(SgrAttribute::from_code(999), None);
    /// ```
    pub fn from_code(code: u16) -> Option<Self> {
        match code {
            0 => Some(Self::Reset),
            1 => Some(Self::Bold(true)),
            2 => Some(Self::Faint(true)),
            3 => Some(Self::Italic(true)),
            4 => Some(Self::Underline(UnderlineStyle::Single)),
            5 => Some(Self::Blink(BlinkStyle::Slow)),
            6 => Some(Self::Blink(BlinkStyle::Rapid)),
            7 => Some(Self::Reverse(true)),
            8 => Some(Self::Conceal(true)),
            9 => Some(Self::Strikethrough(true)),
            21 => Some(Self::Underline(UnderlineStyle::Double)),
            22 => Some(Self::NormalIntensity),
            23 => Some(Self::Italic(false)),
            24 => Some(Self::Underline(UnderlineStyle::None)),
            25 => Some(Self::Blink(BlinkStyle::None)),
            27 => Some(Self::Reverse(false)),
            28 => Some(Self::Conceal(false)),
            29 => Some(Self::Strikethrough(false)),
            39 => Some(Self::DefaultForegroundColor),
            49 => Some(Self::DefaultBackgroundColor),
            53 => Some(Self::Overline(true)),
            55 => Some(Self::Overline(false)),
            _ => None,
        }
    }

    /// Parse foreground color code (30-37, 90-97)
    pub fn foreground_color(code: u16) -> Option<Self> {
        match code {
            30..=37 => Some(Self::ForegroundColor(ColorSpec::Named((code - 30) as u8))),
            39 => Some(Self::DefaultForegroundColor),
            90..=97 => Some(Self::ForegroundColor(ColorSpec::Named(
                (code - 90 + 8) as u8,
            ))),
            _ => None,
        }
    }

    /// Parse background color code (40-47, 100-107)
    pub fn background_color(code: u16) -> Option<Self> {
        match code {
            40..=47 => Some(Self::BackgroundColor(ColorSpec::Named((code - 40) as u8))),
            49 => Some(Self::DefaultBackgroundColor),
            100..=107 => Some(Self::BackgroundColor(ColorSpec::Named(
                (code - 100 + 8) as u8,
            ))),
            _ => None,
        }
    }
}

/// Parse SGR (Select Graphic Rendition) parameters into a list of attributes.
pub fn parse_sgr(params: &[u16]) -> Vec<SgrAttribute> {
    let mut attrs = Vec::new();
    let mut i = 0;
    while i < params.len() {
        let code = params[i];
        match code {
            38 => {
                // FG extended color
                if let Some(color) = parse_extended_color(params, &mut i) {
                    attrs.push(SgrAttribute::ForegroundColor(color));
                }
            }
            48 => {
                // BG extended color
                if let Some(color) = parse_extended_color(params, &mut i) {
                    attrs.push(SgrAttribute::BackgroundColor(color));
                }
            }
            _ => {
                if let Some(attr) = SgrAttribute::from_code(code)
                    .or_else(|| SgrAttribute::foreground_color(code))
                    .or_else(|| SgrAttribute::background_color(code))
                {
                    attrs.push(attr);
                }
            }
        }
        i += 1;
    }
    attrs
}

/// Parse extended color (256 or 24-bit RGB)
fn parse_extended_color(params: &[u16], position: &mut usize) -> Option<ColorSpec> {
    *position += 1;
    let mode = params.get(*position)?;
    match mode {
        5 => {
            // 256-color: 38;5;idx
            *position += 1;
            let idx_val = params.get(*position)?;
            let idx = *idx_val as u8;
            Some(ColorSpec::Indexed(idx))
        }
        2 => {
            // 24-bit RGB: 38;2;r;g;b
            *position += 1;
            let red_val = params.get(*position)?;
            let red = *red_val as u8;
            *position += 1;
            let green_val = params.get(*position)?;
            let green = *green_val as u8;
            *position += 1;
            let blue_val = params.get(*position)?;
            let blue = *blue_val as u8;
            Some(ColorSpec::Rgb {
                r: red,
                g: green,
                b: blue,
            })
        }
        _ => None,
    }
}

/// Apply SGR attributes to a cell
pub fn apply_sgr(attrs: &[SgrAttribute], cell: &mut crate::cell::Cell) {
    for attr in attrs {
        match attr {
            SgrAttribute::Reset => {
                cell.foreground = Color::default();
                cell.background = Color::default();
                cell.attrs = crate::cell::Attrs::default();
            }
            SgrAttribute::Bold(v) => cell.attrs.bold = *v,
            SgrAttribute::Faint(v) => cell.attrs.dim = *v,
            SgrAttribute::Italic(v) => cell.attrs.italic = *v,
            SgrAttribute::Underline(style) => {
                cell.attrs.underline = *style != UnderlineStyle::None;
                cell.attrs.double_underline = *style == UnderlineStyle::Double;
            }
            SgrAttribute::Blink(style) => cell.attrs.blink = *style != BlinkStyle::None,
            SgrAttribute::Reverse(v) => cell.attrs.reverse = *v,
            SgrAttribute::Conceal(v) => cell.attrs.hidden = *v,
            SgrAttribute::Strikethrough(v) => cell.attrs.strikethrough = *v,
            SgrAttribute::Overline(v) => cell.attrs.overline = *v,
            SgrAttribute::NormalIntensity => {
                cell.attrs.bold = false;
                cell.attrs.dim = false;
            }
            SgrAttribute::ForegroundColor(spec) => cell.foreground = color_from_spec(spec),
            SgrAttribute::BackgroundColor(spec) => cell.background = color_from_spec(spec),
            SgrAttribute::DefaultForegroundColor => cell.foreground = Color::default(),
            SgrAttribute::DefaultBackgroundColor => cell.background = Color::default(),
        }
    }
}

/// Convert ColorSpec to Color.
///
/// ```
/// use terminal_core::sgr::{color_from_spec, ColorSpec};
/// use terminal_core::cell::Color;
///
/// let named = color_from_spec(&ColorSpec::Named(1)); // ANSI red
/// assert_eq!(named, Color::from_ansi(1));
///
/// let rgb = color_from_spec(&ColorSpec::Rgb { r: 255, g: 128, b: 0 });
/// assert_eq!(rgb, Color::new(255, 128, 0));
/// ```
pub fn color_from_spec(spec: &ColorSpec) -> Color {
    match spec {
        ColorSpec::Named(idx) => Color::from_ansi(*idx),
        ColorSpec::Indexed(idx) => Color::from_ansi(*idx),
        ColorSpec::Rgb { r, g, b } => Color::new(*r, *g, *b),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cell::Cell;

    #[test]
    fn sgr_reset() {
        let mut cell = Cell::default();
        cell.attrs.bold = true;
        apply_sgr(&[SgrAttribute::Reset], &mut cell);
        assert!(!cell.attrs.bold);
        assert_eq!(cell.foreground, Color::default());
    }

    #[test]
    fn sgr_bold_on() {
        let mut cell = Cell::default();
        apply_sgr(&[SgrAttribute::Bold(true)], &mut cell);
        assert!(cell.attrs.bold);
    }

    #[test]
    fn sgr_bold_off() {
        let mut cell = Cell::default();
        cell.attrs.bold = true;
        apply_sgr(&[SgrAttribute::Bold(false)], &mut cell);
        assert!(!cell.attrs.bold);
    }

    #[test]
    fn sgr_faint() {
        let mut cell = Cell::default();
        apply_sgr(&[SgrAttribute::Faint(true)], &mut cell);
        assert!(cell.attrs.dim);
    }

    #[test]
    fn sgr_italic() {
        let mut cell = Cell::default();
        apply_sgr(&[SgrAttribute::Italic(true)], &mut cell);
        assert!(cell.attrs.italic);
    }

    #[test]
    fn sgr_underline_single() {
        let mut cell = Cell::default();
        apply_sgr(
            &[SgrAttribute::Underline(UnderlineStyle::Single)],
            &mut cell,
        );
        assert!(cell.attrs.underline);
        assert!(!cell.attrs.double_underline);
    }

    #[test]
    fn sgr_underline_double() {
        let mut cell = Cell::default();
        apply_sgr(
            &[SgrAttribute::Underline(UnderlineStyle::Double)],
            &mut cell,
        );
        assert!(cell.attrs.underline);
        assert!(cell.attrs.double_underline);
    }

    #[test]
    fn sgr_underline_none() {
        let mut cell = Cell::default();
        cell.attrs.underline = true;
        apply_sgr(&[SgrAttribute::Underline(UnderlineStyle::None)], &mut cell);
        assert!(!cell.attrs.underline);
    }

    #[test]
    fn sgr_blink_slow() {
        let mut cell = Cell::default();
        apply_sgr(&[SgrAttribute::Blink(BlinkStyle::Slow)], &mut cell);
        assert!(cell.attrs.blink);
    }

    #[test]
    fn sgr_blink_none() {
        let mut cell = Cell::default();
        cell.attrs.blink = true;
        apply_sgr(&[SgrAttribute::Blink(BlinkStyle::None)], &mut cell);
        assert!(!cell.attrs.blink);
    }

    #[test]
    fn sgr_reverse() {
        let mut cell = Cell::default();
        apply_sgr(&[SgrAttribute::Reverse(true)], &mut cell);
        assert!(cell.attrs.reverse);
    }

    #[test]
    fn sgr_conceal() {
        let mut cell = Cell::default();
        apply_sgr(&[SgrAttribute::Conceal(true)], &mut cell);
        assert!(cell.attrs.hidden);
    }

    #[test]
    fn sgr_strikethrough() {
        let mut cell = Cell::default();
        apply_sgr(&[SgrAttribute::Strikethrough(true)], &mut cell);
        assert!(cell.attrs.strikethrough);
    }

    #[test]
    fn sgr_overline() {
        let mut cell = Cell::default();
        apply_sgr(&[SgrAttribute::Overline(true)], &mut cell);
        assert!(cell.attrs.overline);
    }

    #[test]
    fn sgr_fg_named() {
        let mut cell = Cell::default();
        apply_sgr(
            &[SgrAttribute::ForegroundColor(ColorSpec::Named(1))],
            &mut cell,
        );
        assert_eq!(cell.foreground, Color::from_ansi(1));
    }

    #[test]
    fn sgr_bg_named() {
        let mut cell = Cell::default();
        apply_sgr(
            &[SgrAttribute::BackgroundColor(ColorSpec::Named(4))],
            &mut cell,
        );
        assert_eq!(cell.background, Color::from_ansi(4));
    }

    #[test]
    fn sgr_fg_rgb() {
        let mut cell = Cell::default();
        apply_sgr(
            &[SgrAttribute::ForegroundColor(ColorSpec::Rgb {
                r: 10,
                g: 20,
                b: 30,
            })],
            &mut cell,
        );
        assert_eq!(cell.foreground, Color::new(10, 20, 30));
    }

    #[test]
    fn sgr_bg_rgb() {
        let mut cell = Cell::default();
        apply_sgr(
            &[SgrAttribute::BackgroundColor(ColorSpec::Rgb {
                r: 100,
                g: 200,
                b: 50,
            })],
            &mut cell,
        );
        assert_eq!(cell.background, Color::new(100, 200, 50));
    }

    #[test]
    fn sgr_fg_indexed() {
        let mut cell = Cell::default();
        apply_sgr(
            &[SgrAttribute::ForegroundColor(ColorSpec::Indexed(200))],
            &mut cell,
        );
        assert_eq!(cell.foreground, Color::from_ansi(200));
    }

    #[test]
    fn sgr_default_fg() {
        let mut cell = Cell {
            foreground: Color::new(100, 100, 100),
            ..Cell::default()
        };
        apply_sgr(&[SgrAttribute::DefaultForegroundColor], &mut cell);
        assert_eq!(cell.foreground, Color::default());
    }

    #[test]
    fn sgr_default_bg() {
        let mut cell = Cell {
            background: Color::new(100, 100, 100),
            ..Cell::default()
        };
        apply_sgr(&[SgrAttribute::DefaultBackgroundColor], &mut cell);
        assert_eq!(cell.background, Color::default());
    }

    #[test]
    fn sgr_multiple_attrs() {
        let mut cell = Cell::default();
        apply_sgr(
            &[
                SgrAttribute::Bold(true),
                SgrAttribute::Italic(true),
                SgrAttribute::ForegroundColor(ColorSpec::Named(1)),
            ],
            &mut cell,
        );
        assert!(cell.attrs.bold);
        assert!(cell.attrs.italic);
        assert_eq!(cell.foreground, Color::from_ansi(1));
    }

    #[test]
    fn sgr_from_code_all_valid() {
        // All codes that should return Some
        for code in [
            0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 21, 22, 23, 24, 25, 27, 28, 29, 53, 55,
        ] {
            assert!(
                SgrAttribute::from_code(code).is_some(),
                "Code {} should be valid",
                code
            );
        }
    }

    #[test]
    fn sgr_from_code_invalid() {
        // Codes that should return None
        for code in [10, 11, 12, 20, 26, 30, 40, 50, 54, 56, 100] {
            assert!(
                SgrAttribute::from_code(code).is_none(),
                "Code {} should be invalid",
                code
            );
        }
    }

    #[test]
    fn sgr_foreground_color_30_37() {
        for i in 30..=37u16 {
            let attr = SgrAttribute::foreground_color(i).unwrap();
            if let SgrAttribute::ForegroundColor(ColorSpec::Named(idx)) = attr {
                assert_eq!(idx, (i - 30) as u8);
            } else {
                panic!("Expected ForegroundColor");
            }
        }
    }

    #[test]
    fn sgr_foreground_color_90_97() {
        for i in 90..=97u16 {
            let attr = SgrAttribute::foreground_color(i).unwrap();
            if let SgrAttribute::ForegroundColor(ColorSpec::Named(idx)) = attr {
                assert_eq!(idx, (i - 90 + 8) as u8);
            } else {
                panic!("Expected ForegroundColor");
            }
        }
    }

    #[test]
    fn sgr_foreground_color_39_default() {
        assert_eq!(
            SgrAttribute::foreground_color(39),
            Some(SgrAttribute::DefaultForegroundColor)
        );
    }

    #[test]
    fn sgr_background_color_40_47() {
        for i in 40..=47u16 {
            let attr = SgrAttribute::background_color(i).unwrap();
            if let SgrAttribute::BackgroundColor(ColorSpec::Named(idx)) = attr {
                assert_eq!(idx, (i - 40) as u8);
            } else {
                panic!("Expected BackgroundColor");
            }
        }
    }

    #[test]
    fn sgr_background_color_100_107() {
        for i in 100..=107u16 {
            let attr = SgrAttribute::background_color(i).unwrap();
            if let SgrAttribute::BackgroundColor(ColorSpec::Named(idx)) = attr {
                assert_eq!(idx, (i - 100 + 8) as u8);
            } else {
                panic!("Expected BackgroundColor");
            }
        }
    }

    #[test]
    fn sgr_background_color_49_default() {
        assert_eq!(
            SgrAttribute::background_color(49),
            Some(SgrAttribute::DefaultBackgroundColor)
        );
    }

    #[test]
    fn color_from_spec_named() {
        let c = color_from_spec(&ColorSpec::Named(9));
        assert_eq!(c, Color::from_ansi(9));
    }

    #[test]
    fn color_from_spec_indexed() {
        let c = color_from_spec(&ColorSpec::Indexed(200));
        assert_eq!(c, Color::from_ansi(200));
    }

    #[test]
    fn color_from_spec_rgb() {
        let c = color_from_spec(&ColorSpec::Rgb { r: 1, g: 2, b: 3 });
        assert_eq!(c, Color::new(1, 2, 3));
    }

    #[test]
    fn parse_sgr_single() {
        let attrs = parse_sgr(&[1]);
        assert_eq!(attrs.len(), 1);
        assert_eq!(attrs[0], SgrAttribute::Bold(true));
    }

    #[test]
    fn parse_sgr_multiple() {
        let attrs = parse_sgr(&[1, 3, 4]);
        assert_eq!(attrs.len(), 3);
        assert_eq!(attrs[0], SgrAttribute::Bold(true));
        assert_eq!(attrs[1], SgrAttribute::Italic(true));
        assert_eq!(attrs[2], SgrAttribute::Underline(UnderlineStyle::Single));
    }

    #[test]
    fn parse_sgr_fg_256() {
        let attrs = parse_sgr(&[38, 5, 128]);
        assert_eq!(attrs.len(), 1);
        assert_eq!(
            attrs[0],
            SgrAttribute::ForegroundColor(ColorSpec::Indexed(128))
        );
    }

    #[test]
    fn parse_sgr_fg_rgb() {
        let attrs = parse_sgr(&[38, 2, 255, 128, 64]);
        assert_eq!(attrs.len(), 1);
        assert_eq!(
            attrs[0],
            SgrAttribute::ForegroundColor(ColorSpec::Rgb {
                r: 255,
                g: 128,
                b: 64
            })
        );
    }

    #[test]
    fn parse_sgr_bg_256() {
        let attrs = parse_sgr(&[48, 5, 200]);
        assert_eq!(attrs.len(), 1);
        assert_eq!(
            attrs[0],
            SgrAttribute::BackgroundColor(ColorSpec::Indexed(200))
        );
    }

    #[test]
    fn parse_sgr_bg_rgb() {
        let attrs = parse_sgr(&[48, 2, 10, 20, 30]);
        assert_eq!(attrs.len(), 1);
        assert_eq!(
            attrs[0],
            SgrAttribute::BackgroundColor(ColorSpec::Rgb {
                r: 10,
                g: 20,
                b: 30
            })
        );
    }

    #[test]
    fn parse_sgr_empty() {
        let attrs = parse_sgr(&[]);
        assert!(attrs.is_empty());
    }

    #[test]
    fn parse_sgr_invalid_ignored() {
        let attrs = parse_sgr(&[1, 999, 3]);
        assert_eq!(attrs.len(), 2);
    }

    #[test]
    fn sgr_bold_and_reverse() {
        let mut cell = Cell::default();
        apply_sgr(
            &[SgrAttribute::Bold(true), SgrAttribute::Reverse(true)],
            &mut cell,
        );
        assert!(cell.attrs.bold);
        assert!(cell.attrs.reverse);
    }

    #[test]
    fn sgr_italic_off() {
        let mut cell = Cell::default();
        cell.attrs.italic = true;
        apply_sgr(&[SgrAttribute::Italic(false)], &mut cell);
        assert!(!cell.attrs.italic);
    }

    #[test]
    fn sgr_strikethrough_off() {
        let mut cell = Cell::default();
        cell.attrs.strikethrough = true;
        apply_sgr(&[SgrAttribute::Strikethrough(false)], &mut cell);
        assert!(!cell.attrs.strikethrough);
    }

    #[test]
    fn sgr_reverse_off() {
        let mut cell = Cell::default();
        cell.attrs.reverse = true;
        apply_sgr(&[SgrAttribute::Reverse(false)], &mut cell);
        assert!(!cell.attrs.reverse);
    }

    #[test]
    fn sgr_conceal_off() {
        let mut cell = Cell::default();
        cell.attrs.hidden = true;
        apply_sgr(&[SgrAttribute::Conceal(false)], &mut cell);
        assert!(!cell.attrs.hidden);
    }

    #[test]
    fn sgr_overline_off() {
        let mut cell = Cell::default();
        cell.attrs.overline = true;
        apply_sgr(&[SgrAttribute::Overline(false)], &mut cell);
        assert!(!cell.attrs.overline);
    }

    #[test]
    fn sgr_dim_on() {
        let mut cell = Cell::default();
        apply_sgr(&[SgrAttribute::Faint(true)], &mut cell);
        assert!(cell.attrs.dim);
    }

    #[test]
    fn sgr_dim_off() {
        let mut cell = Cell::default();
        cell.attrs.dim = true;
        apply_sgr(&[SgrAttribute::Faint(false)], &mut cell);
        assert!(!cell.attrs.dim);
    }

    #[test]
    fn sgr_underline_curly() {
        let mut cell = Cell::default();
        apply_sgr(&[SgrAttribute::Underline(UnderlineStyle::Curly)], &mut cell);
        assert!(cell.attrs.underline);
        assert!(!cell.attrs.double_underline);
    }

    #[test]
    fn sgr_blink_rapid() {
        let mut cell = Cell::default();
        apply_sgr(&[SgrAttribute::Blink(BlinkStyle::Rapid)], &mut cell);
        assert!(cell.attrs.blink);
    }

    #[test]
    fn sgr_bold_21_off() {
        let mut cell = Cell::default();
        cell.attrs.bold = true;
        let attr = SgrAttribute::from_code(21).unwrap();
        assert_eq!(attr, SgrAttribute::Underline(UnderlineStyle::Double));
        apply_sgr(&[attr], &mut cell);
        assert!(
            cell.attrs.bold,
            "SGR 21 must not clear bold (it means double underline)"
        );
        assert!(
            cell.attrs.double_underline,
            "SGR 21 must set double underline"
        );
    }

    #[test]
    fn sgr_22_normal_intensity() {
        let mut cell = Cell::default();
        cell.attrs.bold = true;
        cell.attrs.dim = true;
        cell.foreground = Color::new(255, 0, 0);
        cell.background = Color::new(0, 255, 0);
        cell.attrs.italic = true;
        let attr = SgrAttribute::from_code(22).unwrap();
        apply_sgr(&[attr], &mut cell);
        assert!(!cell.attrs.bold);
        assert!(!cell.attrs.dim);
        assert_eq!(
            cell.foreground,
            Color::new(255, 0, 0),
            "SGR 22 must not destroy fg color"
        );
        assert_eq!(
            cell.background,
            Color::new(0, 255, 0),
            "SGR 22 must not destroy bg color"
        );
        assert!(
            cell.attrs.italic,
            "SGR 22 must not destroy other attributes"
        );
    }

    #[test]
    fn sgr_25_blink_off() {
        let mut cell = Cell::default();
        cell.attrs.blink = true;
        let attr = SgrAttribute::from_code(25).unwrap();
        apply_sgr(&[attr], &mut cell);
        assert!(!cell.attrs.blink);
    }

    // ── ColorSpec edge cases ──

    #[test]
    fn color_from_spec_named_extremes() {
        assert_eq!(color_from_spec(&ColorSpec::Named(0)), Color::from_ansi(0));
        assert_eq!(color_from_spec(&ColorSpec::Named(15)), Color::from_ansi(15));
    }

    #[test]
    fn color_from_spec_indexed_extremes() {
        assert_eq!(color_from_spec(&ColorSpec::Indexed(0)), Color::from_ansi(0));
        assert_eq!(
            color_from_spec(&ColorSpec::Indexed(255)),
            Color::from_ansi(255)
        );
    }

    #[test]
    fn color_from_spec_rgb_extremes() {
        let black = color_from_spec(&ColorSpec::Rgb { r: 0, g: 0, b: 0 });
        assert_eq!(black, Color::new(0, 0, 0));
        let white = color_from_spec(&ColorSpec::Rgb {
            r: 255,
            g: 255,
            b: 255,
        });
        assert_eq!(white, Color::new(255, 255, 255));
        let mid = color_from_spec(&ColorSpec::Rgb {
            r: 128,
            g: 128,
            b: 128,
        });
        assert_eq!(mid, Color::new(128, 128, 128));
    }

    #[test]
    fn color_from_spec_named_all_16() {
        for i in 0..16u8 {
            assert_eq!(
                color_from_spec(&ColorSpec::Named(i)),
                Color::from_ansi(i),
                "Named({i}) should map to from_ansi({i})"
            );
        }
    }

    // ── SGR reset clears all attributes ──

    #[test]
    fn sgr_reset_clears_every_attribute() {
        let mut cell = Cell::default();
        // Set every attribute to a non-default value
        cell.attrs.bold = true;
        cell.attrs.dim = true;
        cell.attrs.italic = true;
        cell.attrs.underline = true;
        cell.attrs.double_underline = true;
        cell.attrs.reverse = true;
        cell.attrs.strikethrough = true;
        cell.attrs.blink = true;
        cell.attrs.hidden = true;
        cell.attrs.overline = true;
        cell.attrs.protected = true;
        cell.attrs.double_width = true;
        cell.attrs.double_height_top = true;
        cell.attrs.double_height_bottom = true;
        cell.foreground = Color::new(100, 50, 25);
        cell.background = Color::new(200, 150, 100);
        cell.width = 2;

        apply_sgr(&[SgrAttribute::Reset], &mut cell);

        assert_eq!(cell.foreground, Color::default(), "fg must reset");
        assert_eq!(cell.background, Color::default(), "bg must reset");
        assert!(!cell.attrs.bold, "bold must reset");
        assert!(!cell.attrs.dim, "dim must reset");
        assert!(!cell.attrs.italic, "italic must reset");
        assert!(!cell.attrs.underline, "underline must reset");
        assert!(!cell.attrs.double_underline, "double_underline must reset");
        assert!(!cell.attrs.reverse, "reverse must reset");
        assert!(!cell.attrs.strikethrough, "strikethrough must reset");
        assert!(!cell.attrs.blink, "blink must reset");
        assert!(!cell.attrs.hidden, "hidden must reset");
        assert!(!cell.attrs.overline, "overline must reset");
        assert!(!cell.attrs.protected, "protected must reset");
        assert!(!cell.attrs.double_width, "double_width must reset");
        assert!(
            !cell.attrs.double_height_top,
            "double_height_top must reset"
        );
        assert!(
            !cell.attrs.double_height_bottom,
            "double_height_bottom must reset"
        );
    }

    #[test]
    fn sgr_bold_then_normal_intensity_clears() {
        let mut cell = Cell::default();
        apply_sgr(&[SgrAttribute::Bold(true)], &mut cell);
        assert!(cell.attrs.bold);
        apply_sgr(&[SgrAttribute::NormalIntensity], &mut cell);
        assert!(!cell.attrs.bold, "NormalIntensity must clear bold");
        assert!(!cell.attrs.dim, "NormalIntensity must clear dim");
    }

    #[test]
    fn sgr_reverse_then_off() {
        let mut cell = Cell::default();
        apply_sgr(&[SgrAttribute::Reverse(true)], &mut cell);
        assert!(cell.attrs.reverse);
        apply_sgr(&[SgrAttribute::Reverse(false)], &mut cell);
        assert!(!cell.attrs.reverse);
    }

    #[test]
    fn sgr_stacking_bold_italic_underline_strikethrough() {
        let mut cell = Cell::default();
        apply_sgr(
            &[
                SgrAttribute::Bold(true),
                SgrAttribute::Italic(true),
                SgrAttribute::Underline(UnderlineStyle::Single),
                SgrAttribute::Strikethrough(true),
            ],
            &mut cell,
        );
        assert!(cell.attrs.bold);
        assert!(cell.attrs.italic);
        assert!(cell.attrs.underline);
        assert!(cell.attrs.strikethrough);
    }

    #[test]
    fn sgr_reset_in_middle_preserves_later_attrs() {
        let mut cell = Cell::default();
        apply_sgr(
            &[
                SgrAttribute::Bold(true),
                SgrAttribute::Reset,
                SgrAttribute::Italic(true),
            ],
            &mut cell,
        );
        assert!(!cell.attrs.bold, "Reset must clear bold");
        assert!(cell.attrs.italic, "Italic after reset must remain");
        assert!(!cell.attrs.underline, "Reset must not leave underline set");
    }
}
