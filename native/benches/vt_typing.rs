//! Criterion trend benchmarks: VT typing latency.
//!
//! Complements the in-test `bench_*` assertion benchmarks (which guard
//! CI regressions with hard thresholds). Criterion provides statistical
//! confidence intervals for development-time analysis:
//!
//! ```text
//! cargo bench --manifest-path native/Cargo.toml -p native --bench vt_typing
//! ```

mod support;

use criterion::{Criterion, criterion_group, criterion_main};
use native::terminal::ghostty_terminal::GhosttyTerminal;
use std::hint::black_box;

fn typing_latency(c: &mut Criterion) {
    let mut terminal = GhosttyTerminal::new(24, 80, 5000).expect("terminal");
    // Pre-fill with realistic content so the grid is not empty.
    for _ in 0..10 {
        terminal.vt_write(b"A line to fill the screen with some realistic content\n");
    }
    terminal.flush();

    let keystrokes: [&[u8]; 6] = [b"h", b"e", b"l", b"l", b"o", b"\n"];
    // 下限断言：只兜住数量级崩塌（VT 线程被同步等待、整帧克隆、每键重排网格）。
    // 上限按 CI 实测校准（`--quick` 输出 [floor] 行），不按本地体感；2s 对毫秒级
    // 键入路径留足余量，仍能挡住「每键整帧克隆」级别的退化。
    // CI 实测 0.467ms（[floor] 输出），1.5 倍余量；超出说明按键编码路径明显劣化。
    support::Floor::new("typing_keystroke", 0.7).assert_within(32, || {
        for key in &keystrokes {
            terminal.vt_write(key);
        }
        terminal.flush();
        let _ = black_box(terminal.receive_cell_data().map(|(cells, _)| cells.len()));
    });
    c.bench_function("typing_keystroke", |b| {
        b.iter(|| {
            for key in &keystrokes {
                terminal.vt_write(key);
            }
            terminal.flush();
            let count = black_box(terminal.receive_cell_data().map(|(cells, _)| cells.len()));
            black_box(count);
        })
    });
}

criterion_group!(benches, typing_latency);
criterion_main!(benches);
