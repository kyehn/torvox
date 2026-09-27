use super::internal::snapshot_needs_rebuild;

#[test]
fn rebuild_required_on_first_call_without_cache() {
    assert!(snapshot_needs_rebuild(false, false));
}

#[test]
fn rebuild_skipped_when_cache_present_and_unchanged() {
    // 网格未变且已有缓存 → 复用，跳过逐单元格 ghostty FFI。
    assert!(!snapshot_needs_rebuild(false, true));
}

#[test]
fn rebuild_required_when_grid_dirty() {
    assert!(snapshot_needs_rebuild(true, true));
    assert!(snapshot_needs_rebuild(true, false));
}
