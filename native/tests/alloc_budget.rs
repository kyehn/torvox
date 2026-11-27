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

use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

struct CountingAllocator;

static ALLOCATIONS: AtomicUsize = AtomicUsize::new(0);
static COUNTING: AtomicBool = AtomicBool::new(false);

unsafe impl GlobalAlloc for CountingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        if COUNTING.load(Ordering::Relaxed) {
            ALLOCATIONS.fetch_add(1, Ordering::Relaxed);
        }
        unsafe { System.alloc(layout) }
    }
    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        unsafe { System.dealloc(pointer, layout) }
    }
    unsafe fn realloc(&self, pointer: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        if COUNTING.load(Ordering::Relaxed) {
            ALLOCATIONS.fetch_add(1, Ordering::Relaxed);
        }
        unsafe { System.realloc(pointer, layout, new_size) }
    }
}

#[global_allocator]
static ALLOCATOR: CountingAllocator = CountingAllocator;

/// 整屏改写帧允许的分配次数（实测 26 次/帧）。
///
/// 逐单元分配回归时该值回到千次量级（实测 1029 次/帧），故余量取数倍即可。
const ALLOCATIONS_PER_FRAME_BUDGET: usize = 100;

/// 参与统计的帧数：足够摊掉单帧的偶发差异，又不至于让窗口长到跨过空闲节拍。
const MEASURED_FRAMES: usize = 20;

#[test]
fn full_screen_extraction_stays_within_the_allocation_budget() {
    use native::terminal::ghostty_terminal::GhosttyTerminal;

    let mut terminal = GhosttyTerminal::new(24, 80, 5000).expect("terminal");
    // 整屏内容：每帧改写全部 24 行，使抽取走全量路径（1920 个单元）。
    let mut content = Vec::new();
    for row in 0..24 {
        content.extend_from_slice(
            format!("\x1b[{row};1Hrow {row} some realistic terminal content here").as_bytes(),
        );
    }
    content.extend_from_slice(b"\n");

    let run_frame = |terminal: &mut GhosttyTerminal| {
        terminal.vt_write(&content);
        terminal.flush();
        let _ = terminal.receive_cell_data();
    };

    // 预热：终端初始化、渲染提取器与一次性缓冲都在窗口之外。
    for _ in 0..5 {
        run_frame(&mut terminal);
    }

    COUNTING.store(true, Ordering::Relaxed);
    for _ in 0..MEASURED_FRAMES {
        run_frame(&mut terminal);
    }
    COUNTING.store(false, Ordering::Relaxed);

    let allocations = ALLOCATIONS.load(Ordering::Relaxed);
    let budget = ALLOCATIONS_PER_FRAME_BUDGET * MEASURED_FRAMES;
    assert!(
        allocations <= budget,
        "整屏抽取的堆分配超预算：{MEASURED_FRAMES} 帧共 {allocations} 次 \
         （预算 {budget} 次，即每帧 {ALLOCATIONS_PER_FRAME_BUDGET} 次）"
    );
}
