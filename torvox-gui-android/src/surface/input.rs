use std::sync::atomic::Ordering;

use super::AndroidSurface;
use super::{BLINK_SPEED_MAX_MS, BLINK_SPEED_MIN_MS, MAX_COLS, MAX_ROWS, MIN_COLS, MIN_ROWS};

impl AndroidSurface {
    pub(crate) fn blink_period(&self) -> std::time::Duration {
        std::time::Duration::from_millis(self.blink_speed_ms as u64)
    }

    pub(super) fn blink_timer_elapsed(&self) -> bool {
        self.blink_enabled && self.last_blink_toggle.elapsed() >= self.blink_period()
    }

    pub fn set_blink_enabled(&mut self, enabled: bool) {
        self.blink_enabled = enabled;
        if !enabled {
            self.blink_phase = true;
        }
        self.render_requested = true;
    }

    pub fn set_blink_speed_ms(&mut self, speed_ms: u32) {
        self.blink_speed_ms = speed_ms.clamp(BLINK_SPEED_MIN_MS, BLINK_SPEED_MAX_MS);
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
                    gpu.upload_atlas(self.font_pipeline.atlas_bitmap(), aw, ah, None);
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
            gpu.upload_atlas(self.font_pipeline.atlas_bitmap(), aw, ah, None);
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
            gpu.upload_atlas(self.font_pipeline.atlas_bitmap(), aw, ah, None);
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
}
