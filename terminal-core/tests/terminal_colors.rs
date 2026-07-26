use terminal_core::cell::Color;

/// Default color is opaque white (255,255,255,255)
#[test]
fn color_default_is_white() {
    let c = Color::default();
    assert_eq!(c.r, 255);
    assert_eq!(c.g, 255);
    assert_eq!(c.b, 255);
    assert_eq!(c.a, 255);
}

/// Color::new creates correct RGBA
#[test]
fn color_new_creates_exact() {
    let c = Color::new(128, 64, 32);
    assert_eq!(c.r, 128);
    assert_eq!(c.g, 64);
    assert_eq!(c.b, 32);
    assert_eq!(c.a, 255);
}

/// Color::from_ansi produces exact xterm-256color palette values for 0-7
#[test]
fn color_from_ansi_3bit_exact_values() {
    // xterm-256color standard ANSI palette (ANSI_16)
    let expected: [[u8; 3]; 8] = [
        [0, 0, 0],       // 0 black
        [128, 0, 0],     // 1 red
        [0, 128, 0],     // 2 green
        [128, 128, 0],   // 3 yellow
        [0, 0, 128],     // 4 blue
        [128, 0, 128],   // 5 magenta
        [0, 128, 128],   // 6 cyan
        [192, 192, 192], // 7 white
    ];
    for i in 0u8..8u8 {
        let c = Color::from_ansi(i);
        assert_eq!(c.r, expected[i as usize][0], "ANSI color {i} red component");
        assert_eq!(
            c.g, expected[i as usize][1],
            "ANSI color {i} green component"
        );
        assert_eq!(
            c.b, expected[i as usize][2],
            "ANSI color {i} blue component"
        );
    }
}

/// Color::from_ansi produces exact xterm-256color palette values for 8-15 (bright)
#[test]
fn color_from_ansi_4bit_bright_exact_values() {
    // xterm-256color bright ANSI palette (ANSI_16)
    let expected: [[u8; 3]; 8] = [
        [128, 128, 128], // 8 bright_black
        [255, 0, 0],     // 9 bright_red
        [0, 255, 0],     // 10 bright_green
        [255, 255, 0],   // 11 bright_yellow
        [0, 0, 255],     // 12 bright_blue
        [255, 0, 255],   // 13 bright_magenta
        [0, 255, 255],   // 14 bright_cyan
        [255, 255, 255], // 15 bright_white
    ];
    for i in 0u8..8u8 {
        let c = Color::from_ansi(i | 8);
        assert_eq!(
            c.r,
            expected[i as usize][0],
            "bright ANSI color {} red component",
            i | 8
        );
        assert_eq!(
            c.g,
            expected[i as usize][1],
            "bright ANSI color {} green component",
            i | 8
        );
        assert_eq!(
            c.b,
            expected[i as usize][2],
            "bright ANSI color {} blue component",
            i | 8
        );
    }
}

/// Black (0) and bright white (15) differ in all channels
#[test]
fn color_black_not_white() {
    let black = Color::from_ansi(0);
    let white = Color::from_ansi(15);
    assert_ne!(black, white, "ANSI black (0) should differ from white (15)");
    assert_eq!(black.r, 0, "palette 0 red (xterm)");
    assert_eq!(black.g, 0, "palette 0 green (xterm)");
    assert_eq!(black.b, 0, "palette 0 blue (xterm)");
    assert_eq!(white.r, 255);
    assert_eq!(white.g, 255);
    assert_eq!(white.b, 255);
}

/// Default alpha is always opaque
#[test]
fn color_default_is_opaque() {
    let c = Color::default();
    assert_eq!(c.a, 255, "default alpha should be opaque");
}

/// Truecolor round-trips correctly
#[test]
fn color_truecolor_preserves_values() {
    let c = Color::new(255, 0, 128);
    assert_eq!(c.r, 255);
    assert_eq!(c.g, 0);
    assert_eq!(c.b, 128);
}

/// All 16 standard ANSI colors are opaque
#[test]
fn color_ansi_all_opaque() {
    for i in 0u8..16u8 {
        let c = Color::from_ansi(i);
        assert_eq!(c.a, 255, "ANSI color {i} should be opaque");
    }
}

/// Color::from_ansi 216-color cube produces expected first and last entries
#[test]
fn color_ansi_216_cube_boundaries() {
    let c0 = Color::from_ansi(16);
    assert_eq!(c0.r, 0);
    assert_eq!(c0.g, 0);
    assert_eq!(c0.b, 0);
    let c231 = Color::from_ansi(231);
    assert_eq!(c231.r, 255);
    assert_eq!(c231.g, 255);
    assert_eq!(c231.b, 255);
}

/// Color::from_ansi grayscale ramp starts and ends correctly
#[test]
fn color_ansi_grayscale_boundaries() {
    let c232 = Color::from_ansi(232);
    assert_eq!(c232.r, 8);
    assert_eq!(c232.g, 8);
    assert_eq!(c232.b, 8);
    let c255 = Color::from_ansi(255);
    assert_eq!(c255.r, 238);
    assert_eq!(c255.g, 238);
    assert_eq!(c255.b, 238);
}
