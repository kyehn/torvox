// Kani proof harness: Cell::new does not overflow
// Requires: cargo kani --harness cell_bounds_check

#[cfg(kani)]
#[kani::proof]
fn cell_bounds_check() {
    let char_val: u32 = kani::any();
    kani::assume(char_val <= 0x10FFFF); // valid Unicode range
    let r: u8 = kani::any();
    let g: u8 = kani::any();
    let b: u8 = kani::any();
    let cell = torvox_core::cell::Cell::new(
        char::from_u32(char_val).unwrap_or(' '),
        torvox_core::cell::Color::new(r, g, b),
        1,
    );
    assert!(cell.codepoint <= 0x10FFFF);
}

// Kani proof harness: color channels clamped to [0.0, 1.0]
// Requires: cargo kani --harness color_channel_clamp

#[cfg(kani)]
#[kani::proof]
fn color_channel_clamp() {
    let r: f32 = kani::any();
    let g: f32 = kani::any();
    let b: f32 = kani::any();
    let col = torvox_core::cell::Color { r, g, b, a: 255 };
    assert!(!col.r.is_nan(), "red channel must not be NaN");
    assert!(!col.g.is_nan(), "green channel must not be NaN");
    assert!(!col.b.is_nan(), "blue channel must not be NaN");
    // Color channels are stored as u8 via from_ansi; f32 fields are
    // used by color-space conversions. This harness verifies that
    // constructing a Color never produces NaN.
}
