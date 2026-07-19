use std::collections::HashSet;
use std::sync::atomic::Ordering;
use std::time::Instant;

use torvox_renderer::gpu::CellInstance;
use torvox_renderer::gpu::SearchHighlight;
use torvox_renderer::gpu::SelectionRange;

use torvox_terminal::ghostty_terminal::CellSnapshot;
use torvox_terminal::ghostty_terminal::KgpImageData;

use super::{AndroidSurface, SurfaceError};
use super::{FRAME_TIME_TARGET_MS, KGP_ATLAS_WIDTH};

fn color_changed(a: [f32; 4], b: [f32; 4]) -> bool {
    a[0].to_bits() != b[0].to_bits()
        || a[1].to_bits() != b[1].to_bits()
        || a[2].to_bits() != b[2].to_bits()
        || a[3].to_bits() != b[3].to_bits()
}

/// Computes which rows changed between `prev_cells` and `snapshot`, writing
/// the result into `dirty_rows` (reused to avoid reallocation each frame).
///
/// A row is dirty when any of its cells differs, when it holds the cursor
/// (current or previous position), or when a global redraw is forced by a
/// scroll-offset change, render-height change, or active search highlights.
///
/// This is pure logic extracted from the render hot path so it can be unit
/// tested without a GPU — the call site only supplies the inputs and reuses
/// the buffer; all frame submission state stays in `AndroidSurface`.
#[allow(clippy::too_many_arguments)]
fn compute_dirty_rows_into(
    dirty_rows: &mut Vec<bool>,
    prev_cells: &[CellSnapshot],
    snapshot: &torvox_terminal::ghostty_terminal::GridSnapshot,
    last_cursor_row: u32,
    scroll_offset: u32,
    prev_scroll_offset: u32,
    render_height: u32,
    prev_render_height: u32,
    has_highlights: bool,
) {
    let total_cells = (snapshot.rows * snapshot.cols) as usize;
    dirty_rows.clear();
    if prev_cells.len() == total_cells {
        dirty_rows.reserve(snapshot.rows as usize);
        for row in 0..snapshot.rows as usize {
            let row_off = row * snapshot.cols as usize;
            let mut row_dirty = false;
            for col in 0..snapshot.cols as usize {
                if cell_ne(&prev_cells[row_off + col], &snapshot.cells[row_off + col]) {
                    row_dirty = true;
                    break;
                }
            }
            dirty_rows.push(row_dirty);
        }
    } else {
        dirty_rows.resize(snapshot.rows as usize, true);
    }

    let cr = snapshot.cursor_row as usize;
    if cr < dirty_rows.len() {
        dirty_rows[cr] = true;
    }
    let pcr = last_cursor_row as usize;
    if pcr < dirty_rows.len() && pcr != cr {
        dirty_rows[pcr] = true;
    }

    let scroll_changed = prev_scroll_offset != scroll_offset;
    let render_height_changed = prev_render_height != render_height;
    if has_highlights || scroll_changed || render_height_changed {
        dirty_rows.fill(true);
    }
}

/// Compare two CellSnapshots for equality of fields that affect rendering.
/// Returns `true` if they differ (row should be marked dirty).
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
        || color_changed(a.foreground, b.foreground)
        || color_changed(a.background, b.background)
        || a.uri != b.uri
        || a.graphemes.len() != b.graphemes.len()
        || (!a.graphemes.is_empty() && a.graphemes != b.graphemes)
}

/// Requirement 4 (CJK font scale, Fix D): the glyph raster scale is the ratio of
/// the physical surface width (ANativeWindow pixels) to the wgpu surface-config
/// width (logical density). It is clamped to a sane range so a misreported
/// surface metric cannot blow up the atlas. Pure + testable.
pub(crate) fn compute_raster_scale(surface_width: u32, config_width: u32) -> f32 {
    let surface_width = surface_width.max(1);
    let config_width = config_width.max(1);
    (surface_width as f32 / config_width as f32).clamp(0.5, 4.0)
}

impl AndroidSurface {
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
        super::trace_begin(c"AndroidSurface::render");
        log::trace!(
            "RENDER_ENTER: session={} sw={} sh={} native={}",
            self.session.is_some(),
            self.surface_width.load(Ordering::Relaxed),
            self.surface_height.load(Ordering::Relaxed),
            self.native_window.is_some(),
        );

        let has_search_highlights = !self.search_highlights.is_empty();
        if has_search_highlights {
            log::debug!(
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
            super::trace_end(); // AndroidSurface::render
            return Ok(false);
        }
        log::debug!(
            "RENDER_PROCEED: had_output={} frame_count={} highlights={} render_requested={}",
            had_output,
            self.frame_count,
            has_search_highlights,
            self.render_requested,
        );

        super::trace_begin(c"snapshot+instances");

        if let (Some(row), Some(col)) = (self.mouse_row, self.mouse_col) {
            self.last_hovered_url = snapshot.uri_at(row, col).map(String::from);
        }

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
        let mut dirty_rows = std::mem::take(&mut self.frame_buffers.dirty_rows_buf);
        if snapshot.cursor_col != self.last_cursor_col {
            self.blink_phase = true;
            self.last_blink_toggle = std::time::Instant::now();
        }
        let highlights_present = !self.search_highlights.is_empty();
        compute_dirty_rows_into(
            &mut dirty_rows,
            &self.prev_cells,
            &snapshot,
            self.last_cursor_row,
            scroll_offset,
            self.prev_scroll_offset,
            self.render_height,
            self.prev_render_height,
            highlights_present,
        );

        let now = std::time::Instant::now();
        if self.blink_timer_elapsed() {
            self.blink_phase = !self.blink_phase;
            self.last_blink_toggle = now;
        }

        if self.blink_enabled && !self.blink_phase && snapshot.cursor_visible {
            snapshot.cursor_visible = false;
        }

        if self.prev_cells.len() == (snapshot.rows * snapshot.cols) as usize
            && dirty_rows.iter().any(|&d| !d)
        {
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

        let mut row_ends = std::mem::take(&mut self.frame_buffers.row_ends_buf);
        row_ends.clear();
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
                cached_instances: &self.frame_buffers.cached_instances,
                cached_row_ends: &self.frame_buffers.cached_row_ends,
            },
            &mut self.frame_buffers.instance_buffer,
            &mut row_ends,
        );
        self.frame_buffers.dirty_rows_buf = dirty_rows;

        let instances = &self.frame_buffers.instance_buffer[..];

        let diag_has_glyph = |i: &CellInstance| i.atlas_size[0] > 0.0 && i.atlas_size[1] > 0.0;
        let diag_first = instances.iter().find(|i| {
            i.fg_color[0].abs() > 0.001
                || i.fg_color[1].abs() > 0.001
                || i.fg_color[2].abs() > 0.001
        });
        if let Some(diag) = diag_first {
            if self.frame_count.is_multiple_of(60) {
                log::debug!(
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
            log::debug!(
                "RENDER_DIAG: {} instances all have black fg",
                instances.len()
            );
            log::debug!(
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
            log::debug!("RENDER_DIAG: zero instances");
        }

        let gen_after = self.font_pipeline.atlas_generation();
        if self.frame_count.is_multiple_of(60) {
            log::debug!(
                "RENDER_DIAG: atlas gen {}->{}, {} instances",
                gen_before,
                gen_after,
                instances.len(),
            );
        }
        if gen_after != gen_before {
            let dirty = self.font_pipeline.take_dirty_rect();
            self.gpu
                .as_mut()
                .ok_or_else(|| {
                    SurfaceError::GpuInit("GPU not initialized during atlas upload".into())
                })?
                .upload_atlas(
                    self.font_pipeline.atlas_bitmap(),
                    self.atlas_width,
                    self.atlas_height,
                    dirty,
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

        super::trace_end(); // snapshot+instances
        super::trace_begin(c"swapchain_present");

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

        let cpu_work_end = Instant::now();
        let cpu_ms = cpu_work_end.duration_since(frame_start).as_secs_f64() * 1000.0;

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

        #[cfg(target_os = "android")]
        let swapchain_ok = {
            if sync_active {
                log::trace!("sync active — skipping GPU frame");
                super::trace_end(); // swapchain_present
                super::trace_end(); // AndroidSurface::render
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

        super::trace_end(); // swapchain_present

        let elapsed = frame_start.elapsed();
        let present_ms = elapsed.as_secs_f64() * 1000.0 - cpu_ms;
        if cpu_ms >= FRAME_TIME_TARGET_MS {
            log::warn!(
                "RENDER_SLOW: cpu={:.1}ms present={:.1}ms",
                cpu_ms,
                present_ms
            );
        } else {
            log::debug!("RENDER_OK: cpu={:.1}ms present={:.1}ms", cpu_ms, present_ms);
        }

        std::mem::swap(
            &mut self.frame_buffers.cached_instances,
            &mut self.frame_buffers.instance_buffer,
        );
        self.frame_buffers.cached_row_ends = row_ends;
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
            &mut self.frame_buffers.instance_buffer,
            &mut Vec::new(),
        );
        let instances = &self.frame_buffers.instance_buffer[..];
        let gpu = self
            .gpu
            .as_mut()
            .ok_or_else(|| SurfaceError::GpuInit("GPU not initialized for test frame".into()))?;
        gpu.upload_atlas(
            self.font_pipeline.atlas_bitmap(),
            self.atlas_width,
            self.atlas_height,
            None,
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

    /// Force the next `render()` call to actually present a frame even when
    /// there is no new PTY output. Used for view-only state changes such as
    /// scrolling, where `take_snapshot_with_scroll` must run to shift the
    /// displayed rows but `had_output` is false. Without this, `render()`
    /// early-returns at the idle skip and the viewport never scrolls.
    pub fn set_render_requested(&mut self, value: bool) {
        self.render_requested = value;
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

    #[cfg(test)]
    pub fn render_requested(&self) -> bool {
        self.render_requested
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use torvox_terminal::ghostty_terminal::GridSnapshot;

    fn snapshot(rows: u32, cols: u32) -> GridSnapshot {
        GridSnapshot::fallback(rows, cols)
    }

    #[test]
    fn identical_frames_no_dirty_rows() {
        let a = snapshot(3, 3);
        let mut b = snapshot(3, 3);
        b.cursor_row = 99;
        let mut buf = Vec::new();
        compute_dirty_rows_into(&mut buf, &a.cells, &b, 99, 0, 0, 0, 0, false);
        assert_eq!(buf, vec![false, false, false]);
    }

    #[test]
    fn changed_cell_marks_row_dirty() {
        let a = snapshot(3, 3);
        let mut b = snapshot(3, 3);
        b.cursor_row = 99;
        b.cells[1 * 3 + 1].codepoint = 'X' as u32;
        let mut buf = Vec::new();
        compute_dirty_rows_into(&mut buf, &a.cells, &b, 99, 0, 0, 0, 0, false);
        assert_eq!(buf, vec![false, true, false]);
    }

    #[test]
    fn cursor_row_always_dirty() {
        let a = snapshot(3, 3);
        let mut b = snapshot(3, 3);
        b.cursor_row = 1;
        let mut buf = Vec::new();
        compute_dirty_rows_into(&mut buf, &a.cells, &b, 0, 0, 0, 0, 0, false);
        assert!(buf[1]);
        assert!(buf[0]);
        assert!(!buf[2]);
    }

    #[test]
    fn scroll_change_forces_all_dirty() {
        let a = snapshot(3, 3);
        let b = snapshot(3, 3);
        let mut buf = Vec::new();
        compute_dirty_rows_into(&mut buf, &a.cells, &b, 0, 5, 0, 0, 0, false);
        assert_eq!(buf, vec![true, true, true]);
    }

    #[test]
    fn size_mismatch_forces_all_dirty() {
        let a = snapshot(3, 3);
        let b = snapshot(2, 2);
        let mut buf = Vec::new();
        compute_dirty_rows_into(&mut buf, &a.cells, &b, 0, 0, 0, 0, 0, false);
        assert_eq!(buf, vec![true, true]);
    }
}
