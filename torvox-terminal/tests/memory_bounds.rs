//! Memory bounds tests: measures process RSS before/after VT operations.
//!
//! These tests ensure that processing large VT sequences does not cause
//! unbounded memory growth. They use /proc/self/statm (Linux-only).

use torvox_terminal::ghostty_terminal::GhosttyTerminal;

fn current_rss_kb() -> u64 {
    let statm = std::fs::read_to_string("/proc/self/statm").expect("/proc/self/statm");
    let resident_pages: u64 = statm
        .split_whitespace()
        .nth(1)
        .expect("resident set size")
        .parse()
        .expect("integer");
    // page size is 4096 on Linux
    resident_pages * 4
}

#[test]
fn vt_write_10k_lines_rss_bounded() {
    let rss_before = current_rss_kb();

    let mut terminal = GhosttyTerminal::new(24, 80, 10_000).expect("GhosttyTerminal::new");
    let line = b"AAAA BBBB CCCC DDDD EEEE FFFF GGGG HHHH IIII JJJJ KKKK LLLL MMMM\n";
    let mut input = Vec::with_capacity(line.len() * 1_000);
    for _ in 0..1_000 {
        input.extend_from_slice(line);
    }
    terminal.vt_write(&input);

    let rss_after = current_rss_kb();
    let growth = rss_after.saturating_sub(rss_before);
    // 10K lines of 64 bytes each should not cause > 512MB RSS growth
    assert!(
        growth < 512_000,
        "RSS growth {} KB exceeds 512 MB limit",
        growth
    );
}

#[test]
fn vt_write_scrollback_full_rss_bounded() {
    let rss_before = current_rss_kb();

    let mut terminal = GhosttyTerminal::new(24, 80, 5_000).expect("GhosttyTerminal::new");
    let line = b"A scrollback test line for history buffer\n";
    let mut input = Vec::with_capacity(line.len() * 2_500);
    for _ in 0..2_500 {
        input.extend_from_slice(line);
    }
    terminal.vt_write(&input);

    let rss_after = current_rss_kb();
    let growth = rss_after.saturating_sub(rss_before);
    // 25K lines with 100K scrollback capacity should not cause > 256MB RSS growth
    assert!(
        growth < 256_000,
        "RSS growth {} KB exceeds 256 MB limit",
        growth
    );
}

#[test]
fn vt_write_large_sgr_sequence_rss_bounded() {
    let rss_before = current_rss_kb();

    let mut terminal = GhosttyTerminal::new(24, 80, 5_000).expect("GhosttyTerminal::new");
    // SGR-heavy output — rapid color changes stress the parser
    let mut input = Vec::with_capacity(50_000);
    for _ in 0..2_000 {
        input.extend_from_slice(b"\x1b[38;5;");
        input.push(b'0' + (fast_rand() % 9) as u8);
        input.push(b'm');
        input.extend_from_slice(b"X");
    }
    terminal.vt_write(&input);

    let rss_after = current_rss_kb();
    let growth = rss_after.saturating_sub(rss_before);
    assert!(
        growth < 256_000,
        "SGR RSS growth {} KB exceeds 256 MB limit",
        growth
    );
}

/// Deterministic pseudo-random for test data generation (LCG with fixed seed 42).
fn fast_rand() -> u64 {
    use std::cell::Cell;
    thread_local! {
        static STATE: Cell<u64> = const { Cell::new(42) };
    }
    STATE.with(|s| {
        let x = s.get();
        let next = x
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        s.set(next);
        next
    })
}
