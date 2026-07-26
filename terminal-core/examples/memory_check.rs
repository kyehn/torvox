//! Memory budget check for core types (P3.3.5 <10MB idle).
//!
//! Run with: cargo run -p terminal-core --example memory_check --release

use terminal_core::cell::{Attrs, Cell, Color};
use terminal_core::grid::Grid;

fn main() {
    use std::mem::size_of;

    println!("=== Core Memory Budget (P3.3.5: <10MB idle) ===\n");

    println!("Type sizes (compile-time constants):");
    println!("  Cell:    {} bytes", size_of::<Cell>());
    println!("  Attrs:   {} bytes", size_of::<Attrs>());
    println!("  Color:   {} bytes", size_of::<Color>());
    println!("  Grid:    {} bytes (struct header)", size_of::<Grid>());

    println!("\nHeap usage scenarios:");

    // 24x80 grid with no scrollback (idle)
    let g = Grid::new(24, 80);
    let cells_heap = 24 * 80 * size_of::<Cell>();
    println!(
        "  24x80 grid (idle):              ~{} bytes ({:.1} KB)",
        cells_heap,
        cells_heap as f64 / 1024.0
    );
    let _ = g;

    // 24x80 with 50k scrollback (max)
    let _g = Grid::with_scrollback(24, 80, 50_000);
    let scrollback_heap = 50_000 * 80 * size_of::<Cell>();
    let total_max = cells_heap + scrollback_heap;
    println!(
        "  24x80 + 50k scrollback (max):   ~{} bytes ({:.1} MB)",
        total_max,
        total_max as f64 / 1_048_576.0
    );
    println!(
        "    (idle: ~{:.1} KB, scrollback: ~{:.1} MB)",
        cells_heap as f64 / 1024.0,
        scrollback_heap as f64 / 1_048_576.0
    );

    println!("\nBudget verdict:");
    if cells_heap + 1_000_000 < 10 * 1_048_576 {
        println!(
            "  IDLE: {:.1} KB + overhead < 10MB budget — PASS",
            cells_heap as f64 / 1024.0
        );
    } else {
        println!("  IDLE: EXCEEDS 10MB budget");
    }
    if total_max < 100 * 1_048_576 {
        println!(
            "  MAX:  {:.1} MB < 100MB max — PASS",
            total_max as f64 / 1_048_576.0
        );
    } else {
        println!("  MAX:  EXCEEDS 100MB max");
    }
}
