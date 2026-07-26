//! Reliability tests for the VT-thread grid-snapshot cache (perf fix #6).
//!
//! The VT thread rebuilds the full grid snapshot (≈1920 per-cell ghostty FFI
//! calls) only when the grid content or scroll offset changed. These tests
//! prove, via the `snapshot_rebuild_count` instrument, that unchanged frames
//! are reused (no rebuild) and that a content/scroll change always triggers a
//! rebuild with current content (no stale frame).
//!
//! Note: a freshly spawned terminal runs a shell whose asynchronous PTY
//! output (prompt, etc.) sets `grid_dirty` and causes extra rebuilds at
//! unpredictable times. The tests therefore settle to a stable rebuild count
//! first, then assert *deltas* around an explicit action — this proves the
//! cache behavior without depending on the exact async rebuild total.

use std::thread;
use std::time::Duration;

use terminal_engine::ghostty_terminal::GhosttyTerminal;

/// Issue one snapshot at `offset` and let the VT thread drain its command channel.
fn tick(term: &GhosttyTerminal, offset: u32) {
    let _ = term.take_snapshot_with_scroll(offset);
    thread::sleep(Duration::from_millis(15));
}

/// Take snapshots until the rebuild count is stable for several consecutive
/// ticks (the shell has finished its initial async output), then return it.
fn stable_count(term: &GhosttyTerminal, offset: u32) -> u64 {
    let mut prev = term.snapshot_rebuild_count();
    let mut stable = 0;
    for _ in 0..30 {
        tick(term, offset);
        let now = term.snapshot_rebuild_count();
        if now == prev {
            stable += 1;
            if stable >= 3 {
                return now;
            }
        } else {
            stable = 0;
        }
        prev = now;
    }
    prev
}

#[test]
fn unchanged_frames_reuse_cache_and_never_rebuild() {
    let term = GhosttyTerminal::new(24, 80, 1000).unwrap();

    // Settle past any initial shell output.
    let stable = stable_count(&term, 0);
    assert!(stable >= 1, "initial snapshot must be built at least once");

    // Many more same-state frames must NOT trigger further rebuilds:
    // the cache is reused.
    for _ in 0..5 {
        tick(&term, 0);
    }
    assert_eq!(
        term.snapshot_rebuild_count(),
        stable,
        "unchanged frames must reuse the cached snapshot (no rebuild)"
    );
}

#[test]
fn content_change_triggers_rebuild_and_is_current() {
    let mut term = GhosttyTerminal::new(24, 80, 1000).unwrap();

    let stable = stable_count(&term, 0);
    assert!(stable >= 1);

    // Mutate the grid. Plain byte 'X' is a complete VT sequence.
    term.vt_write(b"X");

    // Flush the write plus a few frames through the VT thread.
    for _ in 0..5 {
        tick(&term, 0);
    }

    // The grid change must have triggered at least one rebuild.
    let after = term.snapshot_rebuild_count();
    assert!(
        after > stable,
        "a grid change must trigger a rebuild (before={stable}, after={after})"
    );

    // The rebuild produced CURRENT content — 'X' is present (no stale frame).
    let snap = term.take_snapshot_with_scroll(0);
    assert!(
        snap.cells.iter().any(|c| c.codepoint == b'X' as u32),
        "snapshot after change must reflect current grid content ('X')"
    );

    // And it stays cached afterwards (no further rebuilds while unchanged).
    let post_change = stable_count(&term, 0);
    assert_eq!(
        post_change, after,
        "frames after the change must reuse the cache again"
    );
}

#[test]
fn scroll_change_triggers_rebuild() {
    let term = GhosttyTerminal::new(24, 80, 5000).unwrap();

    let stable = stable_count(&term, 0);
    assert!(stable >= 1);

    // Shift the viewport: a different scroll offset shows different rows,
    // so the snapshot must be rebuilt even though the grid is unchanged.
    let _ = term.take_snapshot_with_scroll(10);
    for _ in 0..4 {
        tick(&term, 10);
    }
    let after_scroll = term.snapshot_rebuild_count();
    assert!(
        after_scroll > stable,
        "a scroll-offset change must trigger a rebuild (before={stable}, after={after_scroll})"
    );

    // Returning to a different scroll offset must rebuild again.
    let _ = term.take_snapshot_with_scroll(0);
    for _ in 0..4 {
        tick(&term, 0);
    }
    let after_back = term.snapshot_rebuild_count();
    assert!(
        after_back > after_scroll,
        "returning to a different scroll offset must rebuild (before={after_scroll}, after={after_back})"
    );
}
