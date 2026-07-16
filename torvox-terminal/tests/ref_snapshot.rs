#[cfg(test)]
extern crate torvox_terminal;

use torvox_terminal::snapshot_test::run_ref_test_dir;

/// Standard terminal dimensions for regression tests.
const ROWS: u32 = 24;
const COLS: u32 = 80;
const SCROLLBACK: u32 = 1000;

#[test]
fn ref_cursor() {
    run_ref_test_dir("cursor", ROWS, COLS, SCROLLBACK);
}

#[test]
fn ref_text() {
    run_ref_test_dir("text", ROWS, COLS, SCROLLBACK);
}

#[test]
fn ref_erase() {
    run_ref_test_dir("erase", ROWS, COLS, SCROLLBACK);
}

#[test]
fn ref_sgr() {
    run_ref_test_dir("sgr", ROWS, COLS, SCROLLBACK);
}

#[test]
fn ref_scroll() {
    run_ref_test_dir("scroll", ROWS, COLS, SCROLLBACK);
}

#[test]
fn ref_mode() {
    run_ref_test_dir("mode", ROWS, COLS, SCROLLBACK);
}

#[test]
fn ref_tab() {
    run_ref_test_dir("tab", ROWS, COLS, SCROLLBACK);
}

#[test]
fn ref_resize() {
    run_ref_test_dir("resize", ROWS, COLS, SCROLLBACK);
}

#[test]
fn ref_wrap() {
    run_ref_test_dir("wrap", ROWS, COLS, SCROLLBACK);
}

#[test]
fn ref_scrollback_0() {
    run_ref_test_dir("scrollback-0", ROWS, COLS, 0);
}

#[test]
fn ref_scrollback_100k() {
    run_ref_test_dir("scrollback-100k", ROWS, COLS, 100_000);
}

#[test]
fn ref_decsc_bottom() {
    run_ref_test_dir("decsc-bottom", ROWS, COLS, SCROLLBACK);
}

#[test]
fn ref_decsc_top() {
    run_ref_test_dir("decsc-top", ROWS, COLS, SCROLLBACK);
}

#[test]
fn ref_decsc_mid() {
    run_ref_test_dir("decsc-mid", ROWS, COLS, SCROLLBACK);
}

#[test]
fn ref_sgr_bold_italic() {
    run_ref_test_dir("sgr-bold-italic", ROWS, COLS, SCROLLBACK);
}

#[test]
fn ref_sgr_underline_overline() {
    run_ref_test_dir("sgr-underline-overline", ROWS, COLS, SCROLLBACK);
}

#[test]
fn ref_sgr_blink_reverse() {
    run_ref_test_dir("sgr-blink-reverse", ROWS, COLS, SCROLLBACK);
}

#[test]
fn ref_tab_custom() {
    run_ref_test_dir("tab-custom", ROWS, COLS, SCROLLBACK);
}

#[test]
fn ref_origin_scroll() {
    run_ref_test_dir("origin-scroll", ROWS, COLS, SCROLLBACK);
}

#[test]
fn ref_app_cursor_keys() {
    run_ref_test_dir("app-cursor-keys", ROWS, COLS, SCROLLBACK);
}

#[test]
fn ref_mouse_reporting() {
    run_ref_test_dir("mouse-reporting", ROWS, COLS, SCROLLBACK);
}
