use super::{KittyPlacementKeys, kitty_cache_invalidation};

fn keys() -> KittyPlacementKeys {
    KittyPlacementKeys {
        session_id: 7,
        generation: 3,
        placement_generation: 11,
        scroll_offset: 0,
        scrollback_length: 100,
        placeholder_signature: 0xabcd,
        rows: 24,
        cols: 80,
    }
}

/// 与视口无关的放置键变化必须重查放置。
///
/// 判别力：键集合此前是渲染状态里的散落字段，逐字段比较易漏——「占位符签名」
/// 曾缺失时 TUI 移动图像后仍钉在旧位置；这些变化无法用平移表达，只能重查。
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
        let invalidation = kitty_cache_invalidation(changed, previous, false, false);
        assert!(
            invalidation.refetch_placements,
            "放置键变化必须重查放置: {changed:?}"
        );
        assert!(
            invalidation.overlay_dirty,
            "放置键变化必须整帧重绘以去掉累加器里的旧图像: {changed:?}"
        );
        assert_eq!(invalidation.viewport_shift_rows, 0, "内容变化不靠平移");
    }
}

/// 放置生成戳变化而视口没动：必须重查，否则 ED2 撤下的图像留在缓存里。
///
/// 判别力：放置键此前只看图像存储生成戳；ED2 撤下放置不会改动图像存储，
/// 于是首次 clear 后渲染继续复用旧放置，图像要等第二次 clear 才消失。
#[test]
fn placement_generation_change_without_scroll_refetches() {
    let previous = keys();
    let cleared_placements = KittyPlacementKeys {
        placement_generation: previous.placement_generation + 1,
        ..previous
    };
    let invalidation = kitty_cache_invalidation(cleared_placements, previous, false, false);
    assert!(invalidation.refetch_placements, "放置撤下必须重查");
    assert!(invalidation.overlay_dirty, "旧图像像素必须整帧擦除");
    assert!(
        !invalidation.repack_atlas,
        "图像内容未变，不应重传图集（放置生成戳同样随滚动变化）"
    );
    assert_eq!(invalidation.viewport_shift_rows, 0);
}

/// 放置生成戳随滚动变化时仍走平移，不得退化为逐行同步查询。
#[test]
fn placement_generation_change_with_scroll_still_shifts() {
    let previous = keys();
    let scrolled = KittyPlacementKeys {
        placement_generation: previous.placement_generation + 1,
        scroll_offset: 1,
        ..previous
    };
    let invalidation = kitty_cache_invalidation(scrolled, previous, false, false);
    assert!(!invalidation.refetch_placements, "滚动中的放置变化只平移");
    assert!(invalidation.overlay_dirty);
    assert_eq!(invalidation.viewport_shift_rows, 1);
}

/// 纯滚动不重查放置：放置帧是视口相对坐标，按视口顶行差量平移即可。
///
/// 判别力：把滚动偏移放进「重查」集合时，每一行滚动都要在渲染线程同步等一次
/// VT 查询（yazi 预览下实测约 60ms/行），滚动掉到 ~16fps——用户主诉的卡顿。
#[test]
fn viewport_scroll_shifts_cached_frames_without_refetch() {
    let previous = keys();
    // 向上滚入历史一行：视口顶行 100 -> 99，图像在视口内下移一行。
    let scrolled = KittyPlacementKeys {
        scroll_offset: 1,
        ..previous
    };
    let invalidation = kitty_cache_invalidation(scrolled, previous, false, false);
    assert!(!invalidation.refetch_placements, "纯滚动不得同步查放置");
    assert!(!invalidation.sync_cell_size);
    assert!(!invalidation.repack_atlas);
    assert!(
        invalidation.overlay_dirty,
        "图像随视口移动，必须整帧重绘才能擦掉旧位置"
    );
    assert_eq!(invalidation.viewport_shift_rows, 1);

    // 底部新输出把行推入回滚：视口顶行 100 -> 101，图像在视口内上移一行。
    let pushed = KittyPlacementKeys {
        scrollback_length: 101,
        ..previous
    };
    let invalidation = kitty_cache_invalidation(pushed, previous, false, false);
    assert!(!invalidation.refetch_placements);
    assert_eq!(invalidation.viewport_shift_rows, -1);

    // 滚动偏移与回滚长度同时变化而视口顶行不变：屏幕内容没动，无需任何动作。
    let same_top = KittyPlacementKeys {
        scroll_offset: 1,
        scrollback_length: 101,
        ..previous
    };
    let invalidation = kitty_cache_invalidation(same_top, previous, false, false);
    assert!(!invalidation.refetch_placements);
    assert!(!invalidation.overlay_dirty);
    assert_eq!(invalidation.viewport_shift_rows, 0);
}

/// 滚动停稳后补一次真实查询：滚动期间平移的是旧视口裁剪过的源矩形，
/// 不补那次查询，新露出的边缘会一直缺一条。
#[test]
fn settled_scroll_refetches_approximate_frames_once() {
    let aligned = keys();
    let invalidation = kitty_cache_invalidation(aligned, aligned, false, true);
    assert!(invalidation.refetch_placements, "停稳后须补一次真实查询");
    assert!(invalidation.overlay_dirty);
    assert_eq!(invalidation.viewport_shift_rows, 0);

    // 帧已由真实查询刷新（非近似）时，停稳不会再查一次。
    let settled = kitty_cache_invalidation(aligned, aligned, false, false);
    assert!(!settled.refetch_placements);
    assert!(!settled.overlay_dirty);
}

/// 渲染态初值的哨兵键不得让失效判定溢出。
///
/// 判别力：视口顶行由 `回滚长度 - 滚动偏移` 求得，而「从未查询」的初值用
/// `scroll_offset = i64::MIN`；直接相减在 debug 构建里 panic（进程级崩溃），
/// release 则静默回绕成无意义值。
#[test]
fn sentinel_keys_do_not_overflow() {
    let sentinel = KittyPlacementKeys::sentinel();
    let plain = KittyPlacementKeys {
        generation: 0,
        scroll_offset: 0,
        scrollback_length: 100,
        ..sentinel
    };
    let invalidation = kitty_cache_invalidation(plain, sentinel, false, false);
    assert!(!invalidation.refetch_placements, "纯文本首帧不查放置");
    assert!(invalidation.overlay_dirty, "首帧须整帧重绘");
    assert_eq!(invalidation.viewport_shift_rows, 0);
}

/// 键与几何都没变时不重查、不重绘：否则每帧都重解码整幅图。
#[test]
fn unchanged_keys_keep_cache() {
    let previous = keys();
    let invalidation = kitty_cache_invalidation(previous, previous, false, false);
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
    let invalidation = kitty_cache_invalidation(previous, previous, true, false);
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
    let invalidation = kitty_cache_invalidation(changed, previous, false, false);
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
    let invalidation = kitty_cache_invalidation(changed, previous, false, false);
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
    let invalidation = kitty_cache_invalidation(plain, previous, false, false);
    assert!(!invalidation.refetch_placements, "纯文本不重查放置");
    assert!(!invalidation.sync_cell_size);
    assert!(!invalidation.repack_atlas);
    assert!(
        invalidation.overlay_dirty,
        "图像撤下后须整帧重绘以清掉残留像素"
    );

    let still_plain = kitty_cache_invalidation(plain, plain, false, false);
    assert!(!still_plain.overlay_dirty, "纯文本会话状态未变时不重绘");
}
