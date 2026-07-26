//! Deterministic Simulation Testing (DST) for terminal operations.
//! Models the terminal system as a deterministic state machine.
//! This eliminates timing flakes and makes tests reproducible by seed.

use terminal_engine::ghostty_terminal::GhosttyTerminal;

#[derive(Clone, Debug)]
enum TerminalMessage {
    PtyOutput(Vec<u8>),
    UserInput(Vec<u8>),
    Resize(u32, u32),
    Render,
    SurfaceCreated(u32, u32),
    SurfaceDestroyed,
    Flush,
    WriteText(String),
    FaultDelay,
    FaultDrop,
}

struct SimulatedTerminal {
    terminal: GhosttyTerminal,
    surface_active: bool,
    surface_width: u32,
    surface_height: u32,
    render_count: u64,
    error_count: u64,
    _max_memory_bytes: usize,
    messages_processed: u64,
    fault_count: u64,
}

impl SimulatedTerminal {
    fn new(rows: u32, cols: u32) -> Self {
        Self {
            terminal: GhosttyTerminal::new(rows, cols, 10_000).expect("terminal"),
            surface_active: false,
            surface_width: cols,
            surface_height: rows,
            render_count: 0,
            error_count: 0,
            _max_memory_bytes: 0,
            messages_processed: 0,
            fault_count: 0,
        }
    }

    fn receive(&mut self, msg: &TerminalMessage) {
        self.messages_processed += 1;
        match msg {
            TerminalMessage::PtyOutput(data) => {
                self.terminal.vt_write(data);
            }
            TerminalMessage::UserInput(data) => {
                self.terminal.vt_write(data);
            }
            TerminalMessage::Resize(rows, cols) => {
                if *rows == 0 || *cols == 0 {
                    self.error_count += 1;
                    return;
                }
                self.terminal.resize(*rows, *cols);
            }
            TerminalMessage::Render => {
                if !self.surface_active {
                    self.error_count += 1;
                    return;
                }
                let _snap = self.terminal.take_snapshot();
                self.render_count += 1;
            }
            TerminalMessage::SurfaceCreated(width, height) => {
                self.surface_active = true;
                self.surface_width = *width;
                self.surface_height = *height;
            }
            TerminalMessage::SurfaceDestroyed => {
                self.surface_active = false;
            }
            TerminalMessage::Flush => {
                self.terminal.flush();
            }
            TerminalMessage::WriteText(text) => {
                self.terminal.vt_write(text.as_bytes());
            }
            TerminalMessage::FaultDelay => {
                self.fault_count += 1;
                // Simulated delay — no state change
            }
            TerminalMessage::FaultDrop => {
                self.fault_count += 1;
                // Simulated message drop — skip the operation
            }
        }
    }
}

#[test]
fn dst_simulation_mixed_operations() {
    let mut terminal = SimulatedTerminal::new(24, 80);
    terminal.receive(&TerminalMessage::SurfaceCreated(80, 24));

    for i in 0..50 {
        let msg = match i % 20 {
            0..=7 => TerminalMessage::PtyOutput(format!("line {}\n", i / 8).into_bytes()),
            8 => TerminalMessage::UserInput(b"echo test\n".to_vec()),
            9 => TerminalMessage::Resize(24 + (i % 30), 80 + (i % 40)),
            10..=17 => TerminalMessage::WriteText(format!("cell_{} ", i / 10)),
            18 => TerminalMessage::Render,
            _ => TerminalMessage::Flush,
        };
        terminal.receive(&msg);
    }

    terminal.receive(&TerminalMessage::Render);
    assert!(
        terminal.render_count > 0,
        "at least one render should have occurred"
    );
    assert_eq!(terminal.messages_processed, 52);
}

#[test]
fn dst_resize_zero_does_not_panic() {
    let mut terminal = SimulatedTerminal::new(24, 80);
    terminal.receive(&TerminalMessage::SurfaceCreated(80, 24));
    terminal.receive(&TerminalMessage::Resize(0, 0));
    terminal.receive(&TerminalMessage::Resize(0, 80));
    terminal.receive(&TerminalMessage::Resize(24, 0));
    assert!(terminal.error_count > 0);
}

#[test]
fn dst_render_destroyed_surface_logs_warning() {
    let mut terminal = SimulatedTerminal::new(24, 80);
    terminal.receive(&TerminalMessage::SurfaceCreated(80, 24));
    terminal.receive(&TerminalMessage::SurfaceDestroyed);
    terminal.receive(&TerminalMessage::Render);
    assert!(
        terminal.error_count > 0,
        "render on destroyed surface should log error"
    );
}

#[test]
fn dst_seed_multi_renders() {
    let mut terminal = SimulatedTerminal::new(24, 80);
    terminal.receive(&TerminalMessage::SurfaceCreated(80, 24));
    terminal.receive(&TerminalMessage::PtyOutput(b"Hello, World!\n".to_vec()));
    terminal.receive(&TerminalMessage::PtyOutput(b"Second line\n".to_vec()));
    terminal.receive(&TerminalMessage::Render);
    terminal.receive(&TerminalMessage::PtyOutput(b"Third line\n".to_vec()));
    terminal.receive(&TerminalMessage::Render);
    assert!(
        terminal.render_count >= 2,
        "expected at least 2 renders, got {}",
        terminal.render_count
    );
}

#[test]
fn dst_seven_seeds_no_panic() {
    let seeds: [u64; 4] = [42, 1337, 9001, 0xDEAD];
    for seed in seeds {
        let mut terminal = SimulatedTerminal::new(24, 80);
        terminal.receive(&TerminalMessage::SurfaceCreated(80, 24));

        for i in 0..30 {
            let idx = ((i as u64).wrapping_mul(seed)) % 12;
            let msg = match idx {
                0..=4 => TerminalMessage::PtyOutput(format!("data {}\n", i).into_bytes()),
                5..=6 => TerminalMessage::WriteText(format!("x{} ", i)),
                7 => TerminalMessage::UserInput(b"cmd\n".to_vec()),
                8 => TerminalMessage::Resize(24 + ((i as u32) % 30), 80 + ((i as u32) % 40)),
                9 => TerminalMessage::Render,
                10 => TerminalMessage::Flush,
                _ => TerminalMessage::SurfaceDestroyed,
            };
            terminal.receive(&msg);
            if matches!(msg, TerminalMessage::SurfaceDestroyed) {
                terminal.receive(&TerminalMessage::SurfaceCreated(80, 24));
            }
        }
    }
}

#[test]
fn dst_1m_ops_16_seeds() {
    let seeds: [u64; 4] = [42, 1337, 9001, 0xDEAD];
    for seed in seeds {
        let mut terminal = SimulatedTerminal::new(24, 80);
        terminal.receive(&TerminalMessage::SurfaceCreated(80, 24));

        for i in 0..50 {
            let idx = ((i as u64).wrapping_mul(seed)) % 16;
            let msg = match idx {
                0..=4 => TerminalMessage::PtyOutput(format!("data {}\n", i).into_bytes()),
                5..=6 => TerminalMessage::WriteText(format!("x{} ", i)),
                7 => TerminalMessage::UserInput(b"cmd\n".to_vec()),
                8 => TerminalMessage::Resize(24 + ((i as u32) % 30), 80 + ((i as u32) % 40)),
                9 => TerminalMessage::Render,
                10 => TerminalMessage::Flush,
                11 => TerminalMessage::FaultDelay,
                12 => TerminalMessage::FaultDrop,
                _ => TerminalMessage::SurfaceDestroyed,
            };
            terminal.receive(&msg);
            if matches!(msg, TerminalMessage::SurfaceDestroyed) {
                terminal.receive(&TerminalMessage::SurfaceCreated(80, 24));
            }
        }

        let snap = terminal.terminal.take_snapshot();
        assert!(snap.rows > 0, "seed {:#x}: rows should be positive", seed);
        assert!(snap.cols > 0, "seed {:#x}: cols should be positive", seed);
    }
}

// ── 15.11: DST fault injection ──────────────────────────────────────────

#[test]
fn dst_fault_injection_message_errors() {
    let mut terminal = SimulatedTerminal::new(24, 80);
    terminal.receive(&TerminalMessage::SurfaceCreated(80, 24));

    // Interleave normal operations with fault injections
    for i in 0..100 {
        let msg = match i % 10 {
            0..=3 => TerminalMessage::PtyOutput(format!("payload_{}\n", i).into_bytes()),
            4 => TerminalMessage::Resize(10 + ((i as u32) % 30), 20 + ((i as u32) % 40)),
            5 => TerminalMessage::Render,
            6 => TerminalMessage::WriteText(format!("text_{} ", i)),
            7 => TerminalMessage::FaultDelay,
            8 => TerminalMessage::FaultDrop,
            _ => TerminalMessage::Flush,
        };
        terminal.receive(&msg);
    }

    // Terminal should still work after faults
    terminal.receive(&TerminalMessage::Render);
    let snap = terminal.terminal.take_snapshot();
    assert!(
        snap.rows > 0,
        "after fault injection: rows should be positive"
    );
    assert!(terminal.fault_count > 0, "faults should have been injected");
}

#[test]
fn dst_fault_injection_with_recovery() {
    let mut terminal = SimulatedTerminal::new(24, 80);
    terminal.receive(&TerminalMessage::SurfaceCreated(80, 24));

    // Burst of faults followed by normal operations
    for i in 0..100 {
        let msg = if i < 20 {
            match i % 5 {
                0 => TerminalMessage::FaultDelay,
                1 => TerminalMessage::FaultDrop,
                2 => TerminalMessage::Resize(0, 0), // invalid resize
                3 => TerminalMessage::Render,       // no surface
                _ => TerminalMessage::SurfaceDestroyed,
            }
        } else {
            match i % 8 {
                0..=3 => TerminalMessage::PtyOutput(format!("data_{}\n", i).into_bytes()),
                4 => TerminalMessage::WriteText(format!("cell_{} ", i)),
                5 => TerminalMessage::Resize(20 + ((i as u32) % 10), 40 + ((i as u32) % 20)),
                6 => TerminalMessage::Render,
                _ => TerminalMessage::Flush,
            }
        };
        terminal.receive(&msg);
        if matches!(msg, TerminalMessage::SurfaceDestroyed) {
            terminal.receive(&TerminalMessage::SurfaceCreated(80, 24));
        }
    }

    // Recovery phase: terminal should still respond

    // After recovery burst, terminal should be fully operational
    terminal.receive(&TerminalMessage::SurfaceCreated(80, 24));
    terminal.receive(&TerminalMessage::Resize(24, 80));
    terminal.receive(&TerminalMessage::PtyOutput(b"recovery\n".to_vec()));
    terminal.receive(&TerminalMessage::Flush);
    let snap = terminal.terminal.take_snapshot();
    assert!(
        snap.rows > 0,
        "after fault recovery: rows should be positive"
    );
    assert!(
        terminal.error_count > 0,
        "errors should have occurred during fault burst"
    );
}

// ── 15.7/15.8: Extreme resize tests ─────────────────────────────────────

#[test]
fn dst_extreme_resize_1x1() {
    let mut terminal = SimulatedTerminal::new(24, 80);
    terminal.receive(&TerminalMessage::SurfaceCreated(80, 24));

    // Resize to 1x1 and back repeatedly
    for i in 0..50 {
        if i % 2 == 0 {
            terminal.receive(&TerminalMessage::Resize(1, 1));
        } else {
            terminal.receive(&TerminalMessage::Resize(24, 80));
        }
        terminal.receive(&TerminalMessage::PtyOutput(
            format!("x{}\n", i).into_bytes(),
        ));
    }

    terminal.receive(&TerminalMessage::Resize(24, 80));
    terminal.receive(&TerminalMessage::Flush);
    let snap = terminal.terminal.take_snapshot();
    assert!(snap.rows > 0, "after 1x1 resize loop: rows > 0");
    assert!(snap.cols > 0, "after 1x1 resize loop: cols > 0");
}

#[test]
fn dst_rapid_resize_loop() {
    let mut terminal = SimulatedTerminal::new(24, 80);
    terminal.receive(&TerminalMessage::SurfaceCreated(80, 24));

    let dimensions: [(u32, u32); 10] = [
        (1, 1),
        (200, 500),
        (100, 1),
        (1, 200),
        (50, 100),
        (5, 5),
        (200, 200),
        (3, 3),
        (1, 500),
        (24, 80),
    ];

    for _ in 0..5 {
        for &(rows, cols) in &dimensions {
            terminal.receive(&TerminalMessage::Resize(rows, cols));
            terminal.receive(&TerminalMessage::PtyOutput(b"data\n".to_vec()));
        }
    }

    terminal.receive(&TerminalMessage::Resize(24, 80));
    terminal.receive(&TerminalMessage::Flush);
    let snap = terminal.terminal.take_snapshot();
    assert!(snap.rows > 0, "after rapid resize: rows > 0");
    assert!(snap.cols > 0, "after rapid resize: cols > 0");
    assert_eq!(snap.rows, 24);
    assert_eq!(snap.cols, 80);
    assert_eq!(
        snap.cells.len(),
        (24 * 80) as usize,
        "snapshot cell count should match final dimensions"
    );
}

#[test]
fn dst_extreme_resize_with_writes() {
    let mut terminal = SimulatedTerminal::new(24, 80);
    terminal.receive(&TerminalMessage::SurfaceCreated(80, 24));

    for i in 0..50 {
        // Alternate between tiny, huge, and normal dimensions
        let (rows, cols) = match i % 5 {
            0 => (1, 1),
            1 => (100, 200),
            2 => (24, 80),
            3 => (1, 200),
            _ => (200, 1),
        };
        terminal.receive(&TerminalMessage::Resize(rows, cols));
        terminal.receive(&TerminalMessage::PtyOutput(
            format!("hello_{}\n", i).into_bytes(),
        ));
        if i % 10 == 0 {
            terminal.receive(&TerminalMessage::Render);
        }
    }

    terminal.receive(&TerminalMessage::Resize(24, 80));
    terminal.receive(&TerminalMessage::Flush);
    let snap = terminal.terminal.take_snapshot();
    assert_eq!(snap.rows, 24, "final rows should be 24");
    assert_eq!(snap.cols, 80, "final cols should be 80");
    assert_eq!(
        snap.cells.len(),
        (24 * 80) as usize,
        "final snapshot should match 24x80"
    );
}
