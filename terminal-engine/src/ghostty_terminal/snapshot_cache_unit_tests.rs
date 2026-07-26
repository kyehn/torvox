use super::snapshot_needs_rebuild;
#[test]
fn rebuild_required_on_first_call_without_cache() {
    assert!(snapshot_needs_rebuild(false, 0, 0, false));
}

#[test]
fn rebuild_skipped_when_cache_present_and_unchanged() {
    // grid unchanged, scroll unchanged, cache present → reuse.
    assert!(!snapshot_needs_rebuild(false, 0, 0, true));
    assert!(!snapshot_needs_rebuild(false, 42, 42, true));
}

#[test]
fn rebuild_required_when_grid_dirty() {
    assert!(snapshot_needs_rebuild(true, 0, 0, true));
    // grid dirty dominates even if scroll matches and cache present.
    assert!(snapshot_needs_rebuild(true, 7, 7, true));
}

#[test]
fn rebuild_required_when_scroll_offset_changes() {
    // grid unchanged but viewport moved → different rows shown.
    assert!(snapshot_needs_rebuild(false, 1, 0, true));
    assert!(snapshot_needs_rebuild(false, 100, 50, true));
}

#[test]
fn truth_table_exhaustive() {
    // (grid_dirty, scroll_same, has_cache) -> expect
    let cases = [
        (false, true, true, false),
        (false, true, false, true),
        (false, false, true, true),
        (false, false, false, true),
        (true, true, true, true),
        (true, true, false, true),
        (true, false, true, true),
        (true, false, false, true),
    ];
    for (grid_dirty, scroll_same, has_cache, expect) in cases {
        let scroll_offset = if scroll_same { 0 } else { 1 };
        let cached_scroll_offset = 0;
        assert_eq!(
            snapshot_needs_rebuild(grid_dirty, scroll_offset, cached_scroll_offset, has_cache),
            expect,
            "grid_dirty={grid_dirty} scroll_same={scroll_same} has_cache={has_cache}"
        );
    }
}
