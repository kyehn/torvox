use super::{KittyPlacementKeys, kitty_cache_invalidation};

fn keys() -> KittyPlacementKeys {
    KittyPlacementKeys {
        session_id: 7,
        generation: 3,
        placement_generation: 11,
        placeholder_signature: 0xabcd,
        rows: 24,
        cols: 80,
    }
}

/// 任一放置键变化都必须重查放置。
///
/// 判别力：键集合此前是渲染状态里的散落字段，逐字段比较易漏——「占位符签名」
/// 曾缺失时 TUI 移动图像后仍钉在旧位置；这些变化无法用旧缓存表达，只能重查。
#[test]
fn content_key_change_triggers_refetch() {
    let previous = keys();
    for changed in [
        KittyPlacementKeys {
            session_id: 8,
            ..previous
        },
        KittyPlacementKeys {
            generation: 4,
            ..previous
        },
        KittyPlacementKeys {
            placement_generation: 12,
            ..previous
        },
        KittyPlacementKeys {
            placeholder_signature: 0x1234,
            ..previous
        },
        KittyPlacementKeys {
            rows: 25,
            ..previous
        },
        KittyPlacementKeys {
            cols: 81,
            ..previous
        },
    ] {
        let invalidation = kitty_cache_invalidation(changed, previous, false);
        assert!(
            invalidation.refetch_placements,
            "放置键变化必须重查放置: {changed:?}"
        );
        assert!(
            invalidation.overlay_dirty,
            "放置键变化必须整帧重绘以去掉累加器里的旧图像: {changed:?}"
        );
    }
}

/// 放置生成戳变化必须重查：这是上游写明的契约（「值变了就读一次列表」）。
///
/// 判别力：ED2 在提示符行走上游 scrollClear——旧行先滚进回滚，放置随后被撤下。
/// 图像存储生成戳不变（图像仍在回滚里），只有放置生成戳推进。旧实现把这种
/// 视口平移当成「用户滚动」复用旧帧，撤下的图像就留在累加器里，要等下一次
/// 重查才消失（用户实测「clear 一下不清，两下才清」）。
#[test]
fn placement_generation_change_refetches_without_repacking_atlas() {
    let previous = keys();
    let cleared_placements = KittyPlacementKeys {
        placement_generation: previous.placement_generation + 1,
        ..previous
    };
    let invalidation = kitty_cache_invalidation(cleared_placements, previous, false);
    assert!(invalidation.refetch_placements, "放置撤下必须重查");
    assert!(invalidation.overlay_dirty, "旧图像像素必须整帧擦除");
    assert!(
        !invalidation.repack_atlas,
        "图像内容未变，不应重传图集（放置生成戳同样随滚动变化）"
    );
}

/// 滚动推进放置生成戳，故滚动也走同一条重查路径。
///
/// 判别力：旧实现为滚动单开一条「视口差量平移」通道，与上游契约冲突——
/// 上游明确写着放置随视口移动就要推进生成戳。平移近似帧既慢又易错
/// （撤下的放置无法用平移表达），删除后滚动与撤下共用一个信号。
#[test]
fn scroll_advancing_placement_generation_refetches() {
    let previous = keys();
    let scrolled = KittyPlacementKeys {
        placement_generation: previous.placement_generation + 1,
        ..previous
    };
    let invalidation = kitty_cache_invalidation(scrolled, previous, false);
    assert!(invalidation.refetch_placements, "放置随滚动移动必须重查");
    assert!(invalidation.overlay_dirty, "图像随视口移动须整帧重绘");
    assert!(!invalidation.repack_atlas, "滚动不换图像内容");
}

/// 渲染态初值的哨兵键不得与真实键相等：否则首帧被误判成「键未变」。
///
/// 判别力：哨兵取「不可能与真实值相等」的极值；一旦被改成真实值范围，
/// 首帧会跳过重查与整帧重绘，残留图像像素会黏在首帧上。
#[test]
fn sentinel_keys_force_first_frame_repaint() {
    let sentinel = KittyPlacementKeys::sentinel();
    let plain = KittyPlacementKeys {
        generation: 0,
        ..sentinel
    };
    let invalidation = kitty_cache_invalidation(plain, sentinel, false);
    assert!(!invalidation.refetch_placements, "纯文本首帧不查放置");
    assert!(invalidation.overlay_dirty, "首帧须整帧重绘");
}

/// 键与几何都没变时不重查、不重绘：否则每帧都重解码整幅图。
#[test]
fn unchanged_keys_keep_cache() {
    let previous = keys();
    let invalidation = kitty_cache_invalidation(previous, previous, false);
    assert!(!invalidation.refetch_placements);
    assert!(!invalidation.overlay_dirty);
    assert!(!invalidation.sync_cell_size);
    assert!(!invalidation.repack_atlas);
}

/// 单元格几何变化要先同步尺寸到终端侧再重查：旧帧像素尺寸按旧几何解算，
/// 不同步则图像按错误尺寸重算。
#[test]
fn cell_geometry_change_syncs_size_before_refetch() {
    let previous = keys();
    let invalidation = kitty_cache_invalidation(previous, previous, true);
    assert!(invalidation.sync_cell_size, "几何变化须同步尺寸到终端侧");
    assert!(invalidation.refetch_placements, "几何变化须重查放置");
    assert!(invalidation.overlay_dirty);
    assert!(!invalidation.repack_atlas, "几何变化不换图集");
}

/// 换会话要同步尺寸并重打包图集：新会话的图像集与几何都与旧会话无关。
#[test]
fn session_change_syncs_size_and_repacks_atlas() {
    let previous = keys();
    let changed = KittyPlacementKeys {
        session_id: 8,
        ..previous
    };
    let invalidation = kitty_cache_invalidation(changed, previous, false);
    assert!(invalidation.sync_cell_size, "换会话须把尺寸同步到新会话");
    assert!(invalidation.refetch_placements);
    assert!(invalidation.repack_atlas, "换会话须重打包图集");
    assert!(invalidation.overlay_dirty);
}

/// 生成戳变化表示图像增删：须重打包图集，但不必改单元格尺寸。
#[test]
fn generation_change_repacks_atlas_only() {
    let previous = keys();
    let changed = KittyPlacementKeys {
        generation: 4,
        ..previous
    };
    let invalidation = kitty_cache_invalidation(changed, previous, false);
    assert!(invalidation.repack_atlas, "生成戳变化须重打包图集");
    assert!(invalidation.refetch_placements);
    assert!(!invalidation.sync_cell_size, "生成戳变化不涉及单元格尺寸");
}

/// 纯文本会话（生成戳 0）不触发任何图像路径，但撤下图像时仍须整帧重绘，
/// 否则累加器里残留的图像像素不会消失。
#[test]
fn pure_text_session_skips_image_paths_but_clears_residue() {
    let previous = keys();
    let plain = KittyPlacementKeys {
        generation: 0,
        ..previous
    };
    let invalidation = kitty_cache_invalidation(plain, previous, false);
    assert!(!invalidation.refetch_placements, "纯文本不重查放置");
    assert!(!invalidation.sync_cell_size);
    assert!(!invalidation.repack_atlas);
    assert!(
        invalidation.overlay_dirty,
        "图像撤下后须整帧重绘以清掉残留像素"
    );

    let still_plain = kitty_cache_invalidation(plain, plain, false);
    assert!(!still_plain.overlay_dirty, "纯文本会话状态未变时不重绘");
}
