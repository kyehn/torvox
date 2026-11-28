//! 实例构建路径的堆分配预算（含组合簇）。
//!
//! 单元实例构建对每帧要重建的行逐单元整形组合簇。此前每次整形都会新建三样东西：
//! 拼簇文本的 `String`、整形缓存键里的 `text.to_string()`、命中时 `Vec` 的克隆。
//! 24×80 网格中八分之一是组合簇（240 个）时实测 730 次分配/帧；整形缓存键改为
//! 内联文本、缓存值改为共享切片、簇文本写入复用缓冲后降到 8 次/帧。
//!
//! 本用例钉住该量级：预算按实测值留出一个数量级余量，逐单元分配一旦回归
//! （每簇 3 次 × 240 簇 = 720 次/帧）即判红。计数用本测试目标自己的全局分配器
//! （独立测试目标，计数器不与别的用例互相干扰）；统计窗口只覆盖构建循环，
//! 字体管线与字形图集的初始化在窗口之外。

use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

struct CountingAllocator;

static ALLOCATIONS: AtomicUsize = AtomicUsize::new(0);
static BYTES: AtomicUsize = AtomicUsize::new(0);
static COUNTING: AtomicBool = AtomicBool::new(false);

unsafe impl GlobalAlloc for CountingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        if COUNTING.load(Ordering::Relaxed) {
            ALLOCATIONS.fetch_add(1, Ordering::Relaxed);
            BYTES.fetch_add(layout.size(), Ordering::Relaxed);
        }
        unsafe { System.alloc(layout) }
    }
    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        unsafe { System.dealloc(pointer, layout) }
    }
    unsafe fn realloc(&self, pointer: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        if COUNTING.load(Ordering::Relaxed) {
            ALLOCATIONS.fetch_add(1, Ordering::Relaxed);
            BYTES.fetch_add(new_size, Ordering::Relaxed);
        }
        unsafe { System.realloc(pointer, layout, new_size) }
    }
}

#[global_allocator]
static ALLOCATOR: CountingAllocator = CountingAllocator;

use native::render::cell_builder::CellInstanceConfig;
use native::render::font::FontPipeline;
use native::terminal::ghostty_terminal::CellData;

/// 每帧允许的分配次数（实测 8 次/帧）。
///
/// 逐单元分配回归时该值跳到数百次/帧，故余量取一个数量级即可。
const ALLOCATIONS_PER_FRAME_BUDGET: usize = 100;

/// 每帧允许的分配字节数（实测约 1KB/帧）。
///
/// 回归时每簇的 `String`/`Vec` 合计约 64 字节 × 240 簇 ≈ 15KB/帧，
/// 故 64KB 的预算仍能判红，同时容得下图集/缓存的一次性扩容余量。
const BYTES_PER_FRAME_BUDGET: usize = 64 * 1024;

/// 参与统计的帧数：足够摊掉单帧的偶发差异。
const MEASURED_FRAMES: usize = 20;

/// 网格尺寸：与 24×80 终端一屏一致。
const GRID_ROWS: u32 = 24;
const GRID_COLS: u32 = 80;

/// 每 8 个单元出现一次的混合内容：ASCII、CJK、宽字符、下划线、反显与块字符。
/// 下标 5 的 `'e'` 在簇模式下带组合尖音符，使八分之一的单元走簇整形。
const MIXED: [(char, u32, [f32; 4], [f32; 4], u32); 8] = [
    ('A', 1, [0.9, 0.9, 0.9, 1.0], [0.1, 0.1, 0.1, 1.0], 0),
    (' ', 1, [0.9, 0.9, 0.9, 1.0], [0.1, 0.1, 0.1, 1.0], 0),
    ('中', 2, [1.0, 0.8, 0.2, 1.0], [0.05, 0.05, 0.1, 1.0], 0),
    ('W', 1, [0.3, 0.8, 1.0, 1.0], [0.1, 0.1, 0.1, 1.0], 1),
    ('i', 1, [0.5, 1.0, 0.5, 1.0], [0.1, 0.1, 0.1, 1.0], 2),
    ('e', 1, [0.9, 0.9, 0.9, 1.0], [0.2, 0.0, 0.0, 1.0], 4),
    ('█', 1, [0.6, 0.6, 0.6, 1.0], [0.15, 0.15, 0.15, 1.0], 0),
    ('~', 1, [0.4, 0.4, 0.4, 1.0], [0.1, 0.1, 0.1, 1.0], 8),
];

/// 构造整屏单元：所有 `'e'` 单元都带组合尖音符。
fn cluster_cells() -> Vec<CellData> {
    (0..(GRID_ROWS * GRID_COLS) as usize)
        .map(|index| {
            let (character, width, foreground, background, flags) = MIXED[index % MIXED.len()];
            let mut grapheme_extra = [0u32; 7];
            if character == 'e' {
                grapheme_extra[0] = 0x0301;
            }
            CellData {
                codepoint: character as u32,
                width,
                grapheme_extra,
                foreground,
                background,
                underline_color: foreground,
                flags,
                row: (index / GRID_COLS as usize) as u32,
                col: (index % GRID_COLS as usize) as u32,
            }
        })
        .collect()
}

fn measure(mut frame: impl FnMut()) -> (usize, usize) {
    ALLOCATIONS.store(0, Ordering::Relaxed);
    BYTES.store(0, Ordering::Relaxed);
    COUNTING.store(true, Ordering::Relaxed);
    frame();
    COUNTING.store(false, Ordering::Relaxed);
    (
        ALLOCATIONS.load(Ordering::Relaxed),
        BYTES.load(Ordering::Relaxed),
    )
}

#[test]
fn instance_build_with_clusters_stays_within_the_allocation_budget() {
    let mut font_pipeline = FontPipeline::new(2048, 2048, 14.0);
    let raster_scale = font_pipeline.get_raster_scale();
    let (metrics_width, metrics_height) = font_pipeline.cell_metrics();
    let config = CellInstanceConfig {
        rows: GRID_ROWS,
        cols: GRID_COLS,
        grid_cell_width: metrics_width * raster_scale,
        grid_cell_height: metrics_height * raster_scale,
        cursor: native::render::CellCursor {
            row: 12,
            col: 40,
            visible: true,
            style: native::terminal::CursorStyle::Block,
            color: [1.0; 4],
        },
        atlas_width: 2048.0,
        atlas_height: 2048.0,
        search_highlights: &[],
        default_background: [0.0; 4],
    };
    let cell_data = cluster_cells();
    // 生产路径：实例缓存跨帧复用，帧内不重建缓存。
    let mut cache = native::render::cell_builder::CachedInstances::new(GRID_ROWS, GRID_COLS);
    // 预热：字体管线、字形图集与整形缓存的一次性分配都在窗口之外。
    for _ in 0..5 {
        let _ = cache.rebuild(&cell_data, config, &mut font_pipeline, None);
    }

    let (allocations, bytes) = measure(|| {
        for _ in 0..MEASURED_FRAMES {
            let _ = cache.rebuild(&cell_data, config, &mut font_pipeline, None);
        }
    });
    let allocation_budget = ALLOCATIONS_PER_FRAME_BUDGET * MEASURED_FRAMES;
    let byte_budget = BYTES_PER_FRAME_BUDGET * MEASURED_FRAMES;
    assert!(
        allocations <= allocation_budget,
        "实例构建的堆分配超预算：{MEASURED_FRAMES} 帧共 {allocations} 次 \
         （预算 {allocation_budget} 次，即每帧 {ALLOCATIONS_PER_FRAME_BUDGET} 次）"
    );
    assert!(
        bytes <= byte_budget,
        "实例构建的分配字节超预算：{MEASURED_FRAMES} 帧共 {bytes} 字节 \
         （预算 {byte_budget} 字节，即每帧 {BYTES_PER_FRAME_BUDGET} 字节）"
    );
}
