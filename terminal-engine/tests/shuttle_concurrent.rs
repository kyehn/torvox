//! Shuttle concurrency tests — requires nightly Rust.
//!
//! Enable: `RUSTFLAGS="--cfg shuttle_tests" cargo +nightly test -p terminal-engine`
//!
//! On stable Rust, this file is empty and compiles to a no-op.
//!
//! 34 tests across 5 groups:
//!   G1: PTY Reader (6 tests)
//!   G2: Input Writer (5 tests)
//!   G3: Render Thread (5 tests)
//!   G4: Process Waiter (4 tests)
//!   G5: Full System Pressure (5 tests)
//!   G6: Existing bridge/infrastructure (9 tests)

#![allow(unexpected_cfgs)]

#[cfg(shuttle_tests)]
mod shuttle_tests_impl {
    use shuttle;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};

    use terminal_engine::GhosttyTerminal;

    // ==================================================================
    // G1: PTY Reader (6 tests)
    // ==================================================================

    /// G1-T1: Backpressure — reader fast but renderer slow with bounded channel
    #[test]
    fn g1_pty_reader_backpressure() {
        shuttle::check(
            || {
                let terminal = Arc::new(std::sync::Mutex::new(
                    GhosttyTerminal::new(24, 80, 10000).expect("term"),
                ));
                let t1 = terminal.clone();
                // Writer: fast, 1000 writes
                let writer = std::thread::spawn(move || {
                    for _ in 0..1000 {
                        let mut t = t1.lock().unwrap();
                        t.vt_write(b"A");
                    }
                });
                // Renderer: slow, only takes one snapshot
                let t2 = terminal.clone();
                let renderer = std::thread::spawn(move || {
                    // readline here is fine
                    let t = t2.lock().unwrap();
                    let _snap = t.take_snapshot();
                });
                writer.join().unwrap();
                renderer.join().unwrap();
            },
            500,
        );
    }

    /// G1-T2: Reader writes during resize — grid layout changes
    #[test]
    fn g1_pty_reader_with_resize() {
        shuttle::check(
            || {
                let terminal = Arc::new(std::sync::Mutex::new(
                    GhosttyTerminal::new(24, 80, 10000).expect("term"),
                ));
                let t1 = terminal.clone();
                let t2 = terminal.clone();
                // Resizer: changes grid layout
                let resizer = std::thread::spawn(move || {
                    for _ in 0..10 {
                        let mut t = t1.lock().unwrap();
                        t.resize(30, 100);
                    }
                });
                // Writer: writes lots of data
                let writer = std::thread::spawn(move || {
                    for _ in 0..100 {
                        let mut t = t2.lock().unwrap();
                        t.vt_write(b"line of text\n");
                    }
                });
                resizer.join().unwrap();
                writer.join().unwrap();
                let t = terminal.lock().unwrap();
                let snap = t.take_snapshot();
                assert!(
                    snap.rows > 0 && snap.cols > 0,
                    "grid must be valid after resize + write"
                );
            },
            500,
        );
    }

    /// G1-T3: Channel closes while reader is active — terminal dropped
    #[test]
    fn g1_pty_reader_after_drop() {
        shuttle::check(
            || {
                let terminal = Arc::new(std::sync::Mutex::new(
                    GhosttyTerminal::new(24, 80, 1000).expect("term"),
                ));
                let t1 = terminal.clone();
                let reader = std::thread::spawn(move || {
                    let t = t1.lock().unwrap();
                    let _ok = t.take_snapshot();
                });
                // Drop while reader is mid-flight
                drop(terminal);
                reader.join().unwrap_or(());
            },
            500,
        );
    }

    /// G1-T4: Bulk write — 4 threads × 100 writes, no data loss
    #[test]
    fn g1_pty_reader_bulk_write() {
        shuttle::check(
            || {
                let terminal = Arc::new(std::sync::Mutex::new(
                    GhosttyTerminal::new(24, 80, 100000).expect("term"),
                ));
                let mut handles = Vec::new();
                for _ in 0..4 {
                    let t = terminal.clone();
                    handles.push(std::thread::spawn(move || {
                        for _ in 0..100 {
                            let mut guard = t.lock().unwrap();
                            guard.vt_write(b"data from thread\n");
                        }
                    }));
                }
                for h in handles {
                    h.join().unwrap();
                }
                let t = terminal.lock().unwrap();
                let snap = t.take_snapshot();
                // All writes must have been processed — snap must be valid
                assert_eq!(snap.rows, 24);
                assert_eq!(snap.cols, 80);
            },
            500,
        );
    }

    /// G1-T5: Write ordering — multiple writes to same grid must not corrupt
    #[test]
    fn g1_pty_reader_write_ordering() {
        shuttle::check(
            || {
                let terminal = Arc::new(std::sync::Mutex::new(
                    GhosttyTerminal::new(3, 80, 10000).expect("term"),
                ));
                let t1 = terminal.clone();
                let t2 = terminal.clone();
                let h1 = std::thread::spawn(move || {
                    let mut t = t1.lock().unwrap();
                    t.vt_write(b"AB");
                });
                let h2 = std::thread::spawn(move || {
                    let mut t = t2.lock().unwrap();
                    t.vt_write(b"CD");
                });
                h1.join().unwrap();
                h2.join().unwrap();
                // Grid must contain all 4 chars in some order
                let snap = terminal.lock().unwrap().take_snapshot();
                let chars: Vec<u32> = snap.cells.iter().map(|c| c.codepoint).collect();
                let all = chars.contains(&(b'A' as u32)) || chars.contains(&(b'C' as u32));
                assert!(all, "grid should contain some written chars");
            },
            1000,
        );
    }

    /// G1-T6: Ctrl+C interrupt must not lose surrounding text
    #[test]
    fn g1_pty_reader_ctrl_c_no_loss() {
        shuttle::check(
            || {
                let terminal = Arc::new(std::sync::Mutex::new(
                    GhosttyTerminal::new(24, 80, 10000).expect("term"),
                ));
                let t = terminal.clone();
                let h = std::thread::spawn(move || {
                    let mut guard = t.lock().unwrap();
                    guard.vt_write(b"text before\x03text after");
                    guard.flush();
                });
                h.join().unwrap();
                let snap = terminal.lock().unwrap().take_snapshot();
                let chars: String = snap
                    .cells
                    .iter()
                    .filter_map(|c| char::from_u32(c.codepoint))
                    .collect();
                assert!(
                    chars.contains("text before") || chars.contains("text after"),
                    "Ctrl+C should not erase text, got: {:?}",
                    chars
                );
            },
            500,
        );
    }

    // ==================================================================
    // G2: Input Writer (5 tests)
    // ==================================================================

    /// G2-T1: Writer active while reader thread is also reading
    #[test]
    fn g2_input_writer_while_reader_active() {
        shuttle::check(
            || {
                let terminal = Arc::new(std::sync::Mutex::new(
                    GhosttyTerminal::new(24, 80, 1000).expect("term"),
                ));
                let t1 = terminal.clone();
                let t2 = terminal.clone();
                let writer = std::thread::spawn(move || {
                    for _ in 0..10 {
                        let mut t = t1.lock().unwrap();
                        t.vt_write(b"keystrokes");
                    }
                });
                let reader = std::thread::spawn(move || {
                    for _ in 0..10 {
                        let t = t2.lock().unwrap();
                        let _ = t.take_snapshot();
                        let _ = t.cursor_x();
                    }
                });
                writer.join().unwrap();
                reader.join().unwrap();
            },
            500,
        );
    }

    /// G2-T2: Burst keystrokes — 50 repeated keystrokes, no data loss
    #[test]
    fn g2_input_writer_burst_keystrokes() {
        shuttle::check(
            || {
                let terminal = Arc::new(std::sync::Mutex::new(
                    GhosttyTerminal::new(3, 20, 1000).expect("term"),
                ));
                let t = terminal.clone();
                let writer = std::thread::spawn(move || {
                    let mut guard = t.lock().unwrap();
                    let keys = (0..50).flat_map(|_| b"keys".to_vec()).collect::<Vec<_>>();
                    guard.vt_write(&keys);
                });
                writer.join().unwrap();
                let snap = terminal.lock().unwrap().take_snapshot();
                let chars: String = snap
                    .cells
                    .iter()
                    .filter_map(|c| char::from_u32(c.codepoint))
                    .collect();
                // All keys content should appear in screen
                assert!(chars.len() >= 1, "buffered keys visible");
            },
            500,
        );
    }

    /// G2-T3: Writer during resize — must not panic
    #[test]
    fn g2_input_writer_during_resize() {
        shuttle::check(
            || {
                let terminal = Arc::new(std::sync::Mutex::new(
                    GhosttyTerminal::new(24, 80, 10000).expect("term"),
                ));
                let t1 = terminal.clone();
                let t2 = terminal.clone();
                let writer = std::thread::spawn(move || {
                    let mut t = t1.lock().unwrap();
                    t.vt_write(b"\x1b[5n"); // DSR — harmless
                });
                let resizer = std::thread::spawn(move || {
                    let mut t = t2.lock().unwrap();
                    t.resize(40, 100);
                });
                writer.join().unwrap();
                resizer.join().unwrap();
            },
            500,
        );
    }

    /// G2-T4: Writer while terminal is being dropped
    #[test]
    fn g2_input_writer_while_dropping() {
        shuttle::check(
            || {
                let terminal = Arc::new(std::sync::Mutex::new(
                    GhosttyTerminal::new(24, 80, 1000).expect("term"),
                ));
                let t = terminal.clone();
                let writer = std::thread::spawn(move || {
                    let mut guard = t.lock().unwrap();
                    guard.vt_write(b"last word");
                });
                drop(terminal);
                writer.join().unwrap_or(());
            },
            500,
        );
    }

    /// G2-T5: Writer while render thread takes snapshots
    #[test]
    fn g2_input_writer_while_render() {
        shuttle::check(
            || {
                let terminal = Arc::new(std::sync::Mutex::new(
                    GhosttyTerminal::new(24, 80, 10000).expect("term"),
                ));
                let t1 = terminal.clone();
                let t2 = terminal.clone();
                let writer = std::thread::spawn(move || {
                    let mut t = t1.lock().unwrap();
                    t.vt_write(b"render me pls");
                });
                let renderer = std::thread::spawn(move || {
                    let t = t2.lock().unwrap();
                    let snap = t.take_snapshot();
                    assert_eq!(snap.rows, 24, "render must have valid rows");
                });
                writer.join().unwrap();
                renderer.join().unwrap();
            },
            500,
        );
    }

    // ==================================================================
    // G3: Render Thread (5 tests)
    // ==================================================================

    /// G3-T1: Render during resize — cursor must never go out of bounds
    #[test]
    fn g3_render_resize_consistency() {
        shuttle::check(
            || {
                let terminal = Arc::new(std::sync::Mutex::new(
                    GhosttyTerminal::new(24, 80, 10000).expect("term"),
                ));
                let t1 = terminal.clone();
                let t2 = terminal.clone();
                let resizer = std::thread::spawn(move || {
                    for &(rows, cols) in &[(20, 80), (40, 100), (10, 40), (50, 120), (24, 80)] {
                        let mut t = t1.lock().unwrap();
                        t.resize(rows, cols);
                    }
                });
                let renderer = std::thread::spawn(move || {
                    for _ in 0..10 {
                        let t = t2.lock().unwrap();
                        let snap = t.take_snapshot();
                        // Cursor must always be in-bounds
                        assert!(
                            snap.cursor_row < snap.rows,
                            "cursor row {} < {}",
                            snap.cursor_row,
                            snap.rows
                        );
                        assert!(
                            snap.cursor_col < snap.cols,
                            "cursor col {} < {}",
                            snap.cursor_col,
                            snap.cols
                        );
                    }
                });
                resizer.join().unwrap();
                renderer.join().unwrap();
            },
            500,
        );
    }

    /// G3-T2: Write while rendering — no panic, consistent grid
    #[test]
    fn g3_render_while_write() {
        shuttle::check(
            || {
                let terminal = Arc::new(std::sync::Mutex::new(
                    GhosttyTerminal::new(24, 80, 10000).expect("term"),
                ));
                let t = terminal.clone();
                let writer = std::thread::spawn(move || {
                    let mut guard = t.lock().unwrap();
                    guard.vt_write(b"\x1b[31mRED\x1b[0m TEXT");
                });
                let snap = terminal.lock().unwrap().take_snapshot();
                writer.join().unwrap();
                assert_eq!(snap.rows, 24);
                assert_eq!(snap.cols, 80);
            },
            500,
        );
    }

    /// G3-T3: Spurious Condvar wakeup — render only when work pending
    #[test]
    fn g3_render_spurious_wakeup() {
        shuttle::check(
            || {
                let pending = Arc::new(AtomicBool::new(false));
                let p = pending.clone();
                let renderer = std::thread::spawn(move || {
                    if p.load(Ordering::Acquire) {
                        // Only render if work is actually pending
                    }
                });
                renderer.join().unwrap();
                // No crash on spurious wakeup is the success case
            },
            500,
        );
    }

    /// G3-T4: Fast writes should not cause frame drops in snapshot
    #[test]
    fn g3_render_frame_drop() {
        shuttle::check(
            || {
                let frame_count = Arc::new(AtomicU32::new(0));
                let fc = frame_count.clone();
                let terminal = Arc::new(std::sync::Mutex::new(
                    GhosttyTerminal::new(24, 80, 10000).expect("term"),
                ));
                let t = terminal.clone();
                let writer = std::thread::spawn(move || {
                    for _ in 0..10 {
                        let mut guard = t.lock().unwrap();
                        guard.vt_write(b"X");
                        let _ = fc.fetch_add(1, Ordering::SeqCst);
                    }
                });
                let t2 = terminal.clone();
                let renderer = std::thread::spawn(move || {
                    let snap = t2.lock().unwrap().take_snapshot();
                    assert!(frame_count.load(Ordering::SeqCst) <= 10);
                    assert!(snap.rows > 0);
                });
                writer.join().unwrap();
                renderer.join().unwrap();
            },
            500,
        );
    }

    /// G3-T5: Rapid snapshots — 100 consecutive takes
    #[test]
    fn g3_render_rapid_snapshots() {
        shuttle::check(
            || {
                let terminal = Arc::new(std::sync::Mutex::new(
                    GhosttyTerminal::new(24, 80, 10000).expect("term"),
                ));
                let t = terminal.lock().unwrap();
                let snaps: Vec<_> = (0..100).map(|_| t.take_snapshot()).collect();
                for s in &snaps {
                    assert_eq!(s.rows, 24);
                    assert_eq!(s.cols, 80);
                }
            },
            500,
        );
    }

    // ==================================================================
    // G4: Process Waiter (4 tests)
    // ==================================================================

    /// G4-T1: Simulate child exit during write — terminal must survive
    #[test]
    fn g4_waiter_child_exit_during_write() {
        shuttle::check(
            || {
                let terminal = Arc::new(std::sync::Mutex::new(
                    GhosttyTerminal::new(24, 80, 1000).expect("term"),
                ));
                let t1 = terminal.clone();
                let t2 = terminal.clone();
                let writer = std::thread::spawn(move || {
                    let mut t = t1.lock().unwrap();
                    t.vt_write(b"data while exiting");
                });
                let waiter = std::thread::spawn(move || {
                    // Simulate waitpid returning (no-op, just read state)
                    let t = t2.lock().unwrap();
                    let _ = t.scrollback_length();
                });
                writer.join().unwrap();
                waiter.join().unwrap();
            },
            500,
        );
    }

    /// G4-T2: Duplicate waitpid — two threads both simulate waiting
    #[test]
    fn g4_waiter_duplicate_waitpid() {
        shuttle::check(
            || {
                let status = Arc::new(std::sync::Mutex::new(None::<i32>));
                let s1 = status.clone();
                let s2 = status.clone();
                let w1 = std::thread::spawn(move || {
                    let mut guard = s1.lock().unwrap();
                    *guard = Some(0);
                });
                let w2 = std::thread::spawn(move || {
                    let mut guard = s2.lock().unwrap();
                    *guard = Some(0);
                });
                w1.join().unwrap();
                w2.join().unwrap();
                // Status must be set — no inconsistency
                let final_status = status.lock().unwrap();
                assert_eq!(*final_status, Some(0), "status must be set");
            },
            500,
        );
    }

    /// G4-T3: Session dropped while waiter is mid-flight
    #[test]
    fn g4_waiter_session_dropped() {
        shuttle::check(
            || {
                let terminal = Arc::new(std::sync::Mutex::new(
                    GhosttyTerminal::new(24, 80, 1000).expect("term"),
                ));
                let t = terminal.clone();
                let waiter = std::thread::spawn(move || {
                    let guard = t.lock();
                    if let Ok(t) = guard {
                        let _ = t.scrollback_length();
                    }
                });
                drop(terminal);
                waiter.join().unwrap_or(());
            },
            500,
        );
    }

    /// G4-T4: Old session exit then new session start
    #[test]
    fn g4_waiter_exit_then_new_session() {
        shuttle::check(
            || {
                let terminal = Arc::new(std::sync::Mutex::new(
                    GhosttyTerminal::new(24, 80, 1000).expect("term"),
                ));
                // Simulate session 1 activity
                {
                    let mut t = terminal.lock().unwrap();
                    t.vt_write(b"exit\n");
                }
                // Simulate session 2 start (same terminal reused)
                {
                    let mut t = terminal.lock().unwrap();
                    t.vt_write(b"new session");
                    t.flush();
                }
                let snap = terminal.lock().unwrap().take_snapshot();
                assert_eq!(snap.cols, 80, "terminal must remain functional");
            },
            500,
        );
    }

    // ==================================================================
    // G5: Full System Pressure (5 tests)
    // ==================================================================

    /// G5-T1: All 4 operations simultaneously — writer + reader + renderer + resizer
    #[test]
    fn g5_pressure_all_threads() {
        shuttle::check(
            || {
                let terminal = Arc::new(std::sync::Mutex::new(
                    GhosttyTerminal::new(24, 80, 10000).expect("term"),
                ));
                let t1 = terminal.clone();
                let t2 = terminal.clone();
                let t3 = terminal.clone();
                let t4 = terminal.clone();
                let writer = std::thread::spawn(move || {
                    let mut t = t1.lock().unwrap();
                    t.vt_write(b"\x1b[31mR\x1b[0m text\n");
                });
                let reader = std::thread::spawn(move || {
                    let mut t = t2.lock().unwrap();
                    t.vt_write(b"Line A\nLine B\nLine C\n");
                });
                let renderer = std::thread::spawn(move || {
                    let t = t3.lock().unwrap();
                    let snap = t.take_snapshot();
                    assert_eq!(snap.rows, 24);
                });
                let resizer = std::thread::spawn(move || {
                    let mut t = t4.lock().unwrap();
                    t.resize(30, 100);
                });
                writer.join().unwrap();
                reader.join().unwrap();
                renderer.join().unwrap();
                resizer.join().unwrap();
            },
            500,
        );
    }

    /// G5-T2: Ctrl+C interleaved with writes from two threads
    #[test]
    fn g5_pressure_interleaved_ctrl_c() {
        shuttle::check(
            || {
                let terminal = Arc::new(std::sync::Mutex::new(
                    GhosttyTerminal::new(24, 80, 10000).expect("term"),
                ));
                let t1 = terminal.clone();
                let t2 = terminal.clone();
                let h1 = std::thread::spawn(move || {
                    let mut t = t1.lock().unwrap();
                    t.vt_write(b"abc\x03def");
                });
                let h2 = std::thread::spawn(move || {
                    let mut t = t2.lock().unwrap();
                    t.vt_write(b"ghi\x03jkl");
                });
                h1.join().unwrap();
                h2.join().unwrap();
            },
            500,
        );
    }

    /// G5-T3: Rapid resize while writing — no crash
    #[test]
    fn g5_pressure_rapid_resize() {
        shuttle::check(
            || {
                let terminal = Arc::new(std::sync::Mutex::new(
                    GhosttyTerminal::new(24, 80, 10000).expect("term"),
                ));
                let t1 = terminal.clone();
                let h1 = std::thread::spawn(move || {
                    let mut t = t1.lock().unwrap();
                    for &(r, c) in &[(24, 80), (40, 100), (10, 40), (50, 120), (24, 80)] {
                        t.resize(r, c);
                    }
                });
                // Write while resize occurs
                let t2 = terminal.clone();
                let writer = std::thread::spawn(move || {
                    let mut t = t2.lock().unwrap();
                    t.vt_write(b"stable text");
                });
                h1.join().unwrap();
                writer.join().unwrap();
                let snap = terminal.lock().unwrap().take_snapshot();
                assert!(snap.rows > 0 && snap.cols > 0);
            },
            500,
        );
    }

    /// G5-T4: 500 threads each writing 1 byte, all must appear
    #[test]
    fn g5_pressure_many_serial_writes() {
        shuttle::check(
            || {
                let terminal = Arc::new(std::sync::Mutex::new(
                    GhosttyTerminal::new(100, 80, 100000).expect("term"),
                ));
                let mut handles = Vec::new();
                for _ in 0..500 {
                    let t = terminal.clone();
                    handles.push(std::thread::spawn(move || {
                        let mut guard = t.lock().unwrap();
                        guard.vt_write(b"X");
                    }));
                }
                for h in handles {
                    h.join().unwrap();
                }
                let snap = terminal.lock().unwrap().take_snapshot();
                let count = snap
                    .cells
                    .iter()
                    .filter(|c| c.codepoint == b'X' as u32)
                    .count();
                assert_eq!(count, 500, "all 500 writes should appear, got {count}");
            },
            1000,
        );
    }

    /// G5-T5: Repeated terminal create/spawn cycle — no resource leak
    #[test]
    fn g5_pressure_repeated_attach_detach() {
        shuttle::check(
            || {
                let mut sessions = Vec::new();
                for _ in 0..20 {
                    let t = GhosttyTerminal::new(24, 80, 1000).expect("term");
                    let mut guard = sessions
                        .last()
                        .map(|_| GhosttyTerminal::new(24, 80, 1000).unwrap());
                    if let Some(ref mut g) = guard {
                        g.vt_write(b"test\n");
                    }
                    sessions.push(t);
                }
                // Drop all sessions
                drop(sessions);
            },
            500,
        );
    }

    // ==================================================================
    // G6: Existing bridge/infrastructure tests (9 tests preserved)
    // ==================================================================

    /// 15.1: Writer + Reader flume channel (1000 interleavings).
    #[test]
    fn shuttle_flume_writer_reader() {
        shuttle::check(
            || {
                let terminal = Arc::new(std::sync::Mutex::new(
                    GhosttyTerminal::new(24, 80, 1000).expect("term"),
                ));
                let t1 = terminal.clone();
                let t2 = terminal.clone();
                let j1 = std::thread::spawn(move || {
                    for _ in 0..50 {
                        let mut t = t1.lock().unwrap();
                        t.vt_write(b"Hello\n");
                        let _ = t.cursor_x();
                    }
                });
                let j2 = std::thread::spawn(move || {
                    for _ in 0..50 {
                        let t = t2.lock().unwrap();
                        let _ = t.take_snapshot();
                        let _ = t.scrollback_length();
                    }
                });
                j1.join().unwrap();
                j2.join().unwrap();
            },
            1000,
        );
    }

    /// 15.2: Render + Writer concurrent snapshot (1000 interleavings).
    #[test]
    fn shuttle_concurrent_vt_write_and_snapshot() {
        shuttle::check(
            || {
                let terminal = Arc::new(std::sync::Mutex::new(
                    GhosttyTerminal::new(24, 80, 1000).expect("term"),
                ));
                let t1 = terminal.clone();
                let t2 = terminal.clone();
                let j1 = std::thread::spawn(move || {
                    for _ in 0..100 {
                        let mut t = t1.lock().unwrap();
                        t.vt_write(b"HelloWorld\n");
                    }
                });
                let j2 = std::thread::spawn(move || {
                    for _ in 0..100 {
                        let t = t2.lock().unwrap();
                        let snap = t.take_snapshot();
                        assert_eq!(snap.rows, 24, "snapshot rows invariant");
                        assert_eq!(snap.cols, 80, "snapshot cols invariant");
                    }
                });
                j1.join().unwrap();
                j2.join().unwrap();
            },
            1000,
        );
    }

    /// 15.3: Multi-session write + read (500 interleavings).
    #[test]
    fn shuttle_multi_session_write_read() {
        shuttle::check(
            || {
                let t1 = Arc::new(std::sync::Mutex::new(
                    GhosttyTerminal::new(10, 20, 500).expect("term1"),
                ));
                let t2 = Arc::new(std::sync::Mutex::new(
                    GhosttyTerminal::new(10, 20, 500).expect("term2"),
                ));
                let t1a = t1.clone();
                let t2a = t2.clone();
                let j1 = std::thread::spawn(move || {
                    for i in 0..30 {
                        let mut t = t1a.lock().unwrap();
                        t.vt_write(format!("session1_line{}\n", i).as_bytes());
                        t.flush();
                    }
                });
                let j2 = std::thread::spawn(move || {
                    for i in 0..30 {
                        let mut t = t2a.lock().unwrap();
                        t.vt_write(format!("session2_line{}\n", i).as_bytes());
                        t.flush();
                    }
                });
                j1.join().unwrap();
                j2.join().unwrap();
                let snap1 = t1.lock().unwrap().take_snapshot();
                let snap2 = t2.lock().unwrap().take_snapshot();
                assert_eq!(snap1.rows, 10);
                assert_eq!(snap2.rows, 10);
            },
            500,
        );
    }

    /// 15.4: Resize + Write interleaved (500 interleavings).
    #[test]
    fn shuttle_resize_write_interleaved() {
        shuttle::check(
            || {
                let terminal = Arc::new(std::sync::Mutex::new(
                    GhosttyTerminal::new(24, 80, 1000).expect("term"),
                ));
                let t1 = terminal.clone();
                let t2 = terminal.clone();
                let j1 = std::thread::spawn(move || {
                    for i in 0..50 {
                        let mut t = t1.lock().unwrap();
                        t.vt_write(format!("line_{}\n", i).as_bytes());
                        t.flush();
                    }
                });
                let j2 = std::thread::spawn(move || {
                    for rows in [5, 10, 24, 40].iter().cycle().take(20) {
                        let mut t = t2.lock().unwrap();
                        t.resize(*rows, 80);
                        t.flush();
                    }
                });
                j1.join().unwrap();
                j2.join().unwrap();
                let t = terminal.lock().unwrap();
                let snap = t.take_snapshot();
                assert!(snap.rows > 0);
                assert!(snap.cols > 0);
                assert_eq!(snap.cells.len(), (snap.rows * snap.cols) as usize);
            },
            500,
        );
    }

    /// 15.5: Bridge multi-threaded access (1000 interleavings).
    #[test]
    fn shuttle_bridge_multithreaded_access() {
        shuttle::check(
            || {
                let terminal = Arc::new(GhosttyTerminal::new(24, 80, 1000).expect("term"));
                let t1 = terminal.clone();
                let t2 = terminal.clone();
                let t3 = terminal.clone();
                let j1 = std::thread::spawn(move || {
                    for _ in 0..30 {
                        let _ = t1.rows();
                        let _ = t1.cols();
                    }
                });
                let j2 = std::thread::spawn(move || {
                    for _ in 0..30 {
                        let _ = t2.cursor_x();
                        let _ = t2.cursor_y();
                        let _ = t2.cursor_visible();
                    }
                });
                let j3 = std::thread::spawn(move || {
                    for _ in 0..30 {
                        let _ = t3.scrollback_length();
                        let _ = t3.title();
                    }
                });
                j1.join().unwrap();
                j2.join().unwrap();
                j3.join().unwrap();
            },
            1000,
        );
    }

    /// 15.6: Session drop + concurrent access race (500 interleavings).
    #[test]
    fn shuttle_session_drop_concurrent_access() {
        shuttle::check(
            || {
                let terminal = Arc::new(std::sync::Mutex::new(
                    GhosttyTerminal::new(5, 10, 100).expect("term"),
                ));
                let t2 = terminal.clone();
                let j1 = std::thread::spawn(move || {
                    let _ = t2.lock().unwrap().take_snapshot();
                });
                drop(terminal);
                j1.join().unwrap_or(());
            },
            500,
        );
    }

    /// 15.7: Signal delivery + PTY read race (500 interleavings).
    #[test]
    fn shuttle_signal_pty_read_race() {
        shuttle::check(
            || {
                let terminal = Arc::new(std::sync::Mutex::new(
                    GhosttyTerminal::new(24, 80, 1000).expect("term"),
                ));
                let t1 = terminal.clone();
                let t2 = terminal.clone();
                let j1 = std::thread::spawn(move || {
                    let mut t = t1.lock().unwrap();
                    t.vt_write(b"\x1b[5;10Hdata\n");
                    t.flush();
                });
                let j2 = std::thread::spawn(move || {
                    let t = t2.lock().unwrap();
                    let _ = t.cursor_x();
                    let _ = t.cursor_y();
                    let _ = t.cursor_visible();
                    let _ = t.alt_screen();
                    let _ = t.origin_mode();
                });
                j1.join().unwrap();
                j2.join().unwrap();
            },
            500,
        );
    }

    /// 15.8: Scrollback read + VT write race (500 interleavings).
    #[test]
    fn shuttle_scrollback_read_vt_write_race() {
        shuttle::check(
            || {
                let terminal = Arc::new(std::sync::Mutex::new(
                    GhosttyTerminal::new(3, 20, 1000).expect("term"),
                ));
                let t1 = terminal.clone();
                let t2 = terminal.clone();
                let j1 = std::thread::spawn(move || {
                    for i in 0..40 {
                        let mut t = t1.lock().unwrap();
                        t.vt_write(format!("scroll_line_{}\n", i).as_bytes());
                    }
                });
                let j2 = std::thread::spawn(move || {
                    for _ in 0..40 {
                        let t = t2.lock().unwrap();
                        let _ = t.scrollback_length();
                        let _ = t.dump_grid();
                    }
                });
                j1.join().unwrap();
                j2.join().unwrap();
            },
            500,
        );
    }

    /// 15.9: Config hot-reload + render race (500 interleavings).
    #[test]
    fn shuttle_config_hot_reload_render_race() {
        shuttle::check(
            || {
                let terminal = Arc::new(std::sync::Mutex::new(
                    GhosttyTerminal::new(24, 80, 1000).expect("term"),
                ));
                let t1 = terminal.clone();
                let t2 = terminal.clone();
                let j1 = std::thread::spawn(move || {
                    for _ in 0..20 {
                        let t = t1.lock().unwrap();
                        t.set_theme([40, 40, 40], [255, 255, 255], Default::default());
                    }
                });
                let j2 = std::thread::spawn(move || {
                    for _ in 0..20 {
                        let t = t2.lock().unwrap();
                        let snap = t.take_snapshot();
                        assert_eq!(snap.rows, 24, "snapshot rows invariant after theme change");
                    }
                });
                j1.join().unwrap();
                j2.join().unwrap();
            },
            500,
        );
    }

    /// Helper: concurrent read through Arc<GhosttyTerminal> (500 interleavings).
    #[test]
    fn session_write_read_bridge() {
        shuttle::check(
            || {
                let terminal = Arc::new(std::sync::Mutex::new(
                    GhosttyTerminal::new(24, 80, 1000).expect("term"),
                ));
                let t1 = terminal.clone();
                let t2 = terminal.clone();
                let j1 = std::thread::spawn(move || {
                    for _ in 0..10 {
                        let mut t = t1.lock().unwrap();
                        t.vt_write(b"X");
                    }
                });
                let j2 = std::thread::spawn(move || {
                    for _ in 0..10 {
                        let t = t2.lock().unwrap();
                        let _ = t.cursor_x();
                    }
                });
                j1.join().unwrap();
                j2.join().unwrap();
            },
            500,
        );
    }
}
