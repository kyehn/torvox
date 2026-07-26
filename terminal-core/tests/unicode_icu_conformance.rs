use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

/// CJK unified ideographs should have width 2
#[test]
fn cjk_ideograph_width() {
    let cjk_chars = ["\u{4e2d}", "\u{56fd}", "\u{6587}", "\u{5b57}", "\u{7f51}"];
    for &ch in &cjk_chars {
        assert_eq!(
            terminal_core::unicode::string_width(ch),
            2,
            "CJK char U+{:04X} should have width 2",
            ch.chars().next().unwrap() as u32
        );
    }
}

/// Fullwidth Latin characters should have width 2
#[test]
fn fullwidth_latin_width() {
    let fullwidth = ['\u{FF21}', '\u{FF36}', '\u{FF41}', '\u{FF5A}'];
    for &ch in &fullwidth {
        assert_eq!(
            ch.width(),
            Some(2),
            "Fullwidth U+{:04X} should have width 2",
            ch as u32
        );
    }
}

/// unicode-width reports the display width of an emoji ZWJ sequence.
/// The sequence "man + ZWJ + white hair" has display width 2 (emoji width).
#[test]
fn emoji_zwj_sequences_width() {
    let seq = "\u{1F468}\u{200D}\u{1F9B3}";
    let total = seq.width();
    assert_eq!(
        total, 2,
        "emoji ZWJ sequence (man + ZWJ + white hair) should have display width 2, got {total}"
    );
}

/// Regional indicator pair (flag) should have width 2
#[test]
fn regional_indicator_pair_width() {
    let flags = [
        "\u{1F1E8}\u{1F1F3}",
        "\u{1F1FA}\u{1F1F8}",
        "\u{1F1EF}\u{1F1F5}",
        "\u{1F1F0}\u{1F1F7}",
    ];
    for &flag in &flags {
        assert_eq!(
            terminal_core::unicode::string_width(flag),
            2,
            "flag should have width 2"
        );
    }
}

/// ASCII letters have width 1
#[test]
fn ascii_width() {
    for ch in 'a'..='z' {
        assert_eq!(ch.width(), Some(1), "ASCII {ch} should have width 1");
    }
}

/// Control characters have width 0 (via .unwrap_or(0))
#[test]
fn control_chars_width_0() {
    let ctrl = ['\u{0000}', '\u{0001}', '\u{0007}', '\u{001B}', '\u{007F}'];
    for &ch in &ctrl {
        assert_eq!(
            terminal_core::unicode::width_value(ch),
            0,
            "control U+{:04X} should have width 0",
            ch as u32
        );
    }
}

/// Combining characters have width 0
#[test]
fn combining_chars_width() {
    let combining = [
        '\u{0300}', '\u{0301}', '\u{0308}', '\u{20D0}', '\u{FE00}', '\u{FE01}', '\u{FE0F}',
    ];
    for &ch in &combining {
        assert_eq!(
            terminal_core::unicode::width_value(ch),
            0,
            "combining U+{:04X} should have width 0",
            ch as u32
        );
    }
}

/// Korean jamo should have width 2
#[test]
fn korean_jamo_width() {
    assert_eq!(
        terminal_core::unicode::width_value('\u{AC00}'),
        2,
        "Hangul U+AC00 should have width 2"
    );
    assert_eq!(
        terminal_core::unicode::width_value('\u{D7A3}'),
        2,
        "Hangul U+D7A3 should have width 2"
    );
}
