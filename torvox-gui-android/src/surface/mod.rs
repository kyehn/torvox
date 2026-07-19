//! AndroidSurface — render pipeline lifecycle for Android TextureView.
//!
//! # Requirements
//! - [FR-018](crate) — Surface: Android TextureView lifecycle
//! - [FR-024](crate) — Surface: resolution change handling
//! - [FR-052](crate) — Surface: SurfaceView → TextureView migration

mod input;
mod render;
mod session;

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};

use thiserror::Error;
use torvox_renderer::font::FontPipeline;
use torvox_renderer::gpu::GpuContext;
use torvox_renderer::gpu::SearchHighlight;
use torvox_renderer::gpu::SelectionRange;

use torvox_terminal::session::Session;

use crate::lock_util::lock_or_recover;

#[cfg(target_os = "android")]
const WINDOW_FORMAT_RGBA_8888: i32 = 1;
const FONT_SIZE_FALLBACK_MULTIPLIER: f32 = 1.6;
const MIN_FONT_SIZE: f32 = 24.0;
const MIN_COLS: f32 = 20.0;
const MAX_COLS: f32 = 300.0;
const MIN_ROWS: f32 = 5.0;
const MAX_ROWS: f32 = 200.0;
const FRAME_TIME_TARGET_MS: f64 = 16.0;
/// DEC private mode number for synchronous update mode (DECSET/DECRST 2026).
/// When active, the terminal suppresses rendering until an explicit sync
/// boundary is reached, batching multiple mutations into a single frame.
const SYNC_MODE_NUMBER: u16 = 2026;
pub(crate) const DEFAULT_MAX_SCROLLBACK: usize = 2000;
const ATLAS_WIDTH: u32 = 1024;
const ATLAS_HEIGHT: u32 = 1024;
const KGP_ATLAS_WIDTH: u32 = 1024;
const MAX_SURFACE_DIMENSION: u16 = 4096;
/// Default cursor blink period in milliseconds.
const DEFAULT_BLINK_SPEED_MS: u32 = 530;
/// Minimum allowed cursor blink period in milliseconds.
const BLINK_SPEED_MIN_MS: u32 = 100;
/// Maximum allowed cursor blink period in milliseconds.
const BLINK_SPEED_MAX_MS: u32 = 1000;

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
    /// Cached CellSnapshot cells from the previous frame for cell-level
    /// dirty tracking. When empty (first frame), all rows are dirty.
    prev_cells: Vec<torvox_terminal::ghostty_terminal::CellSnapshot>,
    /// Per-frame scratch buffers (instance buffer, instance cache, row-end
    /// cache, dirty-row scratch, row-end scratch) reused across frames to
    /// avoid reallocation on the render hot path.
    frame_buffers: FrameBuffers,
    /// Scroll offset used in the previous frame. When it changes, all rows
    /// are marked dirty because the grid cells shift.
    prev_scroll_offset: u32,
    /// Render height from the previous frame. When it changes (e.g. IME
    /// opens/closes), all rows are marked dirty because the cached instances
    /// were built for a different projection_height, and clean rows beyond
    /// the new projection_height would otherwise be incorrectly reused.
    prev_render_height: u32,
}

/// Per-frame scratch buffers for the cell-instance pipeline, owned as a unit
/// so the render hot path reuses allocations across frames without
/// reallocating any of the five buffers. Centralizing them behind one field
/// keeps the `AndroidSurface` field list shorter and groups the buffer
/// lifecycle (allocation, clear, resize) in one place.
#[derive(Default)]
struct FrameBuffers {
    /// Persistent instance buffer reused across frames so the per-frame cell
    /// instance `Vec` is not reallocated on every render (see
    /// `build_cell_instances_into`). `CellInstance` is `Copy`/`Pod`, so reuse
    /// via `clear` + `reserve` is safe.
    instance_buffer: Vec<torvox_renderer::gpu::CellInstance>,
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
            blink_speed_ms: DEFAULT_BLINK_SPEED_MS,
            last_blink_toggle: std::time::Instant::now(),
            cursor_style: torvox_core::cursor::CursorStyle::Block,
            render_requested: false,
            prev_cells: Vec::new(),
            frame_buffers: FrameBuffers::default(),
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

        self.font_pipeline.set_font_size_in_place(font_size);
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
                gpu.upload_atlas(self.font_pipeline.atlas_bitmap(), aw, ah, None);
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
                gpu.upload_atlas(self.font_pipeline.atlas_bitmap(), aw, ah, None);
                // The clear color defaults to the deep mocha blue; re-apply the
                // per-session theme background so a freshly created surface (or a
                // surface re-bound to a different session) never shows the wrong
                // default background.
                gpu.set_bg_color(self.theme.background);
            }
        }

        Ok(())
    }

    pub fn font_pipeline(&self) -> &FontPipeline {
        &self.font_pipeline
    }

    pub fn font_pipeline_mut(&mut self) -> &mut FontPipeline {
        &mut self.font_pipeline
    }

    pub fn gpu_mut(&mut self) -> Option<&mut GpuContext> {
        self.gpu.as_mut()
    }

    pub fn release_gpu_surface(&mut self) {
        if let Some(gpu) = &mut self.gpu {
            gpu.release_gpu_surface();
        }
    }

    pub fn has_session(&self) -> bool {
        self.session.is_some()
    }

    pub fn set_theme(&mut self, theme: torvox_core::config::Theme) {
        let (bg, fg) = (theme.background, theme.foreground);
        let ansi = theme.ansi;
        self.theme = theme;
        if let Some(ref session_arc) = self.session {
            let session = lock_or_recover(session_arc, "set_theme");
            session.terminal().set_theme(bg, fg, ansi);
        }
        if let Some(gpu) = self.gpu.as_mut() {
            gpu.set_bg_color(bg);
        }
        self.render_requested = true;
    }

    pub fn theme(&self) -> &torvox_core::config::Theme {
        &self.theme
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
    use super::render::compute_raster_scale;
    use super::session::{cell_to_line, line_to_text};
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
            torvox_terminal::ghostty_terminal::CellSnapshot {
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
            torvox_terminal::ghostty_terminal::CellSnapshot {
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
            torvox_terminal::ghostty_terminal::CellSnapshot {
                codepoint: 0x41,
                ..Default::default()
            },
            torvox_terminal::ghostty_terminal::CellSnapshot {
                codepoint: 0x42,
                ..Default::default()
            },
            torvox_terminal::ghostty_terminal::CellSnapshot {
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
        let cells = [torvox_terminal::ghostty_terminal::CellSnapshot {
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
            torvox_terminal::ghostty_terminal::CellSnapshot {
                codepoint: 0x48,
                ..Default::default()
            }, // H
            torvox_terminal::ghostty_terminal::CellSnapshot {
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
            torvox_terminal::ghostty_terminal::CellSnapshot {
                codepoint: 0x48,
                ..Default::default()
            },
            torvox_terminal::ghostty_terminal::CellSnapshot {
                codepoint: 0x69,
                ..Default::default()
            },
            torvox_terminal::ghostty_terminal::CellSnapshot {
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
        let cell = torvox_terminal::ghostty_terminal::CellSnapshot {
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
            torvox_terminal::ghostty_terminal::CellSnapshot {
                codepoint: 0x4E2D,
                ..Default::default()
            }, // 中
            torvox_terminal::ghostty_terminal::CellSnapshot {
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
        let cell = torvox_terminal::ghostty_terminal::CellSnapshot {
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
        let result = surface.render(0);
        assert!(result.is_err(), "render with no session should fail");
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
        let result = surface.render(0);
        assert!(result.is_err(), "render with no session should fail");
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
        surface.blink_phase = false;
        surface.set_blink_enabled(true);
        assert!(surface.blink_enabled, "blink should be enabled");
        assert!(
            surface.render_requested(),
            "set_blink_enabled should request render"
        );
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
        assert!((compute_raster_scale(1080, 540) - 2.0).abs() < f32::EPSILON);
        assert!((compute_raster_scale(540, 1080) - 0.5).abs() < f32::EPSILON);
        assert!((compute_raster_scale(1620, 1080) - 1.5).abs() < f32::EPSILON);
    }

    #[test]
    fn compute_raster_scale_clamps_extremes() {
        assert!((compute_raster_scale(10000, 100) - 4.0).abs() < f32::EPSILON);
        assert!((compute_raster_scale(100, 10000) - 0.5).abs() < f32::EPSILON);
        assert!((compute_raster_scale(800, 0) - 4.0).abs() < f32::EPSILON);
    }

    #[test]
    fn restore_session_nul_becomes_space_and_trims_trailing_blanks() {
        use torvox_terminal::ghostty_terminal::CellSnapshot;
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
        let trailing_blank = cell_to_line(&[], 3);
        let snapshot = torvox_core::snapshot::SessionSnapshot {
            visible_lines: vec![row_a, trailing_blank],
            scrollback_lines: vec![],
            rows: 2,
            cols: 3,
            max_scrollback: DEFAULT_MAX_SCROLLBACK,
        };
        let text = AndroidSurface::restore_session_lines_to_text(&snapshot);
        assert_eq!(text, "ab ");
    }

    #[test]
    fn restore_session_preserves_middle_blank_lines() {
        use torvox_terminal::ghostty_terminal::CellSnapshot;
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
        let snapshot = torvox_core::snapshot::SessionSnapshot {
            visible_lines: vec![row_a, middle_blank, row_b],
            scrollback_lines: vec![],
            rows: 3,
            cols: 1,
            max_scrollback: DEFAULT_MAX_SCROLLBACK,
        };
        let text = AndroidSurface::restore_session_lines_to_text(&snapshot);
        assert_eq!(text, "a\n \nb");
    }

    #[test]
    fn atlas_size_is_1024_square() {
        assert_eq!(ATLAS_WIDTH, 1024, "ATLAS_WIDTH must be 1024");
        assert_eq!(ATLAS_HEIGHT, 1024, "ATLAS_HEIGHT must be 1024");
    }

    #[test]
    fn kgp_atlas_width_matches_atlas() {
        assert_eq!(KGP_ATLAS_WIDTH, 1024, "KGP_ATLAS_WIDTH must match atlas width");
    }

    #[test]
    fn frame_time_target_is_16ms() {
        assert!(
            (FRAME_TIME_TARGET_MS - 16.0).abs() < f64::EPSILON,
            "FRAME_TIME_TARGET_MS must be 16.0 (60fps)"
        );
    }
}
