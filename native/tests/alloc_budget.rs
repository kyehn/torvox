//! 抽取路径的堆分配预算。
//!
//! 单元帧抽取按单元处理 24×80 网格，逐单元一次堆分配即每帧近两千次分配：
//! 分配次数本身不改变输出，却让分配器与缓存成为帧路径上的固定开销。组合记号
//! 曾走上游 `graphemes()`（逐单元新建 `Vec<char>`），改为写调用方缓冲后整屏
//! 改写帧的分配由 1029 次/帧降到 26 次/帧。
//!
//! 本用例钉住该量级：预算按实测值的数倍留出余量，逐单元分配一旦回归即判红。
//! 计数用本测试目标自己的全局分配器（本文件是独立测试目标，计数器不与其他
//! 用例互相干扰）；统计窗口只覆盖帧循环，终端与分配器的初始化在窗口之外。
//!
//! 帧路径分两段，两个窗口各测一段：内容不变的窗口只走抽取——本帧内容与上一帧
//! 逐字节相同，去重短路，帧不外发；内容逐帧变化的窗口额外走推送——帧邮箱克隆
//! 在这一段。只测前者会漏掉推送段的分配回归，故两者都测。
//! 实测基线：内容不变窗口 26 次/帧、384KB/帧（2.1 份整帧缓冲）；内容变化窗口
//! 30 次/帧、569KB/帧（3.1 份整帧缓冲，多出的一份即帧邮箱克隆）。
//! 字节预算按「整帧缓冲份数」表达，不写死字节数：网格快照的尺寸随
//! `CellData` 变化，份数才是与实现无关的口径。

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

/// 整屏改写帧允许的分配次数（实测 26 次/帧，内容逐帧变化时 30 次/帧）。
///
/// 逐单元分配回归时该值回到千次量级（实测 1029 次/帧），故余量取数倍即可。
const ALLOCATIONS_PER_FRAME_BUDGET: usize = 100;

/// 参与统计的帧数：足够摊掉单帧的偶发差异，又不至于让窗口长到跨过空闲节拍。
const MEASURED_FRAMES: usize = 20;

/// 网格尺寸：与分配预算的「整帧份数」口径配套。
const GRID_ROWS: u16 = 24;
const GRID_COLS: u16 = 80;

/// 一份整帧缓冲的字节数（网格单元数 × 单单元载荷）。
fn full_frame_bytes() -> usize {
    (GRID_ROWS as usize)
        * (GRID_COLS as usize)
        * std::mem::size_of::<native::terminal::ghostty_terminal::CellData>()
}

/// 改写帧允许分配的整帧缓冲份数。
///
/// 抽取缓冲、行缓存拷贝与帧邮箱克隆各一份，即三份（实测 3.1 份）；第四份是
/// 余量。多出一整份即出现新的整帧拷贝（逐单元分配回归时是几十份）。
const FULL_FRAME_COPIES_BUDGET: usize = 4;

#[test]
fn full_screen_extraction_stays_within_the_allocation_budget() {
    use native::terminal::ghostty_terminal::GhosttyTerminal;

    let mut terminal = GhosttyTerminal::new(GRID_ROWS, GRID_COLS, 5000).expect("terminal");
    // 整屏内容：每帧改写全部 24 行，使抽取走全量路径（1920 个单元）。
    let content = full_screen_content(0);
    // 逐帧变化的内容：内容不同则本帧与上一帧不同，去重不短路，帧真的外发，
    // 推送段（行缓存拷贝 + 帧邮箱克隆）落在统计窗口内。
    let mut changing_content = Vec::with_capacity(MEASURED_FRAMES);
    for tick in 0..MEASURED_FRAMES {
        changing_content.push(full_screen_content(tick + 1));
    }

    let run_frame = |terminal: &mut GhosttyTerminal, payload: &[u8]| {
        terminal.vt_write(payload);
        terminal.flush();
        let _ = terminal.receive_cell_data();
    };

    // 预热：终端初始化、渲染提取器与一次性缓冲都在窗口之外。
    for _ in 0..5 {
        run_frame(&mut terminal, &content);
    }

    let allocation_budget = ALLOCATIONS_PER_FRAME_BUDGET * MEASURED_FRAMES;
    let byte_budget = full_frame_bytes() * FULL_FRAME_COPIES_BUDGET * MEASURED_FRAMES;

    // 窗口一：内容不变——只走抽取段。
    let (unchanged_allocations, unchanged_bytes) = measure(|| {
        for _ in 0..MEASURED_FRAMES {
            run_frame(&mut terminal, &content);
        }
    });
    assert!(
        unchanged_allocations <= allocation_budget,
        "整屏抽取的堆分配超预算：{MEASURED_FRAMES} 帧共 {unchanged_allocations} 次 \
         （预算 {allocation_budget} 次，即每帧 {ALLOCATIONS_PER_FRAME_BUDGET} 次）"
    );
    assert!(
        unchanged_bytes <= byte_budget,
        "整屏抽取的分配字节超预算：{MEASURED_FRAMES} 帧共 {unchanged_bytes} 字节 \
         （预算 {byte_budget} 字节，即每帧 {FULL_FRAME_COPIES_BUDGET} 份整帧缓冲）"
    );

    // 窗口二：内容逐帧变化——抽取段与推送段都在窗口内。
    let (changing_allocations, changing_bytes) = measure(|| {
        for payload in &changing_content {
            run_frame(&mut terminal, payload);
        }
    });
    assert!(
        changing_allocations <= allocation_budget,
        "整屏改写并推送帧的堆分配超预算：{MEASURED_FRAMES} 帧共 {changing_allocations} 次 \
         （预算 {allocation_budget} 次，即每帧 {ALLOCATIONS_PER_FRAME_BUDGET} 次）"
    );
    assert!(
        changing_bytes <= byte_budget,
        "整屏改写并推送帧的分配字节超预算：{MEASURED_FRAMES} 帧共 {changing_bytes} 字节 \
         （预算 {byte_budget} 字节，即每帧 {FULL_FRAME_COPIES_BUDGET} 份整帧缓冲）"
    );
}

/// 跑一段帧循环并返回窗口内的分配次数与字节数。
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

/// 构造一份改写整屏的内容：`tick` 进入正文，使相邻两帧的内容不同。
fn full_screen_content(tick: usize) -> Vec<u8> {
    let mut content = Vec::new();
    for row in 0..GRID_ROWS as u32 {
        content.extend_from_slice(
            format!("\x1b[{row};1Hrow {row} tick {tick} some realistic terminal content here")
                .as_bytes(),
        );
    }
    content.extend_from_slice(b"\n");
    content
}
