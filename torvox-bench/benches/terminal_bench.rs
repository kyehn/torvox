use criterion::{BatchSize, Criterion, Throughput, criterion_group, criterion_main};
use std::hint::black_box;
use torvox_core::grid::Grid;
use torvox_core::line::Line;
use torvox_terminal::ghostty_terminal::{CellSnapshot, GhosttyTerminal};

// ── B01-B09: Existing (preserved and enhanced) ────────────────────────

fn bench_device_create(c: &mut Criterion) {
    c.bench_function("B01_device_create_24x80", |b| {
        b.iter(|| {
            let t = GhosttyTerminal::new(24, 80, 1000).ok();
            black_box(t)
        });
    });
    c.bench_function("B01_device_create_200x100", |b| {
        b.iter(|| {
            let t = GhosttyTerminal::new(200, 100, 50000).ok();
            black_box(t)
        });
    });
}

fn bench_vt_parse_plain_text(c: &mut Criterion) {
    let input = b"Hello, World! This is a terminal benchmark.\n";
    c.bench_function("B07_vt_parse_plain_text", |b| {
        let mut terminal = GhosttyTerminal::new(24, 80, 1000).unwrap();
        b.iter(|| {
            terminal.vt_write(input);
        });
    });
}

fn bench_vt_parse_sgr_sequences(c: &mut Criterion) {
    let input = b"\x1b[1mBold\x1b[0m \x1b[31mRed\x1b[0m \x1b[1;32mGreenBold\x1b[0m\n";
    c.bench_function("B07_vt_parse_sgr", |b| {
        let mut terminal = GhosttyTerminal::new(24, 80, 1000).unwrap();
        b.iter(|| {
            terminal.vt_write(input);
        });
    });
}

fn bench_vt_parse_cursor_movement(c: &mut Criterion) {
    let input = b"\x1b[2A\x1b[3B\x1b[4C\x1b[5D\x1b[10;20H";
    c.bench_function("B07_vt_cursor_movement", |b| {
        let mut terminal = GhosttyTerminal::new(24, 80, 1000).unwrap();
        b.iter(|| {
            terminal.vt_write(input);
        });
    });
}

fn bench_vt_parse_large_output(c: &mut Criterion) {
    let line =
        b"AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA\n";
    let mut input = Vec::with_capacity(line.len() * 1000);
    for _ in 0..1000 {
        input.extend_from_slice(line);
    }
    let mut group = c.benchmark_group("B07_vt_1k_lines");
    group.throughput(Throughput::Bytes(input.len() as u64));
    group.bench_function("write_1k_lines", |b| {
        let mut terminal = GhosttyTerminal::new(24, 80, 1000).unwrap();
        b.iter(|| {
            terminal.vt_write(&input);
        });
    });
    group.finish();
}

fn bench_grid_sizeof(c: &mut Criterion) {
    c.bench_function("B08_grid_sizeof_24x80_no_sb", |b| {
        b.iter(|| {
            let g = Grid::new(24, 80);
            black_box(g.rows());
        });
    });
    c.bench_function("B08_grid_sizeof_24x80_50k_sb", |b| {
        b.iter(|| {
            let g = Grid::with_scrollback(24, 80, 50_000);
            black_box(g.rows());
        });
    });
}

fn bench_grid_cell_access(c: &mut Criterion) {
    let mut g = Grid::new(24, 80);
    for r in 0..24 {
        g.fill_cells(r, 'A', 0, 80);
    }
    c.bench_function("B08_grid_row_cells_24x80", |b| {
        b.iter(|| {
            let mut count = 0;
            for r in 0..24 {
                if let Some(cells) = g.row_cells(r) {
                    count += cells.len();
                }
            }
            black_box(count);
        });
    });
}

fn bench_grid_resize(c: &mut Criterion) {
    c.bench_function("B17_grid_resize_24x80_to_50x120", |b| {
        b.iter(|| {
            let mut g = Grid::new(24, 80);
            g.resize(50, 120);
            black_box(&g);
        });
    });
    c.bench_function("B17_grid_resize_50x120_to_24x80", |b| {
        b.iter(|| {
            let mut g = Grid::new(50, 120);
            g.resize(24, 80);
            black_box(&g);
        });
    });
}

fn bench_grid_scrollback(c: &mut Criterion) {
    c.bench_function("B11_scrollback_push_1k_lines", |b| {
        b.iter(|| {
            let mut g = Grid::with_scrollback(24, 80, 100_000);
            for _ in 0..1000 {
                g.push_scrollback(Line::new(80));
            }
            black_box(&g);
        });
    });
}

fn bench_grid_fill(c: &mut Criterion) {
    let mut g = Grid::new(24, 80);
    c.bench_function("B16_grid_fill_row", |b| {
        b.iter(|| {
            g.fill_cells(0, 'X', 0, 80);
            black_box(&g);
        });
    });
}

fn bench_ghostty_screenshot(c: &mut Criterion) {
    let mut terminal = GhosttyTerminal::new(24, 80, 1000).unwrap();
    terminal.vt_write(b"Hello, world! This is some sample terminal output.\n");
    std::thread::sleep(std::time::Duration::from_millis(20));
    c.bench_function("B08_ghostty_take_snapshot_24x80", |b| {
        b.iter(|| {
            let snap = terminal.take_snapshot();
            black_box(snap.cells.len());
        });
    });
}

fn bench_vt_throughput_100k_lines(c: &mut Criterion) {
    let line =
        b"AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA\n";
    let mut input = Vec::with_capacity(line.len() * 100_000);
    for _ in 0..100_000 {
        input.extend_from_slice(line);
    }
    let mut group = c.benchmark_group("B12_vt_throughput_100k_lines");
    group.throughput(Throughput::Bytes(input.len() as u64));
    group.bench_function("write_100k_lines", |b| {
        let mut terminal = GhosttyTerminal::new(24, 80, 100_000).unwrap();
        b.iter(|| {
            terminal.vt_write(&input);
        });
    });
    group.finish();
}

fn bench_vt_throughput_ls_la(c: &mut Criterion) {
    let ls_line = b"-rw-r--r-- 1 root root    1234 Jan 01 12:00 some_file_name_here.txt\n";
    let mut input = Vec::with_capacity(ls_line.len() * 100_000);
    for _ in 0..100_000 {
        input.extend_from_slice(ls_line);
    }
    let mut group = c.benchmark_group("B12_vt_throughput_ls_la");
    group.throughput(Throughput::Bytes(input.len() as u64));
    group.bench_function("write_ls_output", |b| {
        let mut terminal = GhosttyTerminal::new(24, 80, 100_000).unwrap();
        b.iter(|| {
            terminal.vt_write(&input);
        });
    });
    group.finish();
}

fn bench_input_to_pixel_latency(c: &mut Criterion) {
    c.bench_function("B12_input_to_pixel_single_char", |b| {
        b.iter(|| {
            let mut terminal = GhosttyTerminal::new(24, 80, 1000).unwrap();
            terminal.vt_write(b"a");
        });
    });
    c.bench_function("B12_input_to_pixel_line_with_newline", |b| {
        b.iter(|| {
            let mut terminal = GhosttyTerminal::new(24, 80, 1000).unwrap();
            terminal.vt_write(b"Hello, World!\n");
        });
    });
}

fn bench_grid_resize_large_scrollback(c: &mut Criterion) {
    c.bench_function("B17_grid_resize_24x80_to_48x160_10k_sb", |b| {
        b.iter_batched(
            || {
                let mut g = Grid::with_scrollback(24, 80, 100_000);
                for _ in 0..10_000 {
                    g.push_scrollback(Line::new(80));
                }
                g
            },
            |mut g| {
                g.resize(48, 160);
                black_box(&g);
            },
            BatchSize::SmallInput,
        );
    });
}

fn bench_scrollback_push_10k(c: &mut Criterion) {
    c.bench_function("B11_scrollback_push_10k_lines", |b| {
        b.iter_batched(
            || Grid::with_scrollback(24, 80, 100_000),
            |mut g| {
                for _ in 0..10_000 {
                    g.push_scrollback(Line::new(80));
                }
                black_box(&g);
            },
            BatchSize::SmallInput,
        );
    });
}

// ── B10: Session startup time ─────────────────────────────────────────
fn bench_session_startup(c: &mut Criterion) {
    c.bench_function("B10_session_create_24x80_1k_sb", |b| {
        b.iter(|| {
            let t = GhosttyTerminal::new(24, 80, 1000);
            black_box(t.ok());
        });
    });
    c.bench_function("B10_session_create_200x100_50k_sb", |b| {
        b.iter(|| {
            let t = GhosttyTerminal::new(200, 100, 50000);
            black_box(t.ok());
        });
    });
}

// ── B13: CSI parse throughput (100K CSI sequences) ───────────────────────
fn bench_csi_throughput(c: &mut Criterion) {
    let mut input = Vec::with_capacity(100_000 * 5);
    for _ in 0..100_000 {
        input.extend_from_slice(b"\x1b[1m");
    }
    let mut group = c.benchmark_group("B13_csi_throughput_100k");
    group.throughput(Throughput::Bytes(input.len() as u64));
    group.bench_function("write_100k_sgr_set", |b| {
        let mut terminal = GhosttyTerminal::new(24, 80, 1000).unwrap();
        b.iter(|| {
            terminal.vt_write(&input);
        });
    });
    group.finish();
}

fn bench_csi_mixed_throughput(c: &mut Criterion) {
    use std::fmt::Write;
    let mut input = String::with_capacity(100_000 * 12);
    for i in 0..100_000 {
        let _ = write!(
            input,
            "\x1b[{};{}H\x1b[{}m",
            (i % 24) + 1,
            (i % 80) + 1,
            i % 8 + 1
        );
    }
    let input = input.into_bytes();
    let mut group = c.benchmark_group("B13_csi_mixed_100k");
    group.throughput(Throughput::Bytes(input.len() as u64));
    group.bench_function("write_mixed_csi", |b| {
        let mut terminal = GhosttyTerminal::new(24, 80, 1000).unwrap();
        b.iter(|| {
            terminal.vt_write(&input);
        });
    });
    group.finish();
}

// ── B14: OSC parse throughput (10K OSC sequences) ──────────────────────
fn bench_osc_throughput(c: &mut Criterion) {
    let mut input = Vec::with_capacity(10_000 * 15);
    for i in 0..10_000 {
        input.extend_from_slice(b"\x1b]4;");
        for b in format!("{}", i % 256).bytes() {
            input.push(b);
        }
        input.extend_from_slice(b";#ff0000\x1b\\");
    }
    let mut group = c.benchmark_group("B14_osc_throughput_10k");
    group.throughput(Throughput::Bytes(input.len() as u64));
    group.bench_function("write_10k_osc", |b| {
        let mut terminal = GhosttyTerminal::new(24, 80, 1000).unwrap();
        b.iter(|| {
            terminal.vt_write(&input);
        });
    });
    group.finish();
}

fn bench_osc_heavy_throughput(c: &mut Criterion) {
    let mut input = Vec::with_capacity(1000 * 100);
    for _ in 0..1000 {
        input.extend_from_slice(
            b"\x1b]0;This is a very long window title that tests string parsing in the terminal emulator\x1b\\",
        );
    }
    let mut group = c.benchmark_group("B14_osc_heavy_1000");
    group.throughput(Throughput::Bytes(input.len() as u64));
    group.bench_function("write_1k_long_osc", |b| {
        let mut terminal = GhosttyTerminal::new(24, 80, 1000).unwrap();
        b.iter(|| {
            terminal.vt_write(&input);
        });
    });
    group.finish();
}

// ── B15: Alt screen toggle (DECSET 1049 × 1000) ────────────────────
fn bench_alt_screen_switch(c: &mut Criterion) {
    let mut group = c.benchmark_group("B15_alt_screen_switch_1000");
    group.bench_function("decset_1049_toggle_1000x", |b| {
        b.iter(|| {
            let mut terminal = GhosttyTerminal::new(24, 80, 1000).unwrap();
            for _ in 0..1000 {
                terminal.vt_write(b"\x1b[?1049h\x1b[?1049l");
            }
        });
    });
    group.finish();
}

// ── B16: Color cycling (256 colors × 100) ────────────────────────────────
fn bench_color_cycling(c: &mut Criterion) {
    use std::fmt::Write;
    let mut input = String::with_capacity(256 * 100 * 12);
    for _ in 0..100 {
        for i in 0..256 {
            let _ = write!(input, "\x1b[38;5;{}mX\x1b[48;5;{}mY", i, (255 - i));
        }
    }
    let input = input.into_bytes();
    let mut group = c.benchmark_group("B16_color_cycle_256x100");
    group.throughput(Throughput::Bytes(input.len() as u64));
    group.bench_function("write_color_cycle", |b| {
        let mut terminal = GhosttyTerminal::new(24, 80, 1000).unwrap();
        b.iter(|| {
            terminal.vt_write(&input);
        });
    });
    group.finish();
}

// ── B17: Window resize (resize × 500) ─────────────────────────
fn bench_window_resize(c: &mut Criterion) {
    let mut group = c.benchmark_group("B17_window_resize_500");
    group.bench_function("resize_24x80_to_50x120_500x", |b| {
        b.iter_batched(
            || Grid::new(24, 80),
            |mut g| {
                for _ in 0..500 {
                    g.resize(50, 120);
                    g.resize(24, 80);
                }
                black_box(&g);
            },
            BatchSize::SmallInput,
        );
    });
    group.finish();
}

fn bench_ghostty_resize(c: &mut Criterion) {
    let mut group = c.benchmark_group("B17_ghostty_resize_100");
    group.bench_function("ghostty_resize_24x80_50x120_100x", |b| {
        b.iter(|| {
            let mut terminal = GhosttyTerminal::new(24, 80, 1000).unwrap();
            for _ in 0..100 {
                terminal.resize(50, 120);
                terminal.resize(24, 80);
            }
        });
    });
    group.finish();
}

// ── B19: Clipboard operations (OSC 52 × 500) ────────────────────────────
fn bench_clipboard_ops(c: &mut Criterion) {
    let mut group = c.benchmark_group("B19_clipboard_ops_500");
    group.bench_function("osc_52_write_500x", |b| {
        b.iter(|| {
            let mut terminal = GhosttyTerminal::new(24, 80, 1000).unwrap();
            for _ in 0..500 {
                terminal.vt_write(b"\x1b]52;c;dGVzdA==\x1b\\");
            }
        });
    });
    group.finish();
}

// ── B24: Grid snapshot allocation (snapshot × 10000) ──────────────────────
fn bench_grid_snapshot_alloc(c: &mut Criterion) {
    let mut terminal = GhosttyTerminal::new(24, 80, 1000).unwrap();
    terminal.vt_write(b"Some sample content to snapshot.\n");
    let mut group = c.benchmark_group("B24_grid_snapshot_10k");
    group.bench_function("snapshot_10k_times", |b| {
        b.iter(|| {
            for _ in 0..10_000 {
                let snap = terminal.take_snapshot();
                black_box(snap.cells.len());
            }
        });
    });
    group.finish();
}

// ── Additional performance benchmarks (Kitty 14 style) ─────────────────────
fn bench_kitty_scrolling_heavy(c: &mut Criterion) {
    use std::fmt::Write;
    let mut input = String::with_capacity(100_000 * 40);
    for i in 0..100_000 {
        let _ = write!(
            input,
            "\x1b[{};{}H\x1b[{}mLine {}\r\n",
            (i % 24) + 1,
            1,
            i % 8 + 1,
            i
        );
    }
    let input = input.into_bytes();
    let mut group = c.benchmark_group("kitty_heavy_scroll_100k");
    group.throughput(Throughput::Bytes(input.len() as u64));
    group.bench_function("mixed_output_scroll", |b| {
        let mut terminal = GhosttyTerminal::new(24, 80, 100_000).unwrap();
        b.iter(|| {
            terminal.vt_write(&input);
        });
    });
    group.finish();
}

fn bench_memory_baseline(c: &mut Criterion) {
    use std::alloc::Layout;
    c.bench_function("B20_memory_baseline_empty_grid", |b| {
        b.iter(|| {
            let g = Grid::new(24, 80);
            let size = Layout::for_value(&g).size();
            black_box(size);
        });
    });
    c.bench_function("B20_memory_baseline_200x100_10k_sb", |b| {
        b.iter(|| {
            let mut g = Grid::with_scrollback(200, 100, 10_000);
            for _ in 0..10_000 {
                g.push_scrollback(Line::new(100));
            }
            let size = Layout::for_value(&g).size();
            black_box(size);
        });
    });
}

fn bench_grid_alloc_stress(c: &mut Criterion) {
    c.bench_function("B21_grid_alloc_1000_new_drop", |b| {
        b.iter(|| {
            for _ in 0..1000 {
                let g = Grid::with_scrollback(24, 80, 100_000);
                black_box(g.rows());
            }
        });
    });
}

fn bench_leak_detection(c: &mut Criterion) {
    c.bench_function("B22_leak_1000_sessions", |b| {
        b.iter(|| {
            let count: usize = (0..1000)
                .filter_map(|_| GhosttyTerminal::new(24, 80, 1000).ok())
                .count();
            black_box(count);
        });
    });
}

// ── kitty 14 style: high memory benchmark ──────────────────────────────────
fn bench_kitty_1024_glyphs(c: &mut Criterion) {
    use guillotiere::{AtlasAllocator, Size};
    let mut group = c.benchmark_group("kitty_atlas_1024_glyphs");
    let glyphs: Vec<Size> = (0..1024).map(|_| Size::new(10, 20)).collect();
    group.bench_function("alloc_dealloc", |b| {
        b.iter_batched(
            || AtlasAllocator::new(Size::new(4096, 4096)),
            |mut alloc| {
                let ids: Vec<_> = glyphs
                    .iter()
                    .filter_map(|sz| alloc.allocate(*sz))
                    .map(|a| a.id)
                    .collect();
                for id in ids {
                    alloc.deallocate(id);
                }
            },
            BatchSize::SmallInput,
        );
    });
    group.finish();
}

// ── B18: Selection operations (Selection × 1000) ──────────────────────────
fn bench_selection_ops(c: &mut Criterion) {
    use torvox_core::selection::{Selection, SelectionAnchor, SelectionMode};
    c.bench_function("B18_selection_char_1000", |b| {
        let mut sel = Selection::new(
            SelectionAnchor { row: 0, col: 0 },
            SelectionAnchor { row: 0, col: 0 },
            SelectionMode::Char,
        );
        b.iter(|| {
            for _ in 0..1000 {
                sel.start = SelectionAnchor { row: 5, col: 10 };
                sel.end = SelectionAnchor { row: 15, col: 20 };
                black_box(sel.is_ordered());
                sel.start = SelectionAnchor { row: 0, col: 0 };
                sel.end = SelectionAnchor { row: 0, col: 0 };
                black_box(sel.contains(0, 0));
            }
        });
    });
}

// ── B23: Fuzz throughput ─────────────────────────────────────────
fn bench_fuzz_throughput(c: &mut Criterion) {
    use std::io::Write;
    let mut input = Vec::with_capacity(500_000);
    for i in 0..1_000 {
        let _ = write!(
            input,
            "\x1b[{};{}H\x1b[{}mLine{}\r\n",
            (i % 24) + 1,
            1,
            i % 8 + 1,
            i
        );
    }
    let input_len = input.len();
    let mut group = c.benchmark_group("B23_fuzz_throughput_1k_sequences");
    group.throughput(Throughput::Bytes(input_len as u64));
    group.bench_function("write_fuzz_style", |b| {
        let mut terminal = GhosttyTerminal::new(24, 80, 100_000).unwrap();
        b.iter(|| {
            terminal.vt_write(&input);
        });
    });
    group.finish();
}

// ── B25: CPU idle usage estimate ────────────────────────────────────
// ── B27: Cached instance copy overhead (the P0 fix) ─────────────────
fn bench_cached_copy_vs_swap(c: &mut Criterion) {
    use std::mem::size_of;
    use torvox_renderer::gpu::CellInstance;

    const SIZE: usize = 10_000; // ~800KB, realistic frame size
    let items: Vec<CellInstance> = std::iter::repeat(CellInstance {
        quad_origin: [0.0; 2],
        atlas_offset: [0.0; 2],
        atlas_size: [1.0; 2],
        fg_color: [1.0; 4],
        bg_color: [0.0; 4],
        quad_size: [10.0; 2],
        flags: 0.0,
        bearing: [0.0; 2],
        glyph_advance_width: 10.0,
    })
    .take(SIZE)
    .collect();

    let total_bytes = SIZE * size_of::<CellInstance>();
    let mut group = c.benchmark_group("B27_cached_copy");
    group.throughput(Throughput::Bytes(total_bytes as u64));

    // OLD: clear() + extend_from_slice() — ~800KB memcpy every frame
    group.bench_function("clear_extend_10k", |b| {
        let mut cached = Vec::with_capacity(items.len());
        b.iter(|| {
            cached.clear();
            cached.extend_from_slice(&items);
            std::hint::black_box(cached.as_ptr());
        });
    });

    // NEW: std::mem::swap — O(1) pointer swap, zero copy
    group.bench_function("swap_10k", |b| {
        b.iter_batched(
            || {
                // cached=old frame data (empty for first frame)
                // data=current frame instances
                let cached = Vec::new();
                let data = items.clone();
                (cached, data)
            },
            |(mut cached, mut data)| {
                std::mem::swap(&mut cached, &mut data);
                // Now cached holds current frame (for dirty row cache)
                // data holds previous cached (will be cleared by build_cell_instances_into)
                std::hint::black_box(cached.as_ptr());
            },
            BatchSize::SmallInput,
        );
    });

    group.finish();
}

fn bench_cpu_idle(c: &mut Criterion) {
    c.bench_function("B25_cpu_idle_terminal_exists", |b| {
        b.iter(|| {
            let terminal = GhosttyTerminal::new(24, 80, 1000).unwrap();
            std::hint::black_box(terminal.take_snapshot().cells.len());
        });
    });
}

fn bench_grid_fill_comprehensive(c: &mut Criterion) {
    let mut group = c.benchmark_group("kitty_grid_fill_comprehensive");
    group.bench_function("fill_5x5_1000_cells_each", |b| {
        let mut g = Grid::new(50, 100);
        b.iter(|| {
            for r in 0..50 {
                for c in (0..100).step_by(2) {
                    g.fill_cells(r, 'X', c, c + 1);
                }
            }
            black_box(&g);
        });
    });
    group.finish();
}

// ── B26: GPU render throughput ─────────────────────────────────────
fn bench_gpu_render_throughput(c: &mut Criterion) {
    use torvox_core::cursor::CursorStyle;
    use torvox_renderer::font::FontPipeline;
    use torvox_renderer::gpu::GpuContext;

    let mut ctx = GpuContext::new_with_no_surface();
    ctx.surface_config = Some(wgpu::SurfaceConfiguration {
        width: 800,
        height: 600,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        present_mode: wgpu::PresentMode::Fifo,
        alpha_mode: wgpu::CompositeAlphaMode::Auto,
        view_formats: vec![],
        desired_maximum_frame_latency: 2,
    });
    ctx.set_bg_color([0, 0, 0]);
    ctx.initialize_pipeline_and_bind_group(256, 256, 800, 600);
    let mut font_pipeline = FontPipeline::new(256, 256, 14.0);

    let mut terminal = GhosttyTerminal::new(24, 80, 1000).unwrap();
    terminal.vt_write(b"\x1b[2J\x1b[1;1H");
    for row in 0..24u8 {
        terminal.vt_write(&[
            b'R',
            0x30 + (row & 0xf),
            b' ',
            b'H',
            0x30 + row,
            b'e',
            b'l',
            b'l',
            b'o',
            b'\n',
        ]);
    }
    terminal.flush();
    let snap = terminal.take_snapshot();

    let mut instance_buffer = Vec::new();
    let mut row_ends = Vec::new();
    let mut cached_instances = Vec::new();
    let mut cached_row_ends = Vec::new();

    // Warm up: build one frame to populate cache
    {
        let config = torvox_renderer::gpu::CellInstanceConfig {
            atlas_width: 256.0,
            atlas_height: 256.0,
            projection_height: 600.0,
            dirty_rows: &[],
            selection: None,
            selection_bg: None,
            search_highlights: &[],
            cursor_color: Some([1.0, 1.0, 1.0, 1.0]),
            cursor_style: CursorStyle::Block,
            cached_instances: &cached_instances,
            cached_row_ends: &cached_row_ends,
        };
        torvox_renderer::gpu::build_cell_instances_into(
            &snap,
            &mut font_pipeline,
            config,
            &mut instance_buffer,
            &mut row_ends,
        );
        cached_instances.clear();
        cached_instances.extend_from_slice(&instance_buffer);
        cached_row_ends.clear();
        cached_row_ends.extend_from_slice(&row_ends);
    }

    c.bench_function("B26_gpu_render_snapshot_24x80", |b| {
        b.iter(|| {
            let config = torvox_renderer::gpu::CellInstanceConfig {
                atlas_width: 256.0,
                atlas_height: 256.0,
                projection_height: 600.0,
                dirty_rows: &[],
                selection: None,
                selection_bg: None,
                search_highlights: &[],
                cursor_color: Some([1.0, 1.0, 1.0, 1.0]),
                cursor_style: CursorStyle::Block,
                cached_instances: &cached_instances,
                cached_row_ends: &cached_row_ends,
            };
            instance_buffer.clear();
            row_ends.clear();
            torvox_renderer::gpu::build_cell_instances_into(
                &snap,
                &mut font_pipeline,
                config,
                &mut instance_buffer,
                &mut row_ends,
            );
            // Simulate old surface.rs cached copy (the P0 bottleneck)
            cached_instances.clear();
            cached_instances.extend_from_slice(&instance_buffer);
            cached_row_ends.clear();
            cached_row_ends.extend_from_slice(&row_ends);
            let pixels = ctx.render_to_buffer(&instance_buffer, &[]).unwrap();
            black_box(pixels.len());
        });
    });

    // Same setup, but measure instance building WITHOUT GPU render
    c.bench_function("B26_gpu_build_only_24x80", |b| {
        b.iter(|| {
            let config = torvox_renderer::gpu::CellInstanceConfig {
                atlas_width: 256.0,
                atlas_height: 256.0,
                projection_height: 600.0,
                dirty_rows: &[],
                selection: None,
                selection_bg: None,
                search_highlights: &[],
                cursor_color: Some([1.0, 1.0, 1.0, 1.0]),
                cursor_style: CursorStyle::Block,
                cached_instances: &cached_instances,
                cached_row_ends: &cached_row_ends,
            };
            instance_buffer.clear();
            row_ends.clear();
            torvox_renderer::gpu::build_cell_instances_into(
                &snap,
                &mut font_pipeline,
                config,
                &mut instance_buffer,
                &mut row_ends,
            );
            std::hint::black_box(instance_buffer.len());
        });
    });

    // Measure GPU render ONLY with pre-built instances
    {
        // Build instances once
        let config = torvox_renderer::gpu::CellInstanceConfig {
            atlas_width: 256.0,
            atlas_height: 256.0,
            projection_height: 600.0,
            dirty_rows: &[],
            selection: None,
            selection_bg: None,
            search_highlights: &[],
            cursor_color: Some([1.0, 1.0, 1.0, 1.0]),
            cursor_style: CursorStyle::Block,
            cached_instances: &cached_instances,
            cached_row_ends: &cached_row_ends,
        };
        let mut built_instances = Vec::new();
        let mut _row_ends = Vec::new();
        torvox_renderer::gpu::build_cell_instances_into(
            &snap,
            &mut font_pipeline,
            config,
            &mut built_instances,
            &mut _row_ends,
        );

        c.bench_function("B26_gpu_render_only_24x80", |b| {
            b.iter(|| {
                let pixels = ctx.render_to_buffer(&built_instances, &[]).unwrap();
                std::hint::black_box(pixels.len());
            });
        });
    }
}

// ── B50-B52: Grid cell access patterns ─────────────────────────────
fn bench_cell_random_access(c: &mut Criterion) {
    let g = Grid::new(24, 80);
    let mut rng: u32 = 12345;
    let mut positions = Vec::with_capacity(1000);
    for _ in 0..1000 {
        rng = rng.wrapping_mul(1664525).wrapping_add(1013904223);
        positions.push(((rng >> 16) % 24, (rng & 0xFFFF) % 80));
    }
    c.bench_function("B50_cell_random_access_1000", |b| {
        b.iter(|| {
            let mut sum: u32 = 0;
            for &(row, col) in &positions {
                if let Some(cell) = g.cell(row, col) {
                    sum += cell.width as u32;
                }
            }
            black_box(sum);
        });
    });
}

fn bench_cell_sequential(c: &mut Criterion) {
    let g = Grid::new(24, 80);
    c.bench_function("B51_cell_sequential", |b| {
        b.iter(|| {
            let mut sum: u32 = 0;
            for row in 0..24 {
                for col in 0..80 {
                    if let Some(cell) = g.cell(row, col) {
                        sum += cell.width as u32;
                    }
                }
            }
            black_box(sum);
        });
    });
}

fn bench_cell_column_by_column(c: &mut Criterion) {
    let g = Grid::new(24, 80);
    c.bench_function("B52_cell_column_by_column", |b| {
        b.iter(|| {
            let mut sum: u32 = 0;
            for col in 0..80 {
                for row in 0..24 {
                    if let Some(cell) = g.cell(row, col) {
                        sum += cell.width as u32;
                    }
                }
            }
            black_box(sum);
        });
    });
}

// ── B53-B55: Selection operations ─────────────────────────────────
fn bench_selection_word(c: &mut Criterion) {
    use torvox_core::selection::{Selection, SelectionAnchor, SelectionMode};
    let mut g = Grid::new(24, 80);
    for row in 0..24 {
        let mut col = 0u32;
        for word_idx in 0..10 {
            let len = (word_idx % 5 + 2) as u32;
            for offset in 0..len {
                if col + offset < 80 {
                    g.fill_cells(row, b'a' as char, col + offset, col + offset + 1);
                }
            }
            col += len;
            if col < 80 {
                g.fill_cells(row, ' ', col, col + 1);
                col += 1;
            }
        }
    }
    let cell_at = |row: u32, col: u32| g.cell(row, col).map(|c| c.char);
    let sel = Selection::new(
        SelectionAnchor { row: 5, col: 10 },
        SelectionAnchor { row: 5, col: 14 },
        SelectionMode::Word,
    );
    c.bench_function("B53_selection_word_expand_1000", |b| {
        b.iter(|| {
            for _ in 0..1000 {
                let expanded = sel.expand_word(&cell_at);
                black_box(expanded.start.col);
            }
        });
    });
}

fn bench_selection_line(c: &mut Criterion) {
    use torvox_core::selection::{Selection, SelectionAnchor, SelectionMode};
    let mut g = Grid::new(24, 80);
    for row in 0..24 {
        g.fill_cells(row, 'A', 0, 80);
    }
    let sel = Selection::new(
        SelectionAnchor { row: 5, col: 0 },
        SelectionAnchor { row: 10, col: 79 },
        SelectionMode::Line,
    );
    c.bench_function("B54_selection_line_text_1000", |b| {
        b.iter(|| {
            for _ in 0..1000 {
                let text = sel.text(&g);
                black_box(text.len());
            }
        });
    });
}

fn bench_selection_text_extraction(c: &mut Criterion) {
    use torvox_core::selection::{Selection, SelectionAnchor, SelectionMode};
    let mut g = Grid::new(24, 80);
    for row in 0..24 {
        g.fill_cells(row, 'A', 0, 80);
    }
    let sel_char = Selection::new(
        SelectionAnchor { row: 3, col: 10 },
        SelectionAnchor { row: 20, col: 70 },
        SelectionMode::Char,
    );
    let sel_block = Selection::new(
        SelectionAnchor { row: 3, col: 10 },
        SelectionAnchor { row: 20, col: 70 },
        SelectionMode::Block,
    );
    c.bench_function("B55_selection_char_text_extract", |b| {
        b.iter(|| {
            let text = sel_char.text(&g);
            black_box(text.len());
        });
    });
    c.bench_function("B55_selection_block_text_extract", |b| {
        b.iter(|| {
            let text = sel_block.text(&g);
            black_box(text.len());
        });
    });
}

// ── B56-B57: SGR attribute parsing and applying ────────────────────
fn bench_sgr_parse_short(c: &mut Criterion) {
    use torvox_core::sgr::parse_sgr;
    c.bench_function("B56_sgr_parse_short_3_params", |b| {
        b.iter(|| {
            let attrs = parse_sgr(&[1, 31, 44]);
            black_box(attrs.len());
        });
    });
    c.bench_function("B56_sgr_parse_medium_6_params", |b| {
        b.iter(|| {
            let attrs = parse_sgr(&[1, 3, 4, 31, 44, 53]);
            black_box(attrs.len());
        });
    });
}

fn bench_sgr_parse_extended_color(c: &mut Criterion) {
    use torvox_core::sgr::parse_sgr;
    c.bench_function("B56_sgr_parse_256_color", |b| {
        b.iter(|| {
            let attrs = parse_sgr(&[38, 5, 196, 48, 5, 255]);
            black_box(attrs.len());
        });
    });
    c.bench_function("B56_sgr_parse_rgb_color", |b| {
        b.iter(|| {
            let attrs = parse_sgr(&[38, 2, 255, 128, 64, 48, 2, 64, 128, 255]);
            black_box(attrs.len());
        });
    });
}

fn bench_sgr_apply(c: &mut Criterion) {
    use torvox_core::cell::Cell;
    use torvox_core::sgr::{ColorSpec, SgrAttribute, UnderlineStyle, apply_sgr};
    c.bench_function("B57_sgr_apply_single_attr", |b| {
        b.iter_batched(
            || Cell::default(),
            |mut cell| {
                apply_sgr(&[SgrAttribute::Bold(true)], &mut cell);
                black_box(cell.attrs.bold);
            },
            BatchSize::SmallInput,
        );
    });
    c.bench_function("B57_sgr_apply_multiple_attrs", |b| {
        let attrs = [
            SgrAttribute::Bold(true),
            SgrAttribute::Italic(true),
            SgrAttribute::Underline(UnderlineStyle::Single),
            SgrAttribute::ForegroundColor(ColorSpec::Named(2)),
            SgrAttribute::BackgroundColor(ColorSpec::Indexed(235)),
            SgrAttribute::Reverse(true),
        ];
        b.iter_batched(
            || Cell::default(),
            |mut cell| {
                apply_sgr(&attrs, &mut cell);
                black_box(cell.attrs.bold);
            },
            BatchSize::SmallInput,
        );
    });
}

fn bench_prev_cells_clone(c: &mut Criterion) {
    let cols = 80;
    let rows = 24;
    let total = cols * rows;
    let cells: Vec<CellSnapshot> = vec![CellSnapshot::default(); total];

    let mut group = c.benchmark_group("B28_prev_cells_clone");
    group.throughput(Throughput::Elements(total as u64));

    // Full clone: Clone all 1920 cells via clone_from
    group.bench_function("full_clone_1920", |b| {
        let mut prev = cells.clone();
        b.iter(|| black_box(prev.clone_from(&cells)));
    });

    // Partial clone: 1 dirty row = 80 cells via clone_from_slice
    group.bench_function("partial_1row_dirty", |b| {
        let mut prev = cells.clone();
        let start = 3 * cols;
        let end = start + cols;
        b.iter(|| {
            prev[start..end].clone_from_slice(&cells[start..end]);
            black_box(&prev);
        });
    });

    // Partial clone: 10 dirty rows = 800 cells
    group.bench_function("partial_10rows_dirty", |b| {
        let mut prev = cells.clone();
        let rows_10: Vec<(usize, usize)> = (0..10).map(|r| (r * cols, r * cols + cols)).collect();
        b.iter(|| {
            for &(start, end) in &rows_10 {
                prev[start..end].clone_from_slice(&cells[start..end]);
            }
            black_box(&prev);
        });
    });

    group.finish();
}

criterion_group!(
    vt_benches,
    bench_device_create,
    bench_vt_parse_plain_text,
    bench_vt_parse_sgr_sequences,
    bench_vt_parse_cursor_movement,
    bench_vt_parse_large_output,
    bench_vt_throughput_100k_lines,
    bench_vt_throughput_ls_la,
    bench_input_to_pixel_latency,
    bench_csi_throughput,
    bench_csi_mixed_throughput,
    bench_osc_throughput,
    bench_osc_heavy_throughput,
    bench_alt_screen_switch,
    bench_color_cycling,
    bench_clipboard_ops,
    bench_ghostty_screenshot,
    bench_session_startup,
    bench_kitty_scrolling_heavy,
    bench_kitty_1024_glyphs,
);

criterion_group!(
    grid_benches,
    bench_grid_sizeof,
    bench_grid_cell_access,
    bench_grid_resize,
    bench_grid_resize_large_scrollback,
    bench_grid_scrollback,
    bench_scrollback_push_10k,
    bench_grid_fill,
    bench_grid_fill_comprehensive,
    bench_window_resize,
    bench_ghostty_resize,
    bench_grid_snapshot_alloc,
    bench_memory_baseline,
    bench_grid_alloc_stress,
    bench_leak_detection,
);

criterion_group!(
    other_benches,
    bench_selection_ops,
    bench_fuzz_throughput,
    bench_cpu_idle,
    bench_gpu_render_throughput,
    bench_cached_copy_vs_swap,
    bench_prev_cells_clone,
);

criterion_group!(
    cell_access_benches,
    bench_cell_random_access,
    bench_cell_sequential,
    bench_cell_column_by_column,
    bench_selection_word,
    bench_selection_line,
    bench_selection_text_extraction,
    bench_sgr_parse_short,
    bench_sgr_parse_extended_color,
    bench_sgr_apply,
);

criterion_main!(vt_benches, grid_benches, other_benches, cell_access_benches);
