//! Criterion trend benchmarks: VT typing latency.
//!
//! Complements the in-test `bench_*` assertion benchmarks (which guard
//! CI regressions with hard thresholds). Criterion provides statistical
//! confidence intervals for development-time analysis:
//!
//! ```text
//! cargo bench -p native --bench vt_typing
//! ```

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
