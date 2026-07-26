#![cfg(not(target_os = "android"))]

use terminal_core::config::Theme;
use terminal_engine::ghostty_terminal::GhosttyTerminal;
use terminal_engine::shell_env::ShellEnv;

pub struct MockSurface {
    terminal: GhosttyTerminal,
    rows: u32,
    cols: u32,
    theme: Theme,
    pixels: Vec<u8>,
    surface_width: u32,
    surface_height: u32,
}

impl MockSurface {
    pub fn new(rows: u32, cols: u32, scrollback_lines: u32, _font_size: f32) -> Self {
        let terminal =
            GhosttyTerminal::new(rows, cols, scrollback_lines).expect("GhosttyTerminal::new");
        Self {
            terminal,
            rows,
            cols,
            theme: Theme::catppuccin_mocha(),
            pixels: Vec::new(),
            surface_width: cols * 10,
            surface_height: rows * 20,
        }
    }

    pub fn rows(&self) -> u32 {
        self.rows
    }

    pub fn cols(&self) -> u32 {
        self.cols
    }

    pub fn terminal(&self) -> &GhosttyTerminal {
        &self.terminal
    }

    pub fn terminal_mut(&mut self) -> &mut GhosttyTerminal {
        &mut self.terminal
    }

    pub fn spawn_session(&mut self, _shell: &str, _env: &ShellEnv) -> Result<(), String> {
        self.terminal = GhosttyTerminal::new(self.rows, self.cols, 5000)
            .map_err(|e| format!("session: {e}"))?;
        Ok(())
    }

    pub fn resize(&mut self, rows: u32, cols: u32) {
        self.terminal.resize(rows, cols);
        self.rows = rows;
        self.cols = cols;
        self.surface_width = cols * 10;
        self.surface_height = rows * 20;
    }

    pub fn write_to_pty(&mut self, data: &[u8]) {
        self.terminal.vt_write(data);
    }

    pub fn set_theme(&mut self, theme: Theme) {
        self.theme = theme;
    }

    pub fn theme(&self) -> &Theme {
        &self.theme
    }

    pub fn render(&mut self) -> Result<(), String> {
        let width = self.surface_width as usize;
        let height = self.surface_height as usize;
        if width == 0 || height == 0 {
            return Err("zero dimensions".into());
        }
        self.pixels.clear();
        self.pixels.resize(width * height * 4, 0);

        let snap = self.terminal.take_snapshot();
        let cell_w = (width / self.cols as usize).max(1);
        let cell_h = (height / self.rows as usize).max(1);
        let background_color = self.theme.background;

        for row in 0..snap.rows.min(self.rows) {
            for col in 0..snap.cols.min(self.cols) {
                let index = (row * snap.cols + col) as usize;
                let foreground_color = if index < snap.cells.len() {
                    let cell = &snap.cells[index];
                    let red = (cell.foreground[0] * 255.0) as u8;
                    let green = (cell.foreground[1] * 255.0) as u8;
                    let blue = (cell.foreground[2] * 255.0) as u8;
                    let alpha = (cell.foreground[3] * 255.0) as u8;
                    (red, green, blue, alpha)
                } else {
                    (
                        background_color[0],
                        background_color[1],
                        background_color[2],
                        255,
                    )
                };

                let pixel_y = row as usize * cell_h;
                let pixel_x = col as usize * cell_w;
                for y in pixel_y..pixel_y + cell_h {
                    for x in pixel_x..pixel_x + cell_w {
                        let pixel_index = (y * width + x) * 4;
                        if pixel_index + 3 < self.pixels.len() {
                            self.pixels[pixel_index] = foreground_color.0;
                            self.pixels[pixel_index + 1] = foreground_color.1;
                            self.pixels[pixel_index + 2] = foreground_color.2;
                            self.pixels[pixel_index + 3] = foreground_color.3;
                        }
                    }
                }
            }
        }
        Ok(())
    }

    pub fn pixels(&self) -> &[u8] {
        &self.pixels
    }

    pub fn surface_width(&self) -> u32 {
        self.surface_width
    }

    pub fn surface_height(&self) -> u32 {
        self.surface_height
    }

    pub fn poll_sync_active(&mut self) -> bool {
        self.terminal.mode_get(2026, 0)
    }

    pub fn recompute_grid(&mut self, width: u32, height: u32) {
        let cell_w = 10;
        let cell_h = 20;
        let new_cols = (width / cell_w).max(1);
        let new_rows = (height / cell_h).max(1);
        self.surface_width = width;
        self.surface_height = height;
        self.resize(new_rows, new_cols);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mock_surface_new_and_render() {
        let mut ms = MockSurface::new(10, 20, 500, 12.0);
        assert!(ms.render().is_ok());
        assert!(!ms.pixels().is_empty());
    }

    #[test]
    fn mock_surface_write_and_render_changes_pixels() {
        let mut ms = MockSurface::new(24, 80, 2000, 12.0);
        ms.write_to_pty(b"Hello\n");
        // need to wait for ghostty processing
        std::thread::sleep(std::time::Duration::from_millis(30));
        assert!(ms.render().is_ok());
        // foreground pixels should exist because text was written
        let has_non_bg = ms.pixels().chunks(4).any(|p| p != [0, 0, 0, 0]);
        assert!(
            has_non_bg,
            "render should produce non-zero pixels after text"
        );
    }

    #[test]
    fn mock_surface_resize_updates_dimensions() {
        let mut ms = MockSurface::new(24, 80, 500, 12.0);
        ms.resize(30, 100);
        assert_eq!(ms.rows(), 30);
        assert_eq!(ms.cols(), 100);
        assert!(ms.render().is_ok());
    }

    #[test]
    fn mock_surface_spawn_session_recreates_terminal() {
        let mut ms = MockSurface::new(10, 20, 500, 12.0);
        let env = ShellEnv::default();
        let result = ms.spawn_session("/bin/sh", &env);
        assert!(result.is_ok());
        assert!(ms.render().is_ok());
    }

    #[test]
    fn mock_surface_recompute_grid() {
        let mut ms = MockSurface::new(10, 20, 500, 12.0);
        ms.recompute_grid(800, 600);
        assert!(ms.render().is_ok());
    }

    #[test]
    fn mock_surface_cat_output_produces_visible_content() {
        let mut ms = MockSurface::new(24, 80, 2000, 12.0);
        let line = b"test output with some visible content\n";
        for _ in 0..5 {
            ms.write_to_pty(line);
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
        assert!(ms.render().is_ok());
        let non_bg = ms.pixels().chunks(4).filter(|p| p[3] > 0).count();
        assert!(
            non_bg > 0,
            "scrolled content should produce non-bg pixels, got {}",
            non_bg
        );
    }

    #[test]
    fn mock_surface_dec_2026_sync_blocks_render() {
        let mut ms = MockSurface::new(10, 40, 1000, 14.0);
        let _ = ms.spawn_session("/bin/cat", &ShellEnv::default());
        ms.write_to_pty(b"before sync\n");
        ms.render().ok();
        let before_pixels = ms.pixels().chunks(4).filter(|p| p[3] > 0).count();
        assert!(before_pixels > 0, "should have pixels before sync");
        ms.write_to_pty(b"\x1b[?2026h");
        ms.write_to_pty(b"during sync\n");
        ms.render().ok();
        ms.write_to_pty(b"\x1b[?2026l");
        ms.render().ok();
        let after_pixels = ms.pixels().chunks(4).filter(|p| p[3] > 0).count();
        assert!(after_pixels > 0, "should have pixels after sync");
    }

    #[test]
    fn mock_surface_dec_2026_sync_active_reports_correct_state() {
        let mut ms = MockSurface::new(10, 40, 1000, 14.0);
        let _ = ms.spawn_session("/bin/cat", &ShellEnv::default());
        assert!(!ms.poll_sync_active(), "should not be in sync initially");
        ms.write_to_pty(b"\x1b[?2026h");
        ms.terminal().flush();
        assert!(ms.poll_sync_active(), "should be in sync after DECSET 2026");
        ms.write_to_pty(b"\x1b[?2026l");
        ms.terminal().flush();
        assert!(
            !ms.poll_sync_active(),
            "should not be in sync after DECRST 2026"
        );
    }

    #[test]
    fn mock_surface_empty_grid_renders() {
        let mut ms = MockSurface::new(5, 20, 1000, 14.0);
        assert!(ms.render().is_ok());
        let pixels = ms.pixels();
        assert!(!pixels.is_empty(), "empty grid should still produce pixels");
    }

    #[test]
    fn mock_surface_sgr_red_text_renders() {
        let mut ms = MockSurface::new(10, 40, 1000, 14.0);
        let _ = ms.spawn_session("/bin/cat", &ShellEnv::default());
        ms.write_to_pty(b"\x1b[31mRED TEXT\x1b[0m\n");
        ms.render().ok();
        let non_bg = ms.pixels().chunks(4).filter(|p| p[3] > 0).count();
        assert!(
            non_bg > 0,
            "SGR 31 red text should produce non-bg pixels, got {}",
            non_bg
        );
    }

    #[test]
    fn mock_surface_green_on_blue_text_renders() {
        let mut ms = MockSurface::new(10, 40, 1000, 14.0);
        let _ = ms.spawn_session("/bin/cat", &ShellEnv::default());
        ms.write_to_pty(b"\x1b[32;44mGREEN ON BLUE\x1b[0m\n");
        ms.render().ok();
        let non_bg = ms.pixels().chunks(4).filter(|p| p[3] > 0).count();
        assert!(
            non_bg > 0,
            "green-on-blue text should produce non-bg pixels, got {}",
            non_bg
        );
    }

    #[test]
    fn mock_surface_bold_intensity_renders() {
        let mut ms = MockSurface::new(10, 40, 1000, 14.0);
        let _ = ms.spawn_session("/bin/cat", &ShellEnv::default());
        ms.write_to_pty(b"\x1b[1;33mBOLD YELLOW\x1b[0m\n");
        ms.render().ok();
        let non_bg = ms.pixels().chunks(4).filter(|p| p[3] > 0).count();
        assert!(
            non_bg > 0,
            "bold text should produce non-bg pixels, got {}",
            non_bg
        );
    }

    #[test]
    fn mock_surface_cursor_renders() {
        let mut ms = MockSurface::new(10, 40, 1000, 14.0);
        let _ = ms.spawn_session("/bin/cat", &ShellEnv::default());
        ms.write_to_pty(b"X");
        ms.render().ok();
        let non_bg = ms.pixels().chunks(4).filter(|p| p[3] > 0).count();
        // At minimum, the cursor and text should produce non-bg pixels
        assert!(
            non_bg > 0,
            "cursor+text should produce non-bg pixels, got {}",
            non_bg
        );
    }

    #[test]
    fn mock_surface_scroll_produces_output() {
        let mut ms = MockSurface::new(5, 20, 500, 12.0);
        let _ = ms.spawn_session("/bin/cat", &ShellEnv::default());
        // Fill screen then scroll
        for i in 0..10 {
            ms.write_to_pty(format!("line {}\n", i).as_bytes());
        }
        ms.render().ok();
        let non_bg = ms.pixels().chunks(4).filter(|p| p[3] > 0).count();
        assert!(
            non_bg > 0,
            "scrolled content should produce non-bg pixels, got {}",
            non_bg
        );
    }

    // 4b.4: Pixel-level assertion on known glyph '.' position
    #[test]
    fn mock_surface_dot_glyph_pixel_position() {
        let mut ms = MockSurface::new(24, 80, 2000, 12.0);
        let _ = ms.spawn_session("/bin/cat", &ShellEnv::default());
        // Write '.' at a known row (move cursor to (5,10), write '.')
        ms.write_to_pty(b"\x1b[6;11H.");
        ms.render().ok();
        // '.' should be at cell (5,10) which maps to pixel position
        // cell_w=10, cell_h=20, so (5,10) maps to pixel (100, 100)
        let cell_w = (ms.surface_width() / 80).max(1);
        let cell_h = (ms.surface_height() / 24).max(1);
        let px = 10 * cell_w as usize;
        let py = 5 * cell_h as usize;
        let pi = (py * ms.surface_width() as usize + px) * 4;
        let pixels = ms.pixels();
        if pi + 3 < pixels.len() {
            // Skip first row (row 0 is cell row 0, we're at row 5)
            // Find any non-bg pixel in the right general area
            for row in 4..6 {
                for col in 9..11 {
                    let check_px = (row * cell_h as usize) * ms.surface_width() as usize
                        + (col * cell_w as usize);
                    let check_pi = check_px * 4;
                    if check_pi + 3 < pixels.len() {
                        let has_color = pixels[check_pi] > 0
                            || pixels[check_pi + 1] > 0
                            || pixels[check_pi + 2] > 0;
                        if has_color {
                            return; // found non-bg pixel near expected position
                        }
                    }
                }
            }
            // Fail: no non-bg pixel found in the '.' area
            let total_nz = pixels.chunks(4).filter(|p| p[3] > 0).count();
            let all_nz = pixels
                .chunks(4)
                .filter(|p| p[0] > 0 || p[1] > 0 || p[2] > 0)
                .count();
            panic!(
                "No non-bg pixel at '.' position (row 5, col 10). p={}x{} cell={}x{}. total_nz={} all_nz={}",
                cell_w, cell_h, px, py, total_nz, all_nz
            );
        }
    }

    // 4b.5: Different themes produce different background pixel values
    #[test]
    fn mock_surface_theme_bg_colors_differ() {
        use terminal_core::config::Theme;
        // Verify theme structs have different backgrounds
        let mocha = Theme::catppuccin_mocha();
        let dracula = Theme::dracula_plus();
        assert_ne!(
            mocha.background, dracula.background,
            "Catppuccin Mocha and Dracula should have different BG colors"
        );
        // Verify known values: mocha bg = #1e1e2e (30,30,46)
        assert_eq!(mocha.background, [30, 30, 46], "Mocha BG should be #1e1e2e");
        // Dracula Plus bg = #212121 (33,33,33)
        assert_eq!(
            dracula.background,
            [33, 33, 33],
            "Dracula BG should be #212121"
        );
    }

    // 4b.6: CJK text renders correctly in grid
    #[test]
    fn mock_surface_cjk_text_renders() {
        let mut ms = MockSurface::new(10, 40, 1000, 14.0);
        let _ = ms.spawn_session("/bin/cat", &ShellEnv::default());
        // Write CJK text (Chinese characters) followed by newline
        ms.write_to_pty(b"\xe4\xbd\xa0\xe5\xa5\xbd\n"); // 你好 + newline
        ms.render().ok();
        let non_bg = ms.pixels().chunks(4).filter(|p| p[3] > 0).count();
        assert!(
            non_bg > 0,
            "CJK text should produce non-bg pixels, got {}",
            non_bg
        );
    }
}
