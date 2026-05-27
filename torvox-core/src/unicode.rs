use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum UnicodeWidth {
    Zero,
    Single,
    Double,
}

pub fn width(c: char) -> UnicodeWidth {
    let w = unicode_width_char(c);
    match w {
        0 => UnicodeWidth::Zero,
        1 => UnicodeWidth::Single,
        _ => UnicodeWidth::Double,
    }
}

pub fn width_val(c: char) -> u8 {
    unicode_width_char(c)
}

fn unicode_width_char(c: char) -> u8 {
    if ('\0'..='\u{001F}').contains(&c) || ('\u{007F}'..='\u{009F}').contains(&c) {
        return 0;
    }
    if ('\u{0300}'..='\u{036F}').contains(&c) {
        return 0;
    }
    if c < '\u{1100}' {
        return 1;
    }
    if ('\u{1100}'..='\u{115F}').contains(&c) {
        return 2;
    }
    if ('\u{2329}'..='\u{232A}').contains(&c) {
        return 2;
    }
    if ('\u{2E80}'..='\u{303E}').contains(&c) {
        return 2;
    }
    if ('\u{3041}'..='\u{3247}').contains(&c) {
        return 2;
    }
    if ('\u{3251}'..='\u{4DBF}').contains(&c) {
        return 2;
    }
    if ('\u{4E00}'..='\u{A4C6}').contains(&c) {
        return 2;
    }
    if ('\u{A960}'..='\u{A97C}').contains(&c) {
        return 2;
    }
    if ('\u{AC00}'..='\u{D7A3}').contains(&c) {
        return 2;
    }
    if ('\u{F900}'..='\u{FAFF}').contains(&c) {
        return 2;
    }
    if ('\u{FE10}'..='\u{FE19}').contains(&c) {
        return 2;
    }
    if ('\u{FE30}'..='\u{FE6B}').contains(&c) {
        return 2;
    }
    if ('\u{FF01}'..='\u{FF60}').contains(&c) {
        return 2;
    }
    if ('\u{FFE0}'..='\u{FFE6}').contains(&c) {
        return 2;
    }
    if ('\u{1B000}'..='\u{1B001}').contains(&c) {
        return 2;
    }
    if ('\u{1F200}'..='\u{1F251}').contains(&c) {
        return 2;
    }
    if ('\u{20000}'..='\u{3FFFD}').contains(&c) {
        return 2;
    }
    if matches!(
        c,
        '\u{20A9}' | '\u{2190}'..='\u{2193}'
            | '\u{2605}'..='\u{2606}'
            | '\u{2FF0}'..='\u{2FFB}'
            | '\u{3000}'
            | '\u{3038}'..='\u{303A}'
            | '\u{31C0}'..='\u{31E3}'
            | '\u{31F0}'..='\u{321E}'
            | '\u{3220}'..='\u{3247}'
    ) {
        return 2;
    }
    1
}

pub fn str_width(s: &str) -> u32 {
    s.chars().map(|c| width_val(c) as u32).sum()
}

pub fn is_wide(c: char) -> bool {
    width_val(c) == 2
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ascii_width() {
        assert_eq!(width_val('A'), 1);
        assert_eq!(width_val('z'), 1);
        assert_eq!(width_val(' '), 1);
    }

    #[test]
    fn control_width_zero() {
        assert_eq!(width_val('\0'), 0);
        assert_eq!(width_val('\u{001F}'), 0);
        assert_eq!(width_val('\u{007F}'), 0);
    }

    #[test]
    fn cjk_width() {
        assert_eq!(width_val('\u{4E00}'), 2);
        assert_eq!(width_val('中'), 2);
        assert_eq!(width_val('文'), 2);
    }

    #[test]
    fn hangul_width() {
        assert_eq!(width_val('\u{AC00}'), 2);
    }

    #[test]
    fn fullwidth_width() {
        assert_eq!(width_val('\u{FF01}'), 2);
    }

    #[test]
    fn combining_zero() {
        assert_eq!(width('\u{0300}'), UnicodeWidth::Zero);
    }

    #[test]
    fn str_width_ascii() {
        assert_eq!(str_width("hello"), 5);
    }

    #[test]
    fn str_width_mixed() {
        assert_eq!(str_width("A中B"), 4);
    }

    #[test]
    fn is_wide_check() {
        assert!(is_wide('中'));
        assert!(!is_wide('A'));
    }

    #[test]
    fn unicode_width_enum() {
        assert_eq!(width('A'), UnicodeWidth::Single);
        assert_eq!(width('中'), UnicodeWidth::Double);
    }
}
