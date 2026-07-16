//! AndroidSurface — render pipeline lifecycle for Android TextureView.
//!
//! # Requirements
//! - [FR-018](crate) — Surface: Android TextureView lifecycle
//! - [FR-024](crate) — Surface: resolution change handling
//! - [FR-052](crate) — Surface: SurfaceView → TextureView migration

use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::time::Instant;

use thiserror::Error;
use torvox_core::line::Line;
use torvox_core::snapshot::SessionSnapshot;
use torvox_renderer::font::FontPipeline;
use torvox_renderer::gpu::CellInstance;
use torvox_renderer::gpu::GpuContext;
use torvox_renderer::gpu::SearchHighlight;
use torvox_renderer::gpu::SelectionRange;
use torvox_terminal::ghostty_terminal::CellSnapshot;
use torvox_terminal::ghostty_terminal::KgpImageData;
use torvox_terminal::session::Session;
use torvox_terminal::shell_env::ShellEnv;

#[cfg(target_os = "android")]
const WINDOW_FORMAT_RGBA_8888: i32 = 1;
const FONT_SIZE_FALLBACK_MULTIPLIER: f32 = 1.6;
const MIN_FONT_SIZE: f32 = 24.0;
const MIN_COLS: f32 = 20.0;
const MAX_COLS: f32 = 300.0;
const MIN_ROWS: f32 = 5.0;
const MAX_ROWS: f32 = 200.0;
const FRAME_TIME_TARGET_MS: f64 = 16.0;
const SYNC_MODE_NUMBER: u16 = 2026;
const DEFAULT_MAX_SCROLLBACK: usize = 2000;
const ATLAS_WIDTH: u32 = 2048;
const ATLAS_HEIGHT: u32 = 2048;
const KGP_ATLAS_WIDTH: u32 = 2048;
const MAX_SURFACE_DIMENSION: u16 = 4096;

// SAFETY: These are FFI function declarations from the Android NDK. They are safe
// to declare — the unsafety is in calling them, which is already annotated at each
// call site. The signatures match the NDK header definitions for ANativeWindow and
// ATrace APIs. Each call site documents its own safety invariants.
#[cfg(target_os = "android")]
#[link(name = "android")]
unsafe extern "C" {
    fn ANativeWindow_release(window: *mut std::ffi::c_void);
    fn ANativeWindow_setBuffersGeometry(
        window: *mut std::ffi::c_void,
        width: i32,
        height: i32,
        format: i32,
    ) -> i32;
    fn ATrace_beginSection(section_name: *const std::os::raw::c_char);
    fn ATrace_endSection();
    fn ATrace_isEnabled() -> bool;
}

/// Call ATrace_beginSection only when systrace is actively recording.
/// On some vendor kernels (e.g. ZTE Mali), unconditional trace_marker
/// writes add significant per-frame overhead even when no trace session
/// is active, causing jank and RENDER_SLOW warnings.
#[cfg(target_os = "android")]
fn trace_begin(name: &core::ffi::CStr) {
    // SAFETY: C string is a static literal; ATrace_isEnabled/beginSection
    // are thread-safe NDK functions.
    unsafe {
        if ATrace_isEnabled() {
            ATrace_beginSection(name.as_ptr());
        }
    }
}

/// Call ATrace_endSection only when systrace was active (paired with
/// a preceding trace_begin that actually wrote to the marker).
#[cfg(target_os = "android")]
fn trace_end() {
    // SAFETY: ATrace_isEnabled/endSection are thread-safe NDK functions.
    unsafe {
        if ATrace_isEnabled() {
            ATrace_endSection();
        }
    }
}

#[cfg(not(target_os = "android"))]
fn trace_begin(_name: &core::ffi::CStr) {}
#[cfg(not(target_os = "android"))]
fn trace_end() {}

#[derive(Debug, Error)]
pub enum SurfaceError {
    #[error("GPU initialization failed: {0}")]
    GpuInit(String),
    #[error("Surface creation failed: {0}")]
    SurfaceCreation(String),
    #[error("No surface available")]
    NoSurface,
    #[error("No active session")]
    NoSession,
    #[error("Render error: {0}")]
    Render(String),
    #[error("session error: {0}")]
    Session(String),
}

impl From<torvox_renderer::gpu::GpuError> for SurfaceError {
    fn from(e: torvox_renderer::gpu::GpuError) -> Self {
        SurfaceError::Render(e.to_string())
    }
}

/// Wrap `NonNull<c_void>` for `Send` (NOT `Sync`).
///
/// ANativeWindow pointer is moved between threads (e.g., from the bridge thread
/// to the render thread via `set_native_window`/`update_native_window`), so `Send`
/// is necessary. However, ANativeWindow operations are NOT thread-safe per the NDK
/// documentation — concurrent access from multiple threads produces undefined behavior.
/// Therefore `Sync` is intentionally omitted: a `&NativeWindow` must NEVER be shared
/// across threads. All ANativeWindow access is serialized through AndroidSurface's
/// internal `Mutex` lock discipline, enforced by the absence of `Sync`.
struct NativeWindow(std::ptr::NonNull<std::ffi::c_void>);
// SAFETY: ANativeWindow_fromSurface returns a pointer to an ANativeWindow with
// reference count 1. The pointer is valid from creation (set_native_window) until
// explicit release (release_surface/release_gpu_surface). NativeWindow is Send
// because it moves between threads (via self.native_window = Some(NativeWindow(...)))
// but is NOT Sync because ANativeWindow operations are not thread-safe. The absence
// of Sync is the compiler-enforced guarantee that &NativeWindow is never shared
// across threads — all access goes through AndroidSurface's Mutex lock discipline.
unsafe impl Send for NativeWindow {}

pub struct AndroidSurface {
    gpu: Option<GpuContext>,
    font_pipeline: FontPipeline,
    session: Option<Arc<Mutex<Session>>>,
    scrollback_lines: u32,
    atlas_width: u32,
    atlas_height: u32,
    theme: torvox_core::config::Theme,
    rows: u32,
    cols: u32,
    exited: Arc<AtomicBool>,
    save_path: Option<PathBuf>,
    mouse_row: Option<u32>,
    mouse_col: Option<u32>,
    last_hovered_url: Option<String>,
    surface_width: AtomicU32,
    surface_height: AtomicU32,
    render_width: u32,
    render_height: u32,
    /// Last derived raster scale, kept to log changes only (fix D).
    last_raster_scale: f32,
    native_window: Option<NativeWindow>,
    frame_count: u64,
    title: String,
    selection: Option<SelectionRange>,
    search_highlights: Vec<SearchHighlight>,
    last_cursor_row: u32,
    last_cursor_col: u32,
    cursor_style: torvox_core::cursor::CursorStyle,
    blink_phase: bool,
    blink_enabled: bool,
    blink_speed_ms: u32,
    last_blink_toggle: std::time::Instant,
    render_requested: bool,
    /// Persistent instance buffer reused across frames so the per-frame cell
    /// instance `Vec` is not reallocated on every render (see
    /// `build_cell_instances_into`). `CellInstance` is `Copy`/`Pod`, so reuse
    /// via `clear` + `reserve` is safe.
    instance_buffer: Vec<torvox_renderer::gpu::CellInstance>,
    /// Cached CellSnapshot cells from the previous frame for cell-level
    /// dirty tracking. When empty (first frame), all rows are dirty.
    prev_cells: Vec<CellSnapshot>,
    /// Cached CellInstances from the previous frame. Clean rows are copied
    /// from this cache instead of rebuilding them through the shaping/color
    /// atlas-lookup hot path.
    cached_instances: Vec<torvox_renderer::gpu::CellInstance>,
    /// Cumulative end offset per row in `cached_instances`.
    /// `cached_row_ends[row]` = exclusive end index of row `row`.
    cached_row_ends: Vec<usize>,
    /// Reused scratch buffer for per-frame dirty-row tracking. Avoids a
    /// `Vec<bool>` allocation on every render (mirrors `instance_buffer`
    /// reuse for the zero-alloc hot path).
    dirty_rows_buf: Vec<bool>,
    /// Reused scratch buffer for per-frame row-end offsets.
    row_ends_buf: Vec<usize>,
    /// Scroll offset used in the previous frame. When it changes, all rows
    /// are marked dirty because the grid cells shift.
    prev_scroll_offset: u32,
    /// Render height from the previous frame. When it changes (e.g. IME
    /// opens/closes), all rows are marked dirty because the cached instances
    /// were built for a different projection_height, and clean rows beyond
    /// the new projection_height would otherwise be incorrectly reused.
    prev_render_height: u32,
}

/// Compare two CellSnapshots for equality of fields that affect rendering.
/// Returns `true` if they differ (row should be marked dirty).
#[allow(clippy::float_cmp)]
fn cell_ne(a: &CellSnapshot, b: &CellSnapshot) -> bool {
    a.codepoint != b.codepoint
        || a.width != b.width
        || a.bold != b.bold
        || a.dim != b.dim
        || a.italic != b.italic
        || a.underline != b.underline
        || a.double_underline != b.double_underline
        || a.reverse != b.reverse
        || a.strikethrough != b.strikethrough
        || a.overline != b.overline
        || a.hidden != b.hidden
        || a.foreground != b.foreground
        || a.background != b.background
        || a.uri != b.uri
        || a.graphemes.len() != b.graphemes.len()
        || (!a.graphemes.is_empty() && a.graphemes != b.graphemes)
}

fn cell_to_line(cells: &[CellSnapshot], cols: u32) -> Line {
    let mut line = Line::new(cols);
    for col in 0..cols as usize {
        if let Some(cs) = cells.get(col)
            && let Some(cell) = line.get_mut(col as u32)
        {
            cell.char = char::from_u32(cs.codepoint).unwrap_or(' ');
            cell.foreground = torvox_core::cell::Color {
                r: (cs.foreground[0] * 255.0) as u8,
                g: (cs.foreground[1] * 255.0) as u8,
                b: (cs.foreground[2] * 255.0) as u8,
                a: (cs.foreground[3] * 255.0) as u8,
            };
            cell.background = torvox_core::cell::Color {
                r: (cs.background[0] * 255.0) as u8,
                g: (cs.background[1] * 255.0) as u8,
                b: (cs.background[2] * 255.0) as u8,
                a: (cs.background[3] * 255.0) as u8,
            };
            cell.attrs.bold = cs.bold;
            cell.attrs.italic = cs.italic;
            cell.attrs.underline = cs.underline;
            cell.attrs.reverse = cs.reverse;
        }
    }
    line
}

fn line_to_text(line: &Line) -> String {
    (0..line.len())
        .filter_map(|c| line.get(c))
        .map(|cell| cell.char)
        .collect()
}

/// Requirement 4 (CJK font scale, Fix D): the glyph raster scale is the ratio of
/// the physical surface width (ANativeWindow pixels) to the wgpu surface-config
/// width (logical density). It is clamped to a sane range so a misreported
/// surface metric cannot blow up the atlas. Pure + testable.
fn compute_raster_scale(surface_width: u32, config_width: u32) -> f32 {
    let surface_width = surface_width.max(1);
    let config_width = config_width.max(1);
    (surface_width as f32 / config_width as f32).clamp(0.5, 4.0)
}

impl AndroidSurface {
    pub fn new(rows: u32, cols: u32, scrollback_lines: u32, font_size: f32) -> Self {
        let atlas_width = ATLAS_WIDTH;
        let atlas_height = ATLAS_HEIGHT;
        let font_pipeline = FontPipeline::new(atlas_width as i32, atlas_height as i32, font_size);

        let gpu = Some(GpuContext::new_with_no_surface());
        Self {
            gpu,
            font_pipeline,
            session: None,
            scrollback_lines,
            atlas_width,
            atlas_height,
            theme: torvox_core::config::Theme::catppuccin_mocha(),
            rows,
            cols,
            exited: Arc::new(AtomicBool::new(false)),
            save_path: None,
            mouse_row: None,
            mouse_col: None,
            last_hovered_url: None,
            surface_width: AtomicU32::new(0),
            surface_height: AtomicU32::new(0),
            render_width: 0,
            render_height: 0,
            last_raster_scale: 0.0,
            native_window: None,
            frame_count: 0,
            title: String::new(),
            selection: None,
            search_highlights: Vec::new(),
            last_cursor_row: 0,
            last_cursor_col: 0,
            blink_phase: true,
            blink_enabled: true,
            blink_speed_ms: 530,
            last_blink_toggle: std::time::Instant::now(),
            cursor_style: torvox_core::cursor::CursorStyle::Block,
            render_requested: false,
            instance_buffer: Vec::new(),
            prev_cells: Vec::new(),
            cached_instances: Vec::new(),
            cached_row_ends: Vec::new(),
            dirty_rows_buf: Vec::new(),
            row_ends_buf: Vec::new(),
            prev_scroll_offset: 0,
            prev_render_height: 0,
        }
    }

    pub fn set_surface_size(&mut self, width: u32, height: u32) {
        self.surface_width.store(width.max(1), Ordering::Relaxed);
        self.surface_height.store(height.max(1), Ordering::Relaxed);
    }

    pub fn set_save_path(&mut self, path: String) {
        self.save_path = Some(PathBuf::from(path));
    }

    pub fn spawn_session(
        &mut self,
        shell: &str,
        env: &ShellEnv,
    ) -> Result<Arc<Mutex<Session>>, SurfaceError> {
        let (background, foreground) = (self.theme.background, self.theme.foreground);
        let ansi = self.theme.ansi;
        let session = Session::spawn_with_theme(
            shell,
            self.rows,
            self.cols,
            env,
            background,
            foreground,
            ansi,
            self.scrollback_lines,
        )
        .map_err(|e| SurfaceError::Session(e.to_string()))?;
        let session_arc = Arc::new(Mutex::new(session));
        {
            let mut guard = match session_arc.lock() {
                Ok(g) => g,
                Err(poisoned) => {
                    log::error!("spawn_session: session mutex poisoned, recovering");
                    poisoned.into_inner()
                }
            };
            self.exited = guard.exited_flag().clone();
            guard.set_pixel_size(
                (self.surface_width.load(Ordering::Relaxed) as u16).min(MAX_SURFACE_DIMENSION),
                (self.surface_height.load(Ordering::Relaxed) as u16).min(MAX_SURFACE_DIMENSION),
            );
        }
        self.session = Some(session_arc.clone());

        Ok(session_arc)
    }
    pub fn update_native_window(
        &mut self,
        window_ptr: *mut std::ffi::c_void,
        width: u32,
        height: u32,
    ) -> Result<(), SurfaceError> {
        let pointer_changed = self
            .native_window
            .as_ref()
            .map(|nw| nw.0.as_ptr() != window_ptr)
            .unwrap_or(true);
        log::info!(
            "UPDATE_NATIVE_WINDOW: ptr={:#x} {}x{} pointer_changed={}",
            window_ptr as usize,
            width,
            height,
            pointer_changed,
        );
        self.native_window = std::ptr::NonNull::new(window_ptr).map(NativeWindow);
        self.surface_width.store(width, Ordering::Relaxed);
        self.surface_height.store(height, Ordering::Relaxed);
        self.render_width = width;
        self.render_height = height;
        // Set ANativeWindow buffer format for wgpu swapchain.
        // Must use RGBA_8888 (format=1).
        #[cfg(target_os = "android")]
        if let Some(nw) = self.native_window.as_ref() {
            // SAFETY: nw.0.as_ptr() is a valid NonNull pointer obtained from
            // ANativeWindow_fromSurface (via set_native_window). The window
            // is still alive because NativeWindow access is serialized through
            // AndroidSurface's lock discipline. width/height are reasonable
            // surface dimensions, and WINDOW_FORMAT_RGBA_8888 is a valid
            // Android native window format constant.
            if unsafe {
                ANativeWindow_setBuffersGeometry(
                    nw.0.as_ptr(),
                    width as i32,
                    height as i32,
                    WINDOW_FORMAT_RGBA_8888,
                )
            } != 0
            {
                log::error!("ANativeWindow_setBuffersGeometry failed");
            }
        }
        // Reconfigure or recreate the wgpu surface.
        // When the ANativeWindow changes (surface destroy/recreate), we must
        // recreate the wgpu Surface from the new window pointer. A plain
        // reconfigure on the old (stale) surface produces no visible output.
        // Also recreate when has_surface() is false (release_gpu_surface was called
        // to release the ANativeWindow for another bridge to use).
        #[cfg(target_os = "android")]
        if let Some(gpu) = &mut self.gpu {
            if pointer_changed || !gpu.has_surface() {
                // When the ANativeWindow has changed (foreground/background transition),
                // the globally cached surface belongs to the old window and must be
                // discarded before creating a new one on the new window.
                if pointer_changed {
                    GpuContext::clear_global_surface();
                }
                gpu.configure_android_surface(window_ptr, width.max(1), height.max(1))
                    .map_err(|e| SurfaceError::GpuInit(e.to_string()))?;
            } else {
                gpu.reconfigure_swapchain(width.max(1), height.max(1));
            }
            // Re-apply the theme background after (re)configuring the swapchain:
            // a session switch reconfigures the surface but must keep its own
            // background color instead of the default deep-blue clear color.
            gpu.set_bg_color(self.theme.background);
        }
        // Desktop: always recreate the surface from the new native window pointer.
        #[cfg(not(target_os = "android"))]
        if let Some(gpu) = &mut self.gpu {
            let ptr = window_ptr;
            gpu.configure_android_surface(ptr, width.max(1), height.max(1))
                .map_err(|e| SurfaceError::GpuInit(e.to_string()))?;
        }
        // Do NOT call recompute_grid here — update_native_window is called during
        // IME show/hide animation where the height changes frame-by-frame, and
        // recomputing the grid on every intermediate frame causes visible text
        // stretch/squash as the row count oscillates. Grid is recomputed once
        // during set_native_window (initial setup) and when the font size changes.
        self.render_requested = true;
        Ok(())
    }

    pub fn set_native_window(
        &mut self,
        window_ptr: *mut std::ffi::c_void,
        width: u32,
        height: u32,
        font_size_tenths: u32,
    ) -> Result<(), SurfaceError> {
        self.native_window = std::ptr::NonNull::new(window_ptr).map(NativeWindow);
        self.surface_width.store(width, Ordering::Relaxed);
        self.surface_height.store(height, Ordering::Relaxed);
        self.render_width = width;
        self.render_height = height;

        // Use configured font size, fall back to geometry-based calc
        let font_size = if font_size_tenths > 0 {
            font_size_tenths as f32 / 10.0
        } else {
            ((width as f32 / self.cols as f32) * FONT_SIZE_FALLBACK_MULTIPLIER).max(MIN_FONT_SIZE)
        };

        log::info!(
            "SET_FONT_SIZE: font_size_tenths={} font_size={} self.rows={} self.cols={}",
            font_size_tenths,
            font_size,
            self.rows,
            self.cols,
        );

        self.font_pipeline =
            FontPipeline::new(self.atlas_width as i32, self.atlas_height as i32, font_size);
        self.font_pipeline.rasterize_ascii();
        let (_aw, _ah) = self.font_pipeline.atlas_dimensions();
        let (cw, ch) = self.font_pipeline.cell_metrics();

        log::info!(
            "CELL_METRICS: cw={} ch={} (sw={} sh={}) recomputed_cols={} recomputed_rows={}",
            cw,
            ch,
            width,
            height,
            (width as f32 / cw).floor(),
            (height as f32 / ch).floor().clamp(5.0, 200.0),
        );

        // Compute grid dimensions to fill the surface
        let cols = (width as f32 / cw).floor().clamp(MIN_COLS, MAX_COLS) as u32;
        let rows = (height as f32 / ch).floor().clamp(MIN_ROWS, MAX_ROWS) as u32;
        self.rows = rows;
        self.cols = cols;

        // Configure ANativeWindow buffer geometry for the wgpu swapchain.
        // Must use RGBA_8888 (format=1) — wgpu requires Android hardware
        // buffer format RGBA_8888 for the swapchain. AHARDWAREBUFFER
        // formats (format=2+) are not compatible with wgpu.
        #[cfg(target_os = "android")]
        if let Some(nw) = self.native_window.as_ref() {
            // SAFETY: nw.0.as_ptr() is a valid NonNull pointer obtained from
            // ANativeWindow_fromSurface. The window is still alive because
            // NativeWindow access is serialized through AndroidSurface's
            // lock discipline. width/height are surface dimensions,
            // WINDOW_FORMAT_RGBA_8888 is a valid format constant.
            if unsafe {
                ANativeWindow_setBuffersGeometry(
                    nw.0.as_ptr(),
                    width as i32,
                    height as i32,
                    WINDOW_FORMAT_RGBA_8888,
                )
            } != 0
            {
                log::error!("ANativeWindow_setBuffersGeometry failed");
            }
        }

        log::info!(
            "SURFACE_SET_NATIVE_WINDOW: grid={}x{} surface={}x{}",
            rows,
            cols,
            width,
            height,
        );

        let needs_init = self
            .gpu
            .as_ref()
            .is_none_or(|g| !g.has_pipeline() || !g.has_surface());

        if needs_init {
            let (aw, ah) = self.font_pipeline.atlas_dimensions();
            let (cw, ch) = self.font_pipeline.cell_metrics();
            log::info!(
                "SURFACE_CELL_METRICS: font_size={} cell={:.1}x{:.1} grid={}x{} surface={}x{} atlas={}x{}",
                font_size,
                cw,
                ch,
                rows,
                cols,
                width,
                height,
                aw,
                ah,
            );

            #[cfg(not(target_os = "android"))]
            {
                let gpu = self
                    .gpu
                    .as_mut()
                    .ok_or_else(|| SurfaceError::GpuInit("GPU not initialized".into()))?;
                gpu.set_surface_from_native_window(window_ptr, width, height, true)
                    .map_err(|e| SurfaceError::GpuInit(e.to_string()))?;
                gpu.create_atlas_texture(aw, ah);
                gpu.upload_atlas(self.font_pipeline.atlas_bitmap(), aw, ah);
                gpu.update_bind_group(
                    aw as f32,
                    ah as f32,
                    self.render_width as f32,
                    self.render_height as f32,
                );
            }

            // Android: no-surface path uses device-only pipeline.
            // Swapchain path creates wgpu Surface from ANativeWindow
            // for Vulkan/GLES presentation.
            #[cfg(target_os = "android")]
            {
                let gpu = self
                    .gpu
                    .as_mut()
                    .ok_or_else(|| SurfaceError::GpuInit("GPU not initialized".into()))?;
                if !window_ptr.is_null() {
                    // Create swapchain surface for GPU presentation
                    gpu.configure_android_surface(window_ptr, width, height)
                        .map_err(|e| SurfaceError::GpuInit(e.to_string()))?;
                }
                gpu.initialize_pipeline_and_bind_group(aw, ah, width, height);
                gpu.upload_atlas(self.font_pipeline.atlas_bitmap(), aw, ah);
                // The clear color defaults to the deep mocha blue; re-apply the
                // per-session theme background so a freshly created surface (or a
                // surface re-bound to a different session) never shows the wrong
                // default background.
                gpu.set_bg_color(self.theme.background);
            }
        }

        Ok(())
    }

    fn blink_period(&self) -> std::time::Duration {
        std::time::Duration::from_millis(self.blink_speed_ms as u64)
    }

    fn blink_timer_elapsed(&self) -> bool {
        self.blink_enabled && self.last_blink_toggle.elapsed() >= self.blink_period()
    }

    /// Returns `true` if new PTY data arrived and was rendered, `false` if idle.
    /// GPU-only render path.  Called from bridge after `process_session_for_render`
    /// has already processed ghostty output and taken a snapshot while the surface
    /// lock was NOT held.  The surface lock is held during this call but only for
    /// fast GPU operations (atlas upload, instance building, wgpu render pass).
    pub fn render_frame(
        &mut self,
        scroll_offset: u32,
        had_output: bool,
        snapshot: torvox_terminal::ghostty_terminal::GridSnapshot,
    ) -> Result<bool, SurfaceError> {
        self.render_inner(scroll_offset, had_output, snapshot)
    }

    /// Full render path: process ghostty output, take snapshot, then run GPU work.
    /// Used by tests and as a fallback when the bridge path is not available.
    pub fn render(&mut self, scroll_offset: u32) -> Result<bool, SurfaceError> {
        let had_output;
        let snapshot;
        {
            let mut guard = self
                .session
                .as_ref()
                .ok_or(SurfaceError::NoSession)?
                .lock()
                .map_err(|_| SurfaceError::NoSession)?;
            let session = &mut *guard;
            had_output = session.process_output();
            snapshot = match session
                .terminal()
                .try_take_snapshot_with_scroll(scroll_offset)
            {
                Some(snap) => snap,
                None => return Ok(false),
            };
        }
        self.render_inner(scroll_offset, had_output, snapshot)
    }

    fn render_inner(
        &mut self,
        scroll_offset: u32,
        had_output: bool,
        mut snapshot: torvox_terminal::ghostty_terminal::GridSnapshot,
    ) -> Result<bool, SurfaceError> {
        let frame_start = Instant::now();
        trace_begin(c"AndroidSurface::render");
        log::trace!(
            "RENDER_ENTER: session={} sw={} sh={} native={}",
            self.session.is_some(),
            self.surface_width.load(Ordering::Relaxed),
            self.surface_height.load(Ordering::Relaxed),
            self.native_window.is_some(),
        );

        let has_search_highlights = !self.search_highlights.is_empty();
        // Skip expensive snapshot + GPU render when nothing changed.
        // Render even without PTY output when search highlights are pending
        // so the user sees the highlighted matches immediately.
        if has_search_highlights {
            log::info!(
                "render: search highlights pending, proceeding (count={})",
                self.search_highlights.len()
            );
        }
        if !had_output
            && self.frame_count > 0
            && !has_search_highlights
            && !self.render_requested
            && !self.blink_timer_elapsed()
        {
            trace_end(); // AndroidSurface::render
            return Ok(false);
        }
        log::debug!(
            "RENDER_PROCEED: had_output={} frame_count={} highlights={} render_requested={}",
            had_output,
            self.frame_count,
            has_search_highlights,
            self.render_requested,
        );

        // Cursor visibility follows the terminal's DECTCEM state directly.
        // No app-level override — cursor_override was removed (FR-057).
        // Cursor blink is handled below as a phase toggle on snapshot visibility.

        trace_begin(c"snapshot+instances");

        if let (Some(row), Some(col)) = (self.mouse_row, self.mouse_col) {
            self.last_hovered_url = snapshot.uri_at(row, col).map(String::from);
        }

        // Always rasterize glyphs so the atlas is populated for GPU rendering.
        // Sync mode is read from the snapshot (set by ghostty thread during
        // snapshot build) to avoid a blocking mode_get query on the render path.
        #[cfg(target_os = "android")]
        let sync_active = snapshot.sync_active;
        let gen_before = self.font_pipeline.atlas_generation();
        let tc = self.theme.cursor;
        let cursor_color = Some([
            tc[0] as f32 / 255.0,
            tc[1] as f32 / 255.0,
            tc[2] as f32 / 255.0,
            1.0,
        ]);
        let selection_bg = self.theme.selection_bg;
        let selection_bg = Some([
            selection_bg[0] as f32 / 255.0,
            selection_bg[1] as f32 / 255.0,
            selection_bg[2] as f32 / 255.0,
            1.0,
        ]);
        // ── COMPUTE DIRTY ROWS ──
        // Diff current vs previous snapshot cells to find changed rows.
        let total_cells = (snapshot.rows * snapshot.cols) as usize;
        let mut dirty_rows = std::mem::take(&mut self.dirty_rows_buf);
        dirty_rows.clear();
        if self.prev_cells.len() == total_cells {
            let cap = snapshot.rows as usize;
            dirty_rows.reserve(cap);
            for row in 0..snapshot.rows as usize {
                let row_off = row * snapshot.cols as usize;
                let mut row_dirty = false;
                for col in 0..snapshot.cols as usize {
                    if cell_ne(
                        &self.prev_cells[row_off + col],
                        &snapshot.cells[row_off + col],
                    ) {
                        row_dirty = true;
                        break;
                    }
                }
                dirty_rows.push(row_dirty);
            }
        } else {
            dirty_rows.resize(snapshot.rows as usize, true);
        }

        // Force-dirty cursor row unconditionally so blink phase changes and
        // terminal DECTCEM state changes are always reflected on screen.
        let cr = snapshot.cursor_row as usize;
        if cr < dirty_rows.len() {
            dirty_rows[cr] = true;
        }
        let pcr = self.last_cursor_row as usize;
        if pcr < dirty_rows.len() && pcr != cr {
            dirty_rows[pcr] = true;
        }
        // Reset blink phase when cursor moves (keyboard input→snapshot col change)
        if snapshot.cursor_col != self.last_cursor_col {
            self.blink_phase = true;
            self.last_blink_toggle = std::time::Instant::now();
        }

        // Conservative: when scroll or search highlights change, mark all
        // rows dirty. Selection changes trigger render_requested on the
        // Kotlin side, so the next frame will have all-dirty anyway.
        let scroll_changed = self.prev_scroll_offset != scroll_offset;
        // When render_height changes (e.g. IME opens/closes), the
        // projection_height used in build_cell_instances_into changes.
        // Force all rows dirty so cached instances are rebuilt for the
        // new projection_height.
        let render_height_changed = self.render_height != self.prev_render_height;
        let highlights_present = !self.search_highlights.is_empty();
        if highlights_present || scroll_changed || render_height_changed {
            dirty_rows.fill(true);
        }

        // Cursor blink phase toggle at the configured interval.
        // blink_timer_elapsed() also gates the early-return above so idle
        // terminals still re-render when blink phase changes.
        let now = std::time::Instant::now();
        if self.blink_timer_elapsed() {
            self.blink_phase = !self.blink_phase;
            self.last_blink_toggle = now;
        }

        // Hide cursor during blink-off phase (applied on top of DECTCEM state).
        // When blink is disabled, blink_phase stays true and the cursor
        // follows DECTCEM visibility directly.
        if self.blink_enabled && !self.blink_phase && snapshot.cursor_visible {
            snapshot.cursor_visible = false;
        }

        // Partial clone: only copy rows that changed (dirty).
        // Full clone is 1920 cells × Vec<String> → heap allocations.
        // Partial: only N dirty rows × 80 cells per row.
        if self.prev_cells.len() == total_cells && dirty_rows.iter().any(|&d| !d) {
            let cols = snapshot.cols as usize;
            for (row, is_dirty) in dirty_rows.iter().enumerate() {
                if *is_dirty {
                    let start = row * cols;
                    let end = start + cols;
                    self.prev_cells[start..end].clone_from_slice(&snapshot.cells[start..end]);
                }
            }
        } else {
            self.prev_cells.clone_from(&snapshot.cells);
        }

        let mut row_ends = std::mem::take(&mut self.row_ends_buf);
        row_ends.clear();
        // Fix D: rasterize glyphs at font_size * raster_scale so the atlas
        // matches the physical surface 1:1. The device pixel ratio is the ratio
        // of the physical surface size (ANativeWindow) to the wgpu surface-config
        // size (which the renderer configures at logical density). This is
        // derived purely from surface metrics — never hardcoded — and equals the
        // factor the compositor upscales the rendered buffer by.
        let surf_w = self.surface_width.load(Ordering::Relaxed).max(1);
        let cfg_w = self
            .gpu
            .as_ref()
            .and_then(|g| g.surface_config.as_ref())
            .map(|cfg| cfg.width.max(1))
            .unwrap_or(1);
        let raster_scale = compute_raster_scale(surf_w, cfg_w);
        if (self.last_raster_scale - raster_scale).abs() > f32::EPSILON {
            self.last_raster_scale = raster_scale;
            log::debug!(
                "RASTER_SCALE: scale={} (surface_w={} config_w={})",
                raster_scale,
                surf_w,
                cfg_w
            );
        }
        self.font_pipeline.set_raster_scale(raster_scale);
        if let Some(gpu) = self.gpu.as_mut() {
            gpu.set_raster_scale(raster_scale);
        }
        torvox_renderer::gpu::build_cell_instances_into(
            &snapshot,
            &mut self.font_pipeline,
            torvox_renderer::gpu::CellInstanceConfig {
                atlas_width: self.atlas_width as f32,
                atlas_height: self.atlas_height as f32,
                projection_height: self.render_height as f32,
                selection: self.selection,
                selection_bg,
                search_highlights: &self.search_highlights,
                cursor_color,
                cursor_style: self.cursor_style,
                surface_bg: [
                    self.theme.background[0] as f32 / 255.0,
                    self.theme.background[1] as f32 / 255.0,
                    self.theme.background[2] as f32 / 255.0,
                    1.0,
                ],
                render_scale: torvox_renderer::gpu::RENDER_SCALE,
                dirty_rows: &dirty_rows,
                cached_instances: &self.cached_instances,
                cached_row_ends: &self.cached_row_ends,
            },
            &mut self.instance_buffer,
            &mut row_ends,
        );
        // Return the dirty-row scratch buffer for reuse next frame.
        self.dirty_rows_buf = dirty_rows;

        let instances = &self.instance_buffer[..];

        // DIAG: log first non-space cell's foreground color and instance count
        let diag_has_glyph = |i: &CellInstance| i.atlas_size[0] > 0.0 && i.atlas_size[1] > 0.0;
        let diag_first = instances.iter().find(|i| {
            i.fg_color[0].abs() > 0.001
                || i.fg_color[1].abs() > 0.001
                || i.fg_color[2].abs() > 0.001
        });
        if let Some(diag) = diag_first {
            if self.frame_count.is_multiple_of(60) {
                log::info!(
                    "RENDER_DIAG: instances={} fg=[{:.3},{:.3},{:.3}] bg=[{:.3},{:.3},{:.3}] has_glyph={} quad_size=[{:.1},{:.1}]",
                    instances.len(),
                    diag.fg_color[0],
                    diag.fg_color[1],
                    diag.fg_color[2],
                    diag.bg_color[0],
                    diag.bg_color[1],
                    diag.bg_color[2],
                    diag_has_glyph(diag),
                    diag.quad_size[0],
                    diag.quad_size[1],
                );
            }
        } else if !instances.is_empty() && self.frame_count.is_multiple_of(60) {
            log::info!(
                "RENDER_DIAG: {} instances all have black fg",
                instances.len()
            );
            log::info!(
                "RENDER_DIAG: first instance fg=[{:.3},{:.3},{:.3}] bg=[{:.3},{:.3},{:.3}] has_glyph={} quad_size=[{:.1},{:.1}]",
                instances[0].fg_color[0],
                instances[0].fg_color[1],
                instances[0].fg_color[2],
                instances[0].bg_color[0],
                instances[0].bg_color[1],
                instances[0].bg_color[2],
                diag_has_glyph(&instances[0]),
                instances[0].quad_size[0],
                instances[0].quad_size[1],
            );
        } else if instances.is_empty() && self.frame_count.is_multiple_of(60) {
            log::info!("RENDER_DIAG: zero instances");
        }

        let gen_after = self.font_pipeline.atlas_generation();
        if self.frame_count.is_multiple_of(60) {
            log::info!(
                "RENDER_DIAG: atlas gen {}->{}, {} instances",
                gen_before,
                gen_after,
                instances.len(),
            );
        }
        if gen_after != gen_before {
            self.gpu
                .as_mut()
                .ok_or_else(|| {
                    SurfaceError::GpuInit("GPU not initialized during atlas upload".into())
                })?
                .upload_atlas(
                    self.font_pipeline.atlas_bitmap(),
                    self.atlas_width,
                    self.atlas_height,
                );
        }

        // KGP image rendering: gather images referenced by placements into a shared RGBA atlas
        let mut kgp_instances = Vec::new();
        if !snapshot.kgp_placements.is_empty() {
            let (cell_w, cell_h) = self.font_pipeline.cell_metrics();
            let mut atlas_pixels: Vec<u8> = Vec::new();
            let atlas_w = KGP_ATLAS_WIDTH;
            let mut atlas_h = 0u32;
            let mut image_ids: HashSet<u32> = HashSet::new();
            for p in &snapshot.kgp_placements {
                image_ids.insert(p.image_id);
            }
            let kgp_data: Vec<KgpImageData> = {
                let guard = self
                    .session
                    .as_ref()
                    .ok_or(SurfaceError::NoSession)?
                    .lock()
                    .map_err(|_| SurfaceError::NoSession)?;
                let session = &*guard;
                let mut data = Vec::with_capacity(image_ids.len());
                for id in &image_ids {
                    if let Some(img) = session.terminal().take_kgp_image(*id) {
                        data.push(img);
                    }
                }
                data
            };
            for img in &kgp_data {
                atlas_h += img.height;
            }
            atlas_pixels.reserve(atlas_w as usize * atlas_h as usize * 4);
            let mut offset_y = 0u32;
            for img in &kgp_data {
                for row in 0..img.height {
                    let start = (row * img.width * 4) as usize;
                    let end = ((row + 1) * img.width * 4) as usize;
                    atlas_pixels.extend_from_slice(&img.data[start..end]);
                    atlas_pixels
                        .resize(atlas_pixels.len() + ((atlas_w - img.width) * 4) as usize, 0);
                }
                for p in &snapshot.kgp_placements {
                    if p.image_id == img.id {
                        let inst = torvox_renderer::gpu::KgpInstance::new(
                            [p.col as f32 * cell_w, p.row as f32 * cell_h],
                            [img.width as f32, img.height as f32],
                            [0.0, offset_y as f32 / atlas_h as f32],
                            [
                                img.width as f32 / atlas_w as f32,
                                img.height as f32 / atlas_h as f32,
                            ],
                            1.0,
                        );
                        kgp_instances.push(inst);
                    }
                }
                offset_y += img.height;
            }
            if let Some(gpu) = self.gpu.as_mut() {
                gpu.set_kgp_atlas(&atlas_pixels, atlas_w, atlas_h);
            }
        }

        trace_end(); // snapshot+instances
        trace_begin(c"swapchain_present");

        if !instances.is_empty() {
            let first = &instances[0];
            log::debug!(
                "RENDER_INSTANCES: count={} first_cell=({:.0},{:.0}) bg=({},{},{}) fg=({},{},{}) flags={:.0} uv_size=({:.4},{:.4}) bearing=({:.1},{:.1}) advance_width={:.1}",
                instances.len(),
                first.quad_origin[0],
                first.quad_origin[1],
                (first.bg_color[0] * 255.0) as u8,
                (first.bg_color[1] * 255.0) as u8,
                (first.bg_color[2] * 255.0) as u8,
                (first.fg_color[0] * 255.0) as u8,
                (first.fg_color[1] * 255.0) as u8,
                (first.fg_color[2] * 255.0) as u8,
                first.flags,
                first.atlas_size[0],
                first.atlas_size[1],
                first.bearing[0],
                first.bearing[1],
                first.glyph_advance_width,
            );
        } else {
            log::trace!("RENDER_INSTANCES: ZERO instances — nothing to render!");
        }

        self.title = snapshot.title.clone();
        self.frame_count += 1;

        self.last_cursor_row = snapshot.cursor_row;
        self.last_cursor_col = snapshot.cursor_col;

        // CPU-side render work ends here (snapshot + dirty diff + instance
        // build + atlas upload). The GPU `render_frame` call below also performs
        // the swapchain `present`, which BLOCKS until the next vsync (≈16.6ms on
        // a 60Hz display with Mailbox). That wait is the display refresh, not our
        // cost — so it must NOT count toward RENDER_SLOW, or every vsync-paced
        // frame would falsely report as slow.
        let cpu_work_end = Instant::now();
        let cpu_ms = cpu_work_end.duration_since(frame_start).as_secs_f64() * 1000.0;

        // Desktop: direct wgpu swapchain presentation.
        #[cfg(not(target_os = "android"))]
        {
            let gpu = self
                .gpu
                .as_mut()
                .ok_or_else(|| SurfaceError::GpuInit("GPU not initialized for render".into()))?;
            if let Err(e) = gpu.render_frame(instances, &[]) {
                log::error!("RENDER_FRAME_FAILED: {}", e);
                return Err(SurfaceError::Render(e.to_string()));
            }
        }

        // Android: wgpu Vulkan swapchain — sole render path.
        // If swapchain fails, reconfigure the surface and try exactly once more
        // (reconfiguring clears the stale state that caused the AcquireFailed /
        // OutOfDate error). If the retry also fails, keep render_requested and
        // return Ok(false) so the caller retries the next frame.
        #[cfg(target_os = "android")]
        let swapchain_ok = {
            if sync_active {
                log::trace!("sync active — skipping GPU frame");
                trace_end(); // swapchain_present
                trace_end(); // AndroidSurface::render
                return Ok(false);
            }
            let gpu = self.gpu.as_mut().ok_or_else(|| {
                SurfaceError::GpuInit("GPU not initialized for Android render".into())
            })?;
            if gpu.has_surface() {
                match gpu.render_frame(instances, &[]) {
                    Ok(()) => true,
                    Err(e) => {
                        log::error!("SWAPCHAIN_FAILED (will reconfigure): {}", e);
                        let (sw, sh) = (
                            self.surface_width
                                .load(std::sync::atomic::Ordering::Relaxed),
                            self.surface_height
                                .load(std::sync::atomic::Ordering::Relaxed),
                        );
                        if let Some(ref nw) = self.native_window {
                            if let Err(reconfig_err) =
                                gpu.configure_android_surface(nw.0.as_ptr(), sw.max(1), sh.max(1))
                            {
                                log::error!("SWAPCHAIN_RECONFIG_FAILED: {}", reconfig_err);
                                self.render_requested = true;
                                false
                            } else {
                                gpu.set_bg_color(self.theme.background);
                                match gpu.render_frame(instances, &[]) {
                                    Ok(()) => true,
                                    Err(retry_err) => {
                                        log::error!("SWAPCHAIN_RETRY_FAILED: {}", retry_err);
                                        self.render_requested = true;
                                        false
                                    }
                                }
                            }
                        } else {
                            self.render_requested = true;
                            false
                        }
                    }
                }
            } else {
                true
            }
        };
        #[cfg(not(target_os = "android"))]
        let swapchain_ok = true;

        trace_end(); // swapchain_present

        let elapsed = frame_start.elapsed();
        let present_ms = elapsed.as_secs_f64() * 1000.0 - cpu_ms;
        // RENDER_SLOW reflects only the CPU-side render cost (snapshot + diff +
        // build + submit). The present/vsync wait is the display refresh and is
        // logged at trace level, never as a warning — otherwise every vsync-paced
        // frame (≈16.6ms present on 60Hz) would spuriously warn.
        if cpu_ms >= FRAME_TIME_TARGET_MS {
            log::warn!(
                "RENDER_SLOW: cpu={:.1}ms present={:.1}ms",
                cpu_ms,
                present_ms
            );
        } else {
            log::debug!("RENDER_OK: cpu={:.1}ms present={:.1}ms", cpu_ms, present_ms);
        }

        // Swap caches for next frame — eliminates ~800KB memcpy/frame
        std::mem::swap(&mut self.cached_instances, &mut self.instance_buffer);
        self.cached_row_ends = row_ends;
        self.prev_scroll_offset = scroll_offset;
        self.prev_render_height = self.render_height;

        if swapchain_ok {
            self.render_requested = false;
        }

        Ok(swapchain_ok)
    }

    /// Render a single test frame to an offscreen GPU buffer and write raw RGBA
    /// data to `{data_dir}/test_frame.rgba`. Returns the file path on success.
    /// This is a test-only path — NOT used for display.
    pub fn save_test_frame(&mut self, data_dir: &str) -> Result<String, SurfaceError> {
        let snapshot = {
            let guard = self
                .session
                .as_ref()
                .ok_or(SurfaceError::NoSession)?
                .lock()
                .map_err(|_| SurfaceError::NoSession)?;
            guard.terminal().take_snapshot()
        };
        log::info!(
            "SAVE_TEST_FRAME: selection={:?}, snapshot_rows={}, snapshot_cols={}",
            self.selection,
            snapshot.rows,
            snapshot.cols,
        );
        let tc = self.theme.cursor;
        let cursor_color = Some([
            tc[0] as f32 / 255.0,
            tc[1] as f32 / 255.0,
            tc[2] as f32 / 255.0,
            1.0,
        ]);
        let selection_bg = self.theme.selection_bg;
        let selection_bg = Some([
            selection_bg[0] as f32 / 255.0,
            selection_bg[1] as f32 / 255.0,
            selection_bg[2] as f32 / 255.0,
            1.0,
        ]);
        torvox_renderer::gpu::build_cell_instances_into(
            &snapshot,
            &mut self.font_pipeline,
            torvox_renderer::gpu::CellInstanceConfig {
                atlas_width: self.atlas_width as f32,
                atlas_height: self.atlas_height as f32,
                projection_height: self.render_height as f32,
                selection: self.selection,
                selection_bg,
                search_highlights: &self.search_highlights,
                cursor_color,
                cursor_style: self.cursor_style,
                surface_bg: [
                    self.theme.background[0] as f32 / 255.0,
                    self.theme.background[1] as f32 / 255.0,
                    self.theme.background[2] as f32 / 255.0,
                    1.0,
                ],
                render_scale: torvox_renderer::gpu::RENDER_SCALE,
                dirty_rows: &[],
                cached_instances: &[],
                cached_row_ends: &[],
            },
            &mut self.instance_buffer,
            &mut Vec::new(),
        );
        let instances = &self.instance_buffer[..];
        let gpu = self
            .gpu
            .as_mut()
            .ok_or_else(|| SurfaceError::GpuInit("GPU not initialized for test frame".into()))?;
        gpu.upload_atlas(
            self.font_pipeline.atlas_bitmap(),
            self.atlas_width,
            self.atlas_height,
        );
        let pixels = gpu
            .render_to_buffer(instances, &[])
            .map_err(|e| SurfaceError::Render(e.to_string()))?;
        let surface_width = self
            .surface_width
            .load(std::sync::atomic::Ordering::Relaxed);
        let width_header = (surface_width as u32).to_le_bytes();
        let mut framed = Vec::with_capacity(4 + pixels.len());
        framed.extend_from_slice(&width_header);
        framed.extend_from_slice(&pixels);
        let path = format!("{}/test_frame.rgba", data_dir);
        std::fs::write(&path, &framed).map_err(|e| SurfaceError::Render(e.to_string()))?;
        log::info!(
            "SAVED_TEST_FRAME: {} ({} bytes + 4 header)",
            path,
            pixels.len()
        );
        Ok(path)
    }

    pub fn font_pipeline(&self) -> &FontPipeline {
        &self.font_pipeline
    }

    pub fn font_pipeline_mut(&mut self) -> &mut FontPipeline {
        &mut self.font_pipeline
    }

    pub fn recompute_grid(&mut self, width: u32, height: u32) {
        let (cw, ch) = self.font_pipeline.cell_metrics();
        let new_cols = (width as f32 / cw).floor().clamp(MIN_COLS, MAX_COLS) as u32;
        let new_rows = (height as f32 / ch).floor().clamp(MIN_ROWS, MAX_ROWS) as u32;

        self.render_width = width;
        self.render_height = height;

        if width != self.surface_width.load(Ordering::Relaxed)
            || height != self.surface_height.load(Ordering::Relaxed)
        {
            self.surface_width.store(width, Ordering::Relaxed);
            self.surface_height.store(height, Ordering::Relaxed);
        }

        if new_cols != self.cols || new_rows != self.rows {
            log::info!(
                "RECOMPUTE_GRID: {}x{} -> {}x{} (cell={:.1}x{:.1})",
                self.rows,
                self.cols,
                new_rows,
                new_cols,
                cw,
                ch,
            );
            self.rows = new_rows;
            self.cols = new_cols;
            if let Some(ref session_arc) = self.session
                && let Ok(mut session) = session_arc.lock()
                && let Err(error) = session.resize(new_rows, new_cols)
            {
                log::error!("surface: session resize failed: {error}");
            }
        }
    }

    pub fn resize(&mut self, rows: u32, cols: u32) {
        log::trace!(
            "SURFACE_RESIZE: rows={} cols={} has_session={}",
            rows,
            cols,
            self.session.is_some(),
        );
        self.rows = rows;
        self.cols = cols;
        if let Some(ref session_arc) = self.session {
            let mut session = match session_arc.lock() {
                Ok(guard) => guard,
                Err(poisoned) => {
                    let guard = poisoned.into_inner();
                    log::warn!("resize: session mutex was poisoned, recovered");
                    guard
                }
            };
            if let Err(error) = session.resize(rows, cols) {
                log::error!("surface: session resize failed: {error}");
            }
        }
    }

    pub fn write_to_pty(&mut self, data: &[u8]) {
        if let Some(ref session_arc) = self.session {
            let mut session = match session_arc.lock() {
                Ok(guard) => guard,
                Err(poisoned) => {
                    let guard = poisoned.into_inner();
                    log::warn!("write_to_pty: session mutex was poisoned, recovered");
                    guard
                }
            };
            if let Err(error) = session.write(data) {
                log::error!("surface: PTY write failed: {error}");
            }
        } else {
            log::warn!("surface: write_to_pty skipped — session not available");
        }
    }

    pub fn is_exited(&self) -> bool {
        self.exited.load(Ordering::Acquire)
    }

    pub fn poll_bel(&mut self) -> bool {
        if let Some(ref session_arc) = self.session
            && let Ok(session) = session_arc.lock()
        {
            return session.poll_bel();
        }
        false
    }

    pub fn poll_clipboard(&mut self) -> Option<String> {
        if let Some(ref session_arc) = self.session
            && let Ok(session) = session_arc.lock()
        {
            return session.poll_clipboard();
        }
        None
    }

    pub fn poll_notification(&mut self) -> Option<(String, String)> {
        if let Some(ref session_arc) = self.session
            && let Ok(session) = session_arc.lock()
        {
            return session.poll_notification();
        }
        None
    }

    pub fn poll_sync_active(&mut self) -> bool {
        if let Some(ref session_arc) = self.session
            && let Ok(session) = session_arc.lock()
        {
            return session.mode_get(SYNC_MODE_NUMBER, 0);
        }
        false
    }

    pub fn poll_shell_integration(&mut self) -> u8 {
        if let Some(ref session_arc) = self.session
            && let Ok(session) = session_arc.lock()
        {
            return session.poll_shell_integration() as u8;
        }
        0
    }

    /// Poll all deferred events (BEL, clipboard, notification, sync mode, shell
    /// integration) in a single session lock acquisition. This avoids the
    /// per-poll session-lock churn that the individual `poll_*` methods incur.
    pub fn poll_all(&mut self) -> (bool, Option<String>, Option<(String, String)>, bool, u8) {
        if let Some(ref session_arc) = self.session
            && let Ok(session) = session_arc.lock()
        {
            return (
                session.poll_bel(),
                session.poll_clipboard(),
                session.poll_notification(),
                session.mode_get(SYNC_MODE_NUMBER, 0),
                session.poll_shell_integration() as u8,
            );
        }
        (false, None, None, false, 0)
    }

    pub fn cwd(&self) -> String {
        if let Some(ref session_arc) = self.session
            && let Ok(session) = session_arc.lock()
        {
            return session.cwd();
        }
        String::new()
    }

    pub fn focus_event(&mut self, focused: bool) {
        if let Some(ref session_arc) = self.session
            && let Ok(mut session) = session_arc.lock()
        {
            session.focus_event(focused);
        }
    }

    pub fn has_session(&self) -> bool {
        self.session.is_some()
    }

    pub fn gpu_mut(&mut self) -> Option<&mut GpuContext> {
        self.gpu.as_mut()
    }

    pub fn release_gpu_surface(&mut self) {
        if let Some(gpu) = &mut self.gpu {
            gpu.release_gpu_surface();
        }
    }

    pub fn set_font_size(&mut self, size: f32) {
        if (self.font_pipeline.font_size() - size).abs() < 0.001 {
            return;
        }
        // Use in-place update to avoid dropping FontPipeline while render thread may reference it.
        // The surface Mutex serializes access between the render thread and font changes.
        self.set_font_size_in_place(size);
    }

    pub fn set_font_family(&mut self, family_name: &str) -> bool {
        let previous_name = self.font_pipeline.current_font_family_name();
        if !self.font_pipeline.set_font_family(family_name) {
            log::warn!(
                "FONT_FAMILY: '{}' not found, restoring previous",
                family_name
            );
            if let Some(ref prev) = previous_name {
                self.font_pipeline.set_font_family(prev);
                self.font_pipeline.rasterize_ascii();
                if let Some(gpu) = &mut self.gpu {
                    let (aw, ah) = self.font_pipeline.atlas_dimensions();
                    gpu.update_bind_group(
                        aw as f32,
                        ah as f32,
                        self.render_width as f32,
                        self.render_height as f32,
                    );
                    gpu.upload_atlas(self.font_pipeline.atlas_bitmap(), aw, ah);
                }
            }
            return false;
        }
        self.font_pipeline.rasterize_ascii();
        let (aw, ah) = self.font_pipeline.atlas_dimensions();
        let (cw, ch) = self.font_pipeline.cell_metrics();
        if let Some(gpu) = &mut self.gpu {
            gpu.update_bind_group(
                aw as f32,
                ah as f32,
                self.render_width as f32,
                self.render_height as f32,
            );
            gpu.upload_atlas(self.font_pipeline.atlas_bitmap(), aw, ah);
        }
        if self.surface_width.load(Ordering::Relaxed) > 0
            && self.surface_height.load(Ordering::Relaxed) > 0
        {
            let new_cols = (self.surface_width.load(Ordering::Relaxed) as f32 / cw)
                .floor()
                .clamp(MIN_COLS, MAX_COLS) as u32;
            let new_rows = (self.surface_height.load(Ordering::Relaxed) as f32 / ch)
                .floor()
                .clamp(MIN_ROWS, MAX_ROWS) as u32;
            self.cols = new_cols;
            self.rows = new_rows;
            if let Some(ref session_arc) = self.session
                && let Ok(mut session) = session_arc.lock()
                && let Err(error) = session.resize(new_rows, new_cols)
            {
                log::error!("surface: session resize failed: {error}");
            }
            log::info!(
                "set_font_family: family='{}' cells={:.1}x{:.1} grid={}x{}",
                family_name,
                cw,
                ch,
                new_rows,
                new_cols,
            );
        }
        true
    }

    pub fn set_font_size_in_place(&mut self, new_size: f32) {
        if (self.font_pipeline.font_size() - new_size).abs() < 0.001 {
            return;
        }
        let (cw, ch) = self.font_pipeline.set_font_size_in_place(new_size);

        let (actual_cw, actual_ch) = self.font_pipeline.cell_metrics();
        if (actual_cw - cw).abs() > 0.1 || (actual_ch - ch).abs() > 0.1 {
            log::warn!(
                "set_font_size_in_place: cell metrics mismatch expected={:.1}x{:.1} actual={:.1}x{:.1}",
                cw,
                ch,
                actual_cw,
                actual_ch
            );
        }

        if let Some(gpu) = &mut self.gpu {
            let (aw, ah) = self.font_pipeline.atlas_dimensions();
            gpu.update_bind_group(
                aw as f32,
                ah as f32,
                self.render_width as f32,
                self.render_height as f32,
            );
            gpu.upload_atlas(self.font_pipeline.atlas_bitmap(), aw, ah);
        }

        if self.surface_width.load(Ordering::Relaxed) > 0
            && self.surface_height.load(Ordering::Relaxed) > 0
        {
            let new_cols = (self.surface_width.load(Ordering::Relaxed) as f32 / actual_cw)
                .floor()
                .clamp(MIN_COLS, MAX_COLS) as u32;
            let new_rows = (self.surface_height.load(Ordering::Relaxed) as f32 / actual_ch)
                .floor()
                .clamp(MIN_ROWS, MAX_ROWS) as u32;
            self.cols = new_cols;
            self.rows = new_rows;
            if let Some(ref session_arc) = self.session
                && let Ok(mut session) = session_arc.lock()
                && let Err(error) = session.resize(new_rows, new_cols)
            {
                log::error!("surface: session resize failed: {error}");
            }
            log::info!(
                "set_font_size_in_place: size={} cells={:.1}x{:.1} grid={}x{}",
                new_size,
                actual_cw,
                actual_ch,
                new_rows,
                new_cols
            );
        }
    }

    pub fn apply_font_settings(&mut self, font_size: f32, family_name: &str) {
        let font_size_changed = (self.font_pipeline.font_size() - font_size).abs() >= 0.001;
        let family_changed =
            self.font_pipeline.current_font_family_name().as_deref() != Some(family_name);

        if !font_size_changed && !family_changed {
            return;
        }

        if family_changed {
            self.set_font_family(family_name);
        }
        if font_size_changed {
            self.set_font_size_in_place(font_size);
        }
    }

    /// Load a font file into the pipeline and return its family name.
    pub fn load_font_file(&mut self, path: &std::path::Path) -> Option<String> {
        self.font_pipeline.load_font_file(path)
    }

    pub fn set_mouse_position(&mut self, row: u32, col: u32) {
        self.mouse_row = Some(row);
        self.mouse_col = Some(col);
    }

    pub fn get_hovered_url(&self) -> Option<String> {
        self.last_hovered_url.clone()
    }

    pub fn get_title(&self) -> String {
        self.title.clone()
    }

    pub fn set_theme(&mut self, theme: torvox_core::config::Theme) {
        let (bg, fg) = (theme.background, theme.foreground);
        let ansi = theme.ansi;
        self.theme = theme;
        if let Some(ref session_arc) = self.session {
            match session_arc.lock() {
                Ok(session) => session.terminal().set_theme(bg, fg, ansi),
                Err(poisoned) => {
                    let session = poisoned.into_inner();
                    session.terminal().set_theme(bg, fg, ansi);
                    log::warn!("set_theme: session mutex was poisoned, recovered");
                }
            }
        }
        if let Some(gpu) = self.gpu.as_mut() {
            gpu.set_bg_color(bg);
        }
        self.render_requested = true;
    }

    #[cfg(test)]
    pub fn render_requested(&self) -> bool {
        self.render_requested
    }

    /// Force the next `render()` call to actually present a frame even when
    /// there is no new PTY output. Used for view-only state changes such as
    /// scrolling, where `take_snapshot_with_scroll` must run to shift the
    /// displayed rows but `had_output` is false. Without this, `render()`
    /// early-returns at the idle skip and the viewport never scrolls.
    pub fn set_render_requested(&mut self, value: bool) {
        self.render_requested = value;
    }

    pub fn set_selection(&mut self, sel: Option<SelectionRange>) {
        self.selection = sel;
        self.render_requested = true;
    }

    pub fn set_search_highlights(&mut self, highlights: Vec<SearchHighlight>) {
        self.search_highlights = highlights;
        self.render_requested = true;
    }

    pub fn clear_search_highlights(&mut self) {
        self.search_highlights.clear();
        self.render_requested = true;
    }

    pub fn theme(&self) -> &torvox_core::config::Theme {
        &self.theme
    }

    pub fn save_session(&self, path: &str) -> Result<(), SurfaceError> {
        use std::fs;
        use torvox_core::snapshot::SessionSnapshot;

        let guard = self
            .session
            .as_ref()
            .ok_or(SurfaceError::NoSession)?
            .lock()
            .map_err(|_| SurfaceError::NoSession)?;
        let dumped = guard.terminal().dump_grid();
        let (rows, cols) = (dumped.rows, dumped.cols);

        let mut visible_lines = Vec::with_capacity(rows as usize);
        for row in 0..rows as usize {
            let start = row * cols as usize;
            let row_cells = &dumped.visible[start..start + cols as usize];
            visible_lines.push(cell_to_line(row_cells, cols));
        }

        let mut scrollback_lines = Vec::with_capacity(dumped.scrollback.len());
        for sb_row in &dumped.scrollback {
            scrollback_lines.push(cell_to_line(sb_row, cols));
        }

        let snapshot = SessionSnapshot {
            visible_lines,
            scrollback_lines,
            rows,
            cols,
            max_scrollback: DEFAULT_MAX_SCROLLBACK,
        };

        let bytes = rkyv::to_bytes::<rkyv::rancor::Error>(&snapshot)
            .map_err(|e| SurfaceError::Session(format!("rkyv serialize: {e}")))?;
        fs::write(path, &bytes).map_err(|e| SurfaceError::Session(format!("write failed: {e}")))?;
        Ok(())
    }

    /// Requirement 4 (session restore, Fix G): rebuild the scrollback/visible text
    /// so a restored session matches the saved row count without a spurious
    /// trailing blank line. Pure + testable.
    ///
    /// 1. NUL padding becomes a real space (cell_to_line already maps a NUL
    ///    codepoint to ' ', but normalize defensively) so blank rows carry
    ///    visible blank content instead of garbage.
    /// 2. Do NOT per-line `trim_end`: an intentional blank line is spaces, and
    ///    trimming would collapse it. Middle blank lines must survive.
    /// 3. Trim only genuinely-empty TRAILING lines (whitespace-only). The shell
    ///    echoes the re-fed text and emits one prompt newline; a trailing
    ///    whitespace-only row from the save/restore is the off-by-one extra
    ///    blank line, so it is dropped here. Rows are joined with a single '\n'
    ///    and NO trailing newline, which avoids advancing the cursor onto an
    ///    extra empty row.
    pub fn restore_session_lines_to_text(snapshot: &SessionSnapshot) -> String {
        let mut lines: Vec<String> = snapshot
            .scrollback_lines
            .iter()
            .chain(&snapshot.visible_lines)
            .map(|line| line_to_text(line).replace('\0', " "))
            .collect();
        while let Some(last) = lines.last()
            && last.trim().is_empty()
        {
            lines.pop();
        }
        lines.join("\n")
    }

    pub fn restore_session(&mut self, path: &str) -> Result<(), SurfaceError> {
        use rkyv::rancor;
        use std::fs;
        use torvox_core::snapshot::SessionSnapshot;

        let data =
            fs::read(path).map_err(|e| SurfaceError::Session(format!("read failed: {e}")))?;
        let snapshot = rkyv::from_bytes::<SessionSnapshot, rancor::Error>(&data)
            .map_err(|e| SurfaceError::Session(format!("rkyv deserialize: {e}")))?;

        if let Some(ref session_arc) = self.session
            && let Ok(mut session) = session_arc.lock()
        {
            // Fix G: rebuild the scrollback/visible text so a restored session
            // matches the saved row count without a spurious trailing blank line.
            //
            // 1. NUL padding becomes a real space (cell_to_line already maps a NUL
            //    codepoint to ' ', but normalize defensively) so blank rows carry
            //    visible blank content instead of garbage.
            // 2. Do NOT per-line `trim_end`: an intentional blank line is spaces,
            //    and trimming would collapse it. Middle blank lines must survive
            //    the re-feed (joined as an empty element => a blank row).
            // 3. Trim only genuinely-empty TRAILING lines (whitespace-only). The
            //    shell echoes the re-fed text and emits one prompt newline; a
            //    trailing whitespace-only row from the save/restore is the
            //    off-by-one extra blank line, so it is dropped here. Rows are
            //    joined with a single '\n' and NO trailing newline, which avoids
            //    advancing the cursor onto an extra empty row.
            let text = Self::restore_session_lines_to_text(&snapshot);
            if !text.is_empty() {
                session.terminal_mut().pty_write(text.as_bytes());
            }
        }

        if let Err(error) = fs::remove_file(path) {
            log::warn!("surface: failed to remove temp file {path:?}: {error}");
        }
        Ok(())
    }

    pub fn has_saved_session(path: &str) -> bool {
        std::path::Path::new(path).exists()
    }

    pub fn set_background_image(&mut self, rgba_data: &[u8], width: u32, height: u32) {
        if let Some(gpu) = self.gpu.as_mut() {
            gpu.set_bg_image(rgba_data, width, height);
        }
        self.render_requested = true;
    }

    pub fn clear_background_image(&mut self) {
        if let Some(gpu) = self.gpu.as_mut() {
            gpu.clear_bg_image();
        }
        self.render_requested = true;
    }

    pub fn set_background_params(&mut self, blur_radius: f32, alpha: f32) {
        if let Some(gpu) = self.gpu.as_mut() {
            gpu.set_background_params(blur_radius, alpha);
        }
        self.render_requested = true;
    }

    pub fn set_blink_enabled(&mut self, enabled: bool) {
        self.blink_enabled = enabled;
        if !enabled {
            self.blink_phase = true;
        }
        self.render_requested = true;
    }

    pub fn set_blink_speed_ms(&mut self, speed_ms: u32) {
        self.blink_speed_ms = speed_ms.clamp(100, 1000);
        self.render_requested = true;
    }

    pub fn reset_blink(&mut self) {
        self.blink_phase = true;
        self.last_blink_toggle = std::time::Instant::now();
        self.render_requested = true;
    }

    pub fn set_cursor_style(&mut self, style: torvox_core::cursor::CursorStyle) {
        self.cursor_style = style;
        self.render_requested = true;
    }
}

impl Drop for AndroidSurface {
    fn drop(&mut self) {
        if let Some(path) = self.save_path.as_ref()
            && self.session.is_some()
            && let Err(error) = self.save_session(&path.to_string_lossy())
        {
            log::error!("surface: save_session in Drop failed: {error}");
        }
        self.session.take();
        self.gpu.take();
        #[cfg(target_os = "android")]
        if let Some(nw) = &self.native_window {
            // SAFETY: nw.0 is a NonNull pointer obtained from ANativeWindow_fromSurface.
            // The NativeWindow wrapper is guarded by the Mutex lock discipline — the
            // absence of Sync ensures &NativeWindow is never shared across threads.
            // This Drop impl is called exactly once when AndroidSurface is dropped,
            // which happens after all other access to the native window has completed
            // (the session and GPU are taken first).
            unsafe { ANativeWindow_release(nw.0.as_ptr()) };
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cell_to_line_empty() {
        let cells = [];
        let line = cell_to_line(&cells, 5);
        assert_eq!(line.len(), 5);
    }

    #[test]
    fn cell_to_line_populated() {
        let cells = vec![
            CellSnapshot {
                codepoint: 0x41, // 'A'
                graphemes: Vec::new(),
                foreground: [1.0, 0.0, 0.0, 1.0],
                background: [0.0, 0.0, 1.0, 1.0],
                bold: true,
                italic: false,
                underline: false,
                reverse: false,
                strikethrough: false,
                blink: false,
                hidden: false,
                uri: None,
                semantic: Default::default(),
                overline: false,
                double_underline: false,
                dim: false,
                width: 1,
            },
            CellSnapshot {
                codepoint: 0x42, // 'B'
                ..Default::default()
            },
        ];
        let line = cell_to_line(&cells, 2);
        assert_eq!(line.len(), 2);
        let c0 = line.get(0).unwrap();
        assert_eq!(c0.char, 'A');
        assert!(c0.attrs.bold);
        assert_eq!(c0.foreground.r, 255);
        assert_eq!(c0.background.b, 255);
    }

    #[test]
    fn cell_to_line_truncates_excess() {
        let cells = vec![
            CellSnapshot {
                codepoint: 0x41,
                ..Default::default()
            },
            CellSnapshot {
                codepoint: 0x42,
                ..Default::default()
            },
            CellSnapshot {
                codepoint: 0x43,
                ..Default::default()
            },
        ];
        let line = cell_to_line(&cells, 2);
        assert_eq!(line.len(), 2);
        assert_eq!(line.get(0).unwrap().char, 'A');
        assert_eq!(line.get(1).unwrap().char, 'B');
    }

    #[test]
    fn cell_to_line_pads_short_input() {
        let cells = [CellSnapshot {
            codepoint: 0x41,
            ..Default::default()
        }];
        let line = cell_to_line(&cells, 5);
        assert_eq!(line.len(), 5);
        assert_eq!(line.get(4).unwrap().char, ' ');
    }

    #[test]
    fn line_to_text_basic() {
        let cells = vec![
            CellSnapshot {
                codepoint: 0x48,
                ..Default::default()
            }, // H
            CellSnapshot {
                codepoint: 0x69,
                ..Default::default()
            }, // i
        ];
        let line = cell_to_line(&cells, 2);
        assert_eq!(line_to_text(&line), "Hi");
    }

    #[test]
    fn line_to_text_includes_null_chars() {
        let cells = vec![
            CellSnapshot {
                codepoint: 0x48,
                ..Default::default()
            },
            CellSnapshot {
                codepoint: 0x69,
                ..Default::default()
            },
            CellSnapshot {
                codepoint: 0x00,
                ..Default::default()
            },
        ];
        let line = cell_to_line(&cells, 3);
        assert_eq!(line_to_text(&line), "Hi\0");
    }

    #[test]
    fn line_to_text_all_spaces() {
        let line = cell_to_line(&[], 5);
        assert_eq!(line_to_text(&line), "     ");
    }

    #[test]
    fn line_to_text_preserves_colors() {
        let cell = CellSnapshot {
            codepoint: 0x41,
            foreground: [0.5, 0.0, 0.0, 1.0],
            ..Default::default()
        };
        let line = cell_to_line(&[cell], 1);
        assert_eq!(line_to_text(&line), "A");
    }

    #[test]
    fn line_to_text_unicode() {
        let cells = vec![
            CellSnapshot {
                codepoint: 0x4E2D,
                ..Default::default()
            }, // 中
            CellSnapshot {
                codepoint: 0x6587,
                ..Default::default()
            }, // 文
        ];
        let line = cell_to_line(&cells, 2);
        assert_eq!(line_to_text(&line), "中文");
    }

    #[test]
    fn has_saved_session_nonexistent() {
        assert!(!AndroidSurface::has_saved_session("/nonexistent/path"));
    }

    #[test]
    fn cell_to_line_with_bold_italic_underline_reverse() {
        let cell = CellSnapshot {
            codepoint: 0x41,
            bold: true,
            italic: true,
            underline: true,
            reverse: true,
            ..Default::default()
        };
        let line = cell_to_line(&[cell], 1);
        let c = line.get(0).unwrap();
        assert!(c.attrs.bold);
        assert!(c.attrs.italic);
        assert!(c.attrs.underline);
        assert!(c.attrs.reverse);
    }

    #[test]
    fn no_session_terminal_returns_err() {
        // SurfaceError::NoSession is returned when terminal() is called without an active session.
        // This prevents the old .expect("no session") panic pattern that caused SIGABRT on device.
        let err = SurfaceError::NoSession;
        assert_eq!(err.to_string(), "No active session");
    }

    #[test]
    fn set_theme_sets_render_requested() {
        let mut surface = AndroidSurface::new(24, 80, 1000, 14.0);
        let theme = torvox_core::config::Theme::catppuccin_mocha();
        surface.set_theme(theme);
        assert!(
            surface.render_requested(),
            "set_theme should set render_requested to true"
        );
    }

    #[test]
    fn render_after_set_theme_proceeds() {
        let mut surface = AndroidSurface::new(24, 80, 1000, 14.0);
        let theme = torvox_core::config::Theme::catppuccin_mocha();
        surface.set_theme(theme);
        assert!(
            surface.render_requested(),
            "flag should be true after set_theme"
        );
        // render() with no session returns Err(NoSession), but the flag
        // should not be consumed on error
        let result = surface.render(0);
        assert!(result.is_err(), "render with no session should fail");
        // Flag should remain true on error (not cleared)
        assert!(
            surface.render_requested(),
            "flag should persist after render error"
        );
    }

    #[test]
    fn render_requested_consumed_once() {
        let mut surface = AndroidSurface::new(24, 80, 1000, 14.0);
        let theme = torvox_core::config::Theme::catppuccin_mocha();
        surface.set_theme(theme);
        assert!(
            surface.render_requested(),
            "flag should be true after set_theme"
        );
        // render() will error with NoSession since no session is set up
        let result = surface.render(0);
        assert!(result.is_err(), "render with no session should fail");
        // Flag should still be true after error (not cleared early)
        assert!(
            surface.render_requested(),
            "flag should remain true on render error"
        );
    }

    // ── Cursor visibility invariants ──

    #[test]
    fn blink_phase_default_is_true() {
        let surface = AndroidSurface::new(24, 80, 1000, 14.0);
        assert!(
            surface.blink_phase,
            "blink_phase should default to true so cursor is visible"
        );
    }

    #[test]
    fn blink_phase_toggles_after_timer() {
        let mut surface = AndroidSurface::new(24, 80, 1000, 14.0);
        surface.blink_speed_ms = 10;
        surface.last_blink_toggle = std::time::Instant::now()
            .checked_sub(std::time::Duration::from_millis(20))
            .expect("subtract 20ms");
        let before = surface.blink_phase;
        assert!(surface.blink_timer_elapsed(), "timer should be elapsed");
        // Simulate the toggle that render_frame does
        if surface.blink_timer_elapsed() {
            surface.blink_phase = !surface.blink_phase;
            surface.last_blink_toggle = std::time::Instant::now();
        }
        assert_eq!(
            surface.blink_phase, !before,
            "blink_phase should toggle timer"
        );
    }

    #[test]
    fn blink_phase_toggles_repeatedly() {
        let mut surface = AndroidSurface::new(24, 80, 1000, 14.0);
        surface.blink_speed_ms = 10;
        let mut phase = true;
        for _ in 0..10 {
            surface.last_blink_toggle = std::time::Instant::now()
                .checked_sub(std::time::Duration::from_millis(20))
                .expect("subtract 20ms");
            if surface.blink_timer_elapsed() {
                surface.blink_phase = !surface.blink_phase;
                surface.last_blink_toggle = std::time::Instant::now();
            }
            assert_eq!(
                surface.blink_phase, !phase,
                "phase should toggle every cycle"
            );
            phase = !phase;
        }
    }

    #[test]
    fn blink_phase_stays_true_when_blink_disabled() {
        let mut surface = AndroidSurface::new(24, 80, 1000, 14.0);
        surface.set_blink_enabled(false);
        surface.blink_phase = true;
        // Simulate many blink cycles
        surface.blink_speed_ms = 10;
        for _ in 0..5 {
            surface.last_blink_toggle = std::time::Instant::now()
                .checked_sub(std::time::Duration::from_millis(50))
                .expect("subtract 50ms");
            if surface.blink_timer_elapsed() {
                surface.blink_phase = !surface.blink_phase;
                surface.last_blink_toggle = std::time::Instant::now();
            }
        }
        assert!(
            surface.blink_phase,
            "blink_phase should stay true when blink disabled"
        );
    }

    // ── Cursor blink tests ──

    #[test]
    fn set_blink_enabled_sets_flag() {
        let mut surface = AndroidSurface::new(24, 80, 1000, 14.0);
        assert!(surface.blink_enabled, "blink should default to true");
        surface.set_blink_enabled(false);
        assert!(!surface.blink_enabled, "blink should be disabled");
        assert!(
            surface.render_requested(),
            "set_blink_enabled should request render"
        );
        assert!(
            surface.blink_phase,
            "blink_phase should be true when blink disabled"
        );
    }

    #[test]
    fn set_blink_enabled_true_keeps_phase() {
        let mut surface = AndroidSurface::new(24, 80, 1000, 14.0);
        // Force blink_phase to false (normally this only happens after ~530ms)
        surface.blink_phase = false;
        surface.set_blink_enabled(true);
        assert!(surface.blink_enabled, "blink should be enabled");
        assert!(
            surface.render_requested(),
            "set_blink_enabled should request render"
        );
        // blink_phase should remain false when enabling (not reset)
        // (reset_blink is the explicit method for that)
    }

    #[test]
    fn set_blink_speed_ms_stores_value() {
        let mut surface = AndroidSurface::new(24, 80, 1000, 14.0);
        assert_eq!(
            surface.blink_speed_ms, 530,
            "default blink speed should be 530"
        );
        surface.set_blink_speed_ms(750);
        assert_eq!(surface.blink_speed_ms, 750, "blink speed should be 750");
        assert!(
            surface.render_requested(),
            "set_blink_speed_ms should request render"
        );
    }

    #[test]
    fn set_blink_speed_ms_clamps_low() {
        let mut surface = AndroidSurface::new(24, 80, 1000, 14.0);
        surface.set_blink_speed_ms(25);
        assert_eq!(
            surface.blink_speed_ms, 100,
            "blink speed should clamp to 100 minimum"
        );
    }

    #[test]
    fn set_blink_speed_ms_clamps_high() {
        let mut surface = AndroidSurface::new(24, 80, 1000, 14.0);
        surface.set_blink_speed_ms(9999);
        assert_eq!(
            surface.blink_speed_ms, 1000,
            "blink speed should clamp to 1000 maximum"
        );
    }

    #[test]
    fn reset_blink_sets_phase_and_requests_render() {
        let mut surface = AndroidSurface::new(24, 80, 1000, 14.0);
        surface.blink_phase = false;
        surface.reset_blink();
        assert!(
            surface.blink_phase,
            "reset_blink should set blink_phase to true"
        );
        assert!(
            surface.render_requested(),
            "reset_blink should request render"
        );
    }

    #[test]
    fn blink_period_default_530ms() {
        let surface = AndroidSurface::new(24, 80, 1000, 14.0);
        assert_eq!(
            surface.blink_period(),
            std::time::Duration::from_millis(530)
        );
    }

    #[test]
    fn blink_period_custom_speed() {
        let mut surface = AndroidSurface::new(24, 80, 1000, 14.0);
        surface.blink_speed_ms = 300;
        assert_eq!(
            surface.blink_period(),
            std::time::Duration::from_millis(300)
        );
    }

    #[test]
    fn blink_timer_elapsed_initially_false() {
        let surface = AndroidSurface::new(24, 80, 1000, 14.0);
        // Immediately after construction, last_blink_toggle = now → not elapsed
        assert!(
            !surface.blink_timer_elapsed(),
            "blink timer should not be elapsed immediately"
        );
    }

    #[test]
    fn blink_timer_not_elapsed_when_disabled() {
        let mut surface = AndroidSurface::new(24, 80, 1000, 14.0);
        surface.blink_enabled = false;
        assert!(
            !surface.blink_timer_elapsed(),
            "blink timer should not be elapsed when disabled"
        );
    }

    #[test]
    fn blink_timer_elapsed_after_speed_ms() {
        let mut surface = AndroidSurface::new(24, 80, 1000, 14.0);
        surface.blink_speed_ms = 10;
        surface.last_blink_toggle = std::time::Instant::now()
            .checked_sub(std::time::Duration::from_millis(20))
            .unwrap();
        assert!(
            surface.blink_timer_elapsed(),
            "blink timer should be elapsed after 20ms with 10ms period"
        );
    }

    #[test]
    fn compute_raster_scale_is_surface_over_config_width() {
        // 2x physical surface vs logical config => scale 2.0.
        assert!((compute_raster_scale(1080, 540) - 2.0).abs() < f32::EPSILON);
        assert!((compute_raster_scale(540, 1080) - 0.5).abs() < f32::EPSILON);
        // Not hardcoded: any ratio works; here 1.5x.
        assert!((compute_raster_scale(1620, 1080) - 1.5).abs() < f32::EPSILON);
    }

    #[test]
    fn compute_raster_scale_clamps_extremes() {
        // Surface much larger than config is clamped to 4.0.
        assert!((compute_raster_scale(10000, 100) - 4.0).abs() < f32::EPSILON);
        // Surface much smaller than config is clamped to 0.5.
        assert!((compute_raster_scale(100, 10000) - 0.5).abs() < f32::EPSILON);
        // A zero config width must not divide-by-zero (normalized to 1, then
        // clamped to 4.0).
        assert!((compute_raster_scale(800, 0) - 4.0).abs() < f32::EPSILON);
    }

    #[test]
    fn restore_session_nul_becomes_space_and_trims_trailing_blanks() {
        // Build two visible rows: "ab\0" (NUL padding) and a trailing
        // whitespace-only row that simulates the off-by-one blank line.
        let row_a = cell_to_line(
            &[
                CellSnapshot {
                    codepoint: 0x61,
                    ..Default::default()
                }, // a
                CellSnapshot {
                    codepoint: 0x62,
                    ..Default::default()
                }, // b
                CellSnapshot {
                    codepoint: 0x00,
                    ..Default::default()
                }, // NUL
            ],
            3,
        );
        let trailing_blank = cell_to_line(&[], 3); // all spaces
        let snapshot = SessionSnapshot {
            visible_lines: vec![row_a, trailing_blank],
            scrollback_lines: vec![],
            rows: 2,
            cols: 3,
            max_scrollback: DEFAULT_MAX_SCROLLBACK,
        };
        let text = AndroidSurface::restore_session_lines_to_text(&snapshot);
        // NUL -> space, trailing whitespace-only row trimmed, single '\n', no
        // trailing newline.
        assert_eq!(text, "ab ");
    }

    #[test]
    fn restore_session_preserves_middle_blank_lines() {
        // A blank line in the MIDDLE must survive (no per-line trim_end).
        let row_a = cell_to_line(
            &[CellSnapshot {
                codepoint: 0x61,
                ..Default::default()
            }],
            1,
        );
        let middle_blank = cell_to_line(&[], 1);
        let row_b = cell_to_line(
            &[CellSnapshot {
                codepoint: 0x62,
                ..Default::default()
            }],
            1,
        );
        let snapshot = SessionSnapshot {
            visible_lines: vec![row_a, middle_blank, row_b],
            scrollback_lines: vec![],
            rows: 3,
            cols: 1,
            max_scrollback: DEFAULT_MAX_SCROLLBACK,
        };
        let text = AndroidSurface::restore_session_lines_to_text(&snapshot);
        // The middle blank line is spaces and must survive (no per-line trim_end).
        assert_eq!(text, "a\n \nb");
    }
}
