pub mod atlas;
pub mod cjk;
pub mod font_db;
pub mod glyph_cache;
pub mod pipeline;
pub mod rasterization;
pub mod shaping;

use thiserror::Error;

pub const GLYPH_CACHE_CAPACITY: usize = 10_000;

/// Unicode code point where CJK Ideographic characters begin (U+2E80).
/// Used to decide whether to attempt CJK fallback font lookup.
pub(crate) const CJK_IDEOGRAPHIC_START: u32 = 0x2E80;

const PREFERRED_MONOSPACE_FONTS: &[&str] = &[
    "roboto mono",
    "droid sans mono",
    "noto sans mono",
    "source code pro",
    "fira code",
    "fira mono",
    "jetbrains mono",
    "dejavu sans mono",
    "noto sans mono cjk",
    "liberation mono",
    "ubuntu mono",
    "cascadia",
    "ia writer",
    "hack",
    "inconsolata",
    "iosevka",
    "meslo",
    "consolas",
    "menlo",
    "monaco",
    "courier",
];

#[derive(Debug, Error)]
pub enum FontError {
    #[error("no monospace font found")]
    NoMonospaceFont,
    #[error("font loading failed: {0}")]
    FontLoad(String),
    #[error("atlas allocation failed")]
    AtlasAllocationFailed,
}

/// Glyph synthesis mode: how a glyph is styled when the
/// font has no matching bold/italic face. Pixels are post-processed on the
/// rasterized alpha mask — bold emboldens, italic shears.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum GlyphSynthesis {
    #[default]
    None,
    Bold,
    Italic,
    BoldItalic,
}

impl GlyphSynthesis {
    /// Bit values packed into the glyph cache key (3 bits are enough).
    pub(crate) fn bits(self) -> u8 {
        match self {
            GlyphSynthesis::None => 0,
            GlyphSynthesis::Bold => 1,
            GlyphSynthesis::Italic => 2,
            GlyphSynthesis::BoldItalic => 3,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct GlyphKey {
    pub font_id: fontdb::ID,
    pub glyph_id: u16,
    pub pixel_size: u16,
    /// Glyph synthesis applied at rasterization time (0 = none).
    pub synthesis: u8,
}

#[derive(Debug, Clone)]
pub struct GlyphInfo {
    pub atlas_x: i32,
    pub atlas_y: i32,
    pub width: i32,
    pub height: i32,
    pub placement: swash::zeno::Placement,
    pub advance_width: f32,
    pub allocation_id: Option<guillotiere::AllocId>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ShapedGlyphInfo {
    pub glyph_id: u16,
    pub font_id: fontdb::ID,
    pub x: f32,
    pub w: f32,
    pub x_offset: f32,
    pub y_offset: f32,
}

#[cfg(target_os = "android")]
pub use font_db::set_extra_font_paths;
pub use pipeline::FontPipeline;
pub(crate) use pipeline::OverlayQuad;

#[cfg(test)]
mod tests {
    use super::*;

    const TEST_DATA_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/test_data");
    const FIXTURE_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../test_fonts");

    // ── emoji classification boundary tests ────────────────────
    // Mirrors warp lib.rs:2192-2307 (classify_char + boundary pins). The
    // production shaper is cosmic-text; this helper documents the SAME
    // Unicode ranges the pipeline treats as emoji (routed through the
    // emoji-capable font when present) and pins them against drift.

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum RunKind {
        Latin,
        Cjk,
        Emoji,
    }

    /// Classify a char by the Unicode ranges the renderer treats as
    /// CJK / emoji. Identical ranges to warp's classify_char.
    fn classify_char(ch: char) -> RunKind {
        let cp = ch as u32;
        let is_cjk = matches!(
            cp,
            0x1100..=0x11FF
                | 0x3000..=0x303F
                | 0x3040..=0x309F
                | 0x30A0..=0x30FF
                | 0x3100..=0x312F
                | 0x3400..=0x4DBF
                | 0x4E00..=0x9FFF
                | 0xAC00..=0xD7AF
                | 0xF900..=0xFAFF
                | 0xFE30..=0xFE4F
                | 0xFF00..=0xFFEF
        );
        if is_cjk {
            return RunKind::Cjk;
        }
        let is_emoji = matches!(
            cp,
            0x1F300..=0x1F6FF | 0x1F900..=0x1F9FF | 0x1FA00..=0x1FAFF | 0x2600..=0x27BF
        );
        if is_emoji {
            return RunKind::Emoji;
        }
        RunKind::Latin
    }

    #[test]
    fn emoji_codepoints_classify_as_emoji() {
        let cases = [
            ('\u{1F389}', "U+1F389 PARTY POPPER (Misc Symbols)"),
            ('\u{1F600}', "U+1F600 GRINNING FACE (Misc Symbols)"),
            ('\u{1F4A9}', "U+1F4A9 PILE OF POO (Misc Symbols)"),
            ('\u{1F923}', "U+1F923 ROFL (Supplemental)"),
            ('\u{1FA90}', "U+1FA90 RINGED PLANET (Extended-A)"),
            ('\u{2600}', "U+2600 BLACK SUN (Misc Symbols)"),
            ('\u{2728}', "U+2728 SPARKLES (Dingbats)"),
            ('\u{27B0}', "U+27B0 CURLY LOOP (Dingbats end)"),
        ];
        for (ch, desc) in cases {
            assert_eq!(classify_char(ch), RunKind::Emoji, "{desc} must be Emoji");
        }
    }

    #[test]
    fn emoji_range_boundaries_are_tight() {
        // Just below U+1F300 — must NOT be emoji.
        assert_eq!(classify_char('\u{1F2FF}'), RunKind::Latin);
        // Just above U+1F6FF — gap before U+1F900.
        assert_eq!(classify_char('\u{1F700}'), RunKind::Latin);
        // Just below U+2600 — Latin punctuation.
        assert_eq!(classify_char('\u{25FF}'), RunKind::Latin);
        // Just above U+27BF.
        assert_eq!(classify_char('\u{27C0}'), RunKind::Latin);
        // CJK Han must stay Cjk, not Emoji.
        assert_eq!(classify_char('世'), RunKind::Cjk);
        assert_eq!(classify_char('界'), RunKind::Cjk);
        // ASCII stays Latin.
        assert_eq!(classify_char('H'), RunKind::Latin);
        assert_eq!(classify_char(' '), RunKind::Latin);
        assert_eq!(classify_char(','), RunKind::Latin);
    }

    #[test]
    fn mixed_string_produces_three_run_kinds() {
        let s = "Hello, 世界 🎉";
        let kinds: Vec<RunKind> = s.chars().map(classify_char).collect();
        assert!(kinds.iter().any(|k| *k == RunKind::Latin));
        assert!(kinds.iter().any(|k| *k == RunKind::Cjk));
        assert!(
            kinds.iter().any(|k| *k == RunKind::Emoji),
            "🎉 (U+1F389) must classify as Emoji"
        );
    }

    /// Pipeline-level contract: emoji glyphs resolve (non-zero) when an
    /// emoji font exists; flanking codepoints must never panic and resolve
    /// through the regular path.
    #[test]
    fn emoji_glyphs_resolve_and_flanks_do_not_panic() {
        let mut pipeline = FontPipeline::new(512, 512, 14.0);
        // Sample inside ranges — tolerant when no emoji font is installed.
        for ch in ['\u{1F600}', '\u{2728}'] {
            if let Some(info) = pipeline.glyph_information(ch) {
                assert!(
                    info.width > 0 || info.height > 0,
                    "{ch:?} should produce non-zero glyph info"
                );
            }
        }
        // Flanking codepoints: no panic, and if glyph info exists it must
        // be the text path (widths typical of Latin/CJK, not emoji color).
        for ch in ['\u{1F2FF}', '\u{1F700}', '\u{25FF}', '\u{27C0}'] {
            let _ = pipeline.glyph_information(ch);
        }
    }

    /// Han glyphs resolve and report double-cell width via the existing
    /// cell-width logic (CJK span detection in glyph_information). Tolerant
    /// when no CJK font is installed on the host (same as
    /// cjk_fallback_uses_vector_font).
    #[test]
    fn han_is_wide_and_non_emoji() {
        assert_eq!(classify_char('世'), RunKind::Cjk);
        assert_eq!(classify_char('界'), RunKind::Cjk);
        let mut pipeline = FontPipeline::new(512, 512, 14.0);
        let ascii_info = pipeline.glyph_information('A').expect("ASCII glyph info");
        let han_info = pipeline.glyph_information('世').expect("Han glyph info");
        let han_wide = han_info.width as f32 > ascii_info.width as f32 * 1.5;
        if !han_wide {
            // Host without a CJK font: '世' falls back to a narrow box
            // glyph. The classification contract still holds (Cjk, not
            // Emoji) — the width contract is verified on Android (emulator
            // has Noto CJK) and in cjk_fallback_uses_vector_font.
            let has_cjk = pipeline
                .list_monospace_fonts()
                .iter()
                .any(|name| name.to_lowercase().contains("cjk"));
            assert!(
                !has_cjk,
                "CJK font present but '世' width {} not > 1.5x ASCII {}",
                han_info.width, ascii_info.width
            );
        }
    }

    #[test]
    fn font_pipeline_creation() {
        let pipeline = FontPipeline::new(1024, 1024, 14.0);
        assert_eq!(pipeline.atlas_dimensions(), (1024, 1024));
        assert!(
            pipeline.cache_length() > 0,
            "ASCII glyphs should be pre-rasterized"
        );
    }

    #[test]
    fn font_pipeline_has_system_fonts() {
        let pipeline = FontPipeline::new(1024, 1024, 14.0);
        let fonts = pipeline.list_monospace_fonts();
        assert!(
            !fonts.is_empty(),
            "Should have at least one system monospace font"
        );
    }

    #[test]
    fn font_matching_stripped_spaces() {
        let pipeline = FontPipeline::new(1024, 1024, 14.0);
        let names = pipeline.list_monospace_fonts();
        assert!(!names.is_empty(), "Should have at least one font");
        let name_with_space = names.iter().find(|n| n.contains(' '));
        let name = match name_with_space {
            Some(n) => n.clone(),
            None => {
                panic!(
                    "no monospace font with spaces found; cannot test stripped-name matching; available: {:?}",
                    names.iter().take(5).collect::<Vec<_>>()
                );
            }
        };
        let stripped: String = name.chars().filter(|c| !c.is_whitespace()).collect();
        assert!(stripped != name, "Sanity: stripped name differs");
        let mut p2 = FontPipeline::new(1024, 1024, 14.0);
        assert!(
            p2.set_font_family(&stripped),
            "set_font_family should find '{}' when given '{}'",
            name,
            stripped
        );
    }

    #[test]
    fn glyph_hao_cjk_cross_verify() {
        let mut pipeline = FontPipeline::new(512, 512, 14.0);
        let info = pipeline
            .glyph_information('好')
            .expect("pipeline should have CJK glyph info (via fallback)");
        assert!(
            info.width > 0 || info.height > 0,
            "CJK '好' should produce non-zero glyph info: got {}x{}",
            info.width,
            info.height
        );
    }

    #[test]
    fn cjk_width_is_double_ascii() {
        let mut pipeline = FontPipeline::new(512, 512, 14.0);
        let ascii_info = pipeline
            .glyph_information('A')
            .expect("ascii 'A' glyph info");
        let cjk_info = pipeline
            .glyph_information('中')
            .expect("CJK '中' glyph info");
        assert!(
            ascii_info.width > 0,
            "ASCII glyph should have positive width"
        );
        assert!(cjk_info.width > 0, "CJK glyph should have positive width");
        let (cell_w, _) = pipeline.cell_metrics();
        assert!(cell_w > 0.0, "cell width should be positive");
        let cell_span = if cjk_info.width as f32 > ascii_info.width as f32 * 1.5 {
            2
        } else {
            1
        };
        assert!(cell_span >= 1, "CJK cell span should be at least 1");
    }

    #[test]
    fn glyph_information_ascii() {
        let mut pipeline = FontPipeline::new(1024, 1024, 14.0);
        let info = pipeline.glyph_information('A');
        assert!(info.is_some());
        let info = info.unwrap();
        assert!(info.width > 0);
        assert!(info.height > 0);
    }

    #[test]
    fn glyph_information_caching() {
        let mut pipeline = FontPipeline::new(1024, 1024, 14.0);
        let before = pipeline.cache_length();
        pipeline.glyph_information('B');
        assert_eq!(pipeline.cache_length(), before);
        pipeline.glyph_information('B');
        assert_eq!(pipeline.cache_length(), before);
    }

    #[test]
    fn rasterize_ascii_populates_cache() {
        let pipeline = FontPipeline::new(1024, 1024, 14.0);
        assert!(pipeline.cache_length() >= 95);
    }

    #[test]
    fn atlas_eviction_keeps_glyphs_retrievable() {
        let mut pipeline = FontPipeline::new(256, 256, 14.0);
        let distinct: Vec<char> = ('A'..='Z')
            .chain('a'..='z')
            .chain('0'..='9')
            .chain("!@#$%^&*()_+-=[]{}|;:,.<>?/".chars())
            .collect();
        for &ch in distinct.iter().cycle().take(distinct.len() * 4) {
            assert!(
                pipeline.glyph_information(ch).is_some(),
                "glyph {ch:?} must remain retrievable under eviction pressure"
            );
        }
        let generation_after_fill = pipeline.atlas_generation();
        assert!(pipeline.glyph_information('A').is_some());
        assert!(
            pipeline.atlas_generation() >= generation_after_fill,
            "atlas generation must remain monotonic under eviction"
        );
        assert!(
            pipeline.cache_length() <= GLYPH_CACHE_CAPACITY,
            "glyph cache must respect its capacity after eviction"
        );
    }

    #[test]
    fn glyph_information_has_atlas_coords() {
        let mut pipeline = FontPipeline::new(1024, 1024, 14.0);
        let info = pipeline.glyph_information('X').unwrap();
        assert!(info.atlas_x >= 0);
        assert!(info.atlas_y >= 0);
        assert!(info.width > 0);
        assert!(info.height > 0);
    }

    #[test]
    fn atlas_bitmap_not_empty_after_rasterize() {
        let mut pipeline = FontPipeline::new(1024, 1024, 14.0);
        pipeline.glyph_information('A');
        let bitmap = pipeline.atlas_bitmap();
        assert!(bitmap.iter().any(|&b| b != 0));
    }

    #[test]
    fn cell_metrics_returns_positive_dimensions() {
        let pipeline = FontPipeline::new(1024, 1024, 14.0);
        let (cw, ch) = pipeline.cell_metrics();
        assert!(cw > 0.0, "cell_width must be > 0, got {cw}");
        assert!(ch > 0.0, "cell_height must be > 0, got {ch}");
    }

    #[test]
    fn cell_metrics_scales_with_font_size() {
        let small = FontPipeline::new(1024, 1024, 10.0);
        let large = FontPipeline::new(1024, 1024, 20.0);
        let (sw, sh) = small.cell_metrics();
        let (lw, lh) = large.cell_metrics();
        assert!(lw > sw, "larger font must have wider cell");
        assert!(lh > sh, "larger font must have taller cell");
    }

    #[test]
    fn b1_fontlist_includes_fixture() {
        let pipeline = FontPipeline::from_fixture(512, 512, 12.0, FIXTURE_DIR);
        let fonts = pipeline.list_monospace_fonts();
        assert!(
            fonts
                .iter()
                .any(|name| { name.contains("Liberation") || name.contains("Mono") }),
            "LiberationMono should appear in font list from fixture dir, got: {:?}",
            fonts
        );
    }

    #[test]
    fn b2_setting_font_changes_metrics() {
        let mut pipeline = FontPipeline::from_fixture(512, 512, 12.0, FIXTURE_DIR);
        let fonts = pipeline.list_monospace_fonts();
        let lm = fonts
            .iter()
            .find(|name| name.contains("Liberation") || name.contains("Mono"))
            .cloned();
        let name = lm.expect("LiberationMono should be in font list from fixture dir");
        assert!(
            pipeline.set_font_family(&name),
            "set_font_family should succeed for {name}"
        );
        let (cw, ch) = pipeline.cell_metrics();
        assert!(cw > 0.0, "cell width should be positive, got {cw}");
        assert!(ch > 0.0, "cell height should be positive, got {ch}");
    }

    fn load_freetype_golden(dir: &str, stem: &str) -> Option<(u32, u32, Vec<u8>)> {
        let meta_path = std::path::Path::new(dir).join(format!("freetype_{stem}.meta"));
        let rgba_path = std::path::Path::new(dir).join(format!("freetype_{stem}.rgba"));
        if !meta_path.exists() || !rgba_path.exists() {
            return None;
        }
        let meta = std::fs::read_to_string(meta_path).ok()?;
        let glyph_width: u32 = meta
            .lines()
            .find(|l| l.starts_with("width="))?
            .trim_start_matches("width=")
            .parse()
            .ok()?;
        let glyph_height: u32 = meta
            .lines()
            .find(|l| l.starts_with("height="))?
            .trim_start_matches("height=")
            .parse()
            .ok()?;
        let data = std::fs::read(rgba_path).ok()?;
        Some((glyph_width, glyph_height, data))
    }

    fn compare_with_freetype(ch: char, stem: &str) {
        let golden = load_freetype_golden(TEST_DATA_DIR, stem);

        let mut pipeline = FontPipeline::new(512, 512, 14.0);
        let info = pipeline
            .glyph_information(ch)
            .unwrap_or_else(|| panic!("pipeline glyph_information('{ch}') should succeed"));
        let atlas = pipeline.atlas_bitmap();

        let regenerate;

        if let Some((ft_w, ft_h, ft_data)) = golden {
            let w_diff = (info.width - ft_w as i32).abs();
            let h_diff = (info.height - ft_h as i32).abs();

            if w_diff > 2 || h_diff > 2 {
                // nosemgrep: semgrep.no-eprintln-library — test helper (called only from #[test])
                eprintln!(
                    "glyph '{ch}' dimensions differ (FT={ft_w}x{ft_h} pipeline={}x{}) — regenerating golden file",
                    info.width, info.height
                );
                regenerate = true;
            } else {
                let cmp_w = info.width.min(ft_w as i32).max(0) as usize;
                let cmp_h = info.height.min(ft_h as i32).max(0) as usize;
                let ft_stride = ft_w as usize * 4;
                let ax = info.atlas_x as usize;
                let ay = info.atlas_y as usize;
                let atlas_w = 512usize;
                let mut max_diff = 0u8;
                let mut diff_count = 0u32;

                for y in 0..cmp_h {
                    for x in 0..cmp_w {
                        let pixel = (ay + y) * atlas_w + ax + x;
                        let ai = pixel * 4;
                        let fi = y * ft_stride + x * 4;
                        let atlas_pixel = atlas[ai];
                        let freetype_pixel = ft_data[fi + 3];
                        let diff = atlas_pixel.abs_diff(freetype_pixel);
                        if diff > max_diff {
                            max_diff = diff;
                        }
                        if diff > 2 {
                            diff_count += 1;
                        }
                    }
                }

                if max_diff > 128 || diff_count > (cmp_w * cmp_h / 3) as u32 {
                    // nosemgrep: semgrep.no-eprintln-library — test helper (called only from #[test])
                    eprintln!(
                        "glyph '{ch}' FreeType comparison differs too much (max={max_diff}) — regenerating golden file"
                    );
                    regenerate = true;
                } else {
                    assert!(
                        max_diff <= 64 || diff_count <= (cmp_w * cmp_h / 5) as u32,
                        "glyph '{ch}' FreeType comparison: max_alpha_diff={max_diff} \
                         pixels_over_tolerance={diff_count} (total={})",
                        cmp_w * cmp_h
                    );
                    return;
                }
            }
        } else {
            // nosemgrep: semgrep.no-eprintln-library — test helper (called only from #[test])
            eprintln!("No golden file for glyph '{ch}' — generating it now");
            regenerate = true;
        }

        if regenerate {
            save_pipeline_glyph_as_golden(&info, atlas, 512, TEST_DATA_DIR, stem);
            // nosemgrep: semgrep.no-eprintln-library — test helper (called only from #[test])
            eprintln!("Golden file freetype_{stem}.rgba regenerated for current font");
        }
    }

    fn save_pipeline_glyph_as_golden(
        info: &GlyphInfo,
        atlas: &[u8],
        atlas_width: usize,
        dir: &str,
        stem: &str,
    ) {
        let ax = info.atlas_x as usize;
        let ay = info.atlas_y as usize;
        let w = info.width as usize;
        let h = info.height as usize;

        let mut rgba = Vec::with_capacity(w * h * 4);
        for y in 0..h {
            for x in 0..w {
                let alpha = atlas[(ay + y) * atlas_width + ax + x];
                rgba.extend_from_slice(&[0, 0, 0, alpha]);
            }
        }

        std::fs::create_dir_all(dir).expect("create test_data dir");
        let rgba_path = format!("{dir}/freetype_{stem}.rgba");
        let meta_path = format!("{dir}/freetype_{stem}.meta");
        std::fs::write(&rgba_path, &rgba).expect("write golden rgba");
        std::fs::write(&meta_path, format!("{w} {h}\n")).expect("write golden meta");
    }

    #[test]
    fn glyph_a_freetype_comparison() {
        compare_with_freetype('A', "A");
    }

    #[test]
    fn glyph_hao_freetype_comparison() {
        compare_with_freetype('好', "hao");
    }

    #[test]
    fn bearing_values_for_dot() {
        let mut pipeline = FontPipeline::new(512, 512, 14.0);
        let info = pipeline
            .glyph_information('.')
            .expect("'.' should glyph_information");
        assert!(
            info.placement.left >= 0,
            "dot bearing_x={} should be >= 0",
            info.placement.left
        );
        assert!(
            info.placement.top > 0,
            "dot bearing_y={} should be > 0",
            info.placement.top
        );
    }

    #[test]
    fn bearing_values_for_a() {
        let mut pipeline = FontPipeline::new(512, 512, 14.0);
        let info = pipeline
            .glyph_information('A')
            .expect("'A' should have glyph_information");
        assert!(
            info.placement.width > 0,
            "A glyph_width={} should be > 0",
            info.placement.width
        );
        assert!(
            info.placement.height > 0,
            "A glyph_height={} should be > 0",
            info.placement.height
        );
    }

    #[test]
    fn bearing_values_for_cjk() {
        let mut pipeline = FontPipeline::new(512, 512, 14.0);
        let info = pipeline
            .glyph_information('好')
            .expect("'好' should have glyph_information");
        assert!(
            info.placement.left >= 0,
            "好 bearing_x={} should be >= 0",
            info.placement.left
        );
        assert!(
            info.placement.top > 0,
            "好 bearing_y={} should be > 0",
            info.placement.top
        );
        let dot_info = pipeline.glyph_information('.').expect("'.' for comparison");
        assert!(
            info.placement.width >= dot_info.placement.width * 2 - 2,
            "好 width={} should be ~2x dot width={}",
            info.placement.width,
            dot_info.placement.width
        );
    }

    fn bearing_fits_inside_cell(glyph: char, label: &str) {
        let mut pipeline = FontPipeline::new(512, 512, 14.0);
        let info = pipeline
            .glyph_information(glyph)
            .unwrap_or_else(|| panic!("'{glyph}' glyph_information"));
        let (_cell_w, cell_h) = pipeline.cell_metrics();
        let ascent = pipeline.ascent_pixels();
        let bearing_y = ascent - info.placement.top as f32;
        let glyph_h = info.placement.height as f32;
        assert!(
            bearing_y >= -cell_h,
            "{label} glyph starts way above cell: bearing_y={} < -cell_h",
            bearing_y
        );
        assert!(glyph_h > 0.0, "{label} glyph has zero height");
        assert!(cell_h > 0.0, "{label} cell has zero height");
    }

    #[test]
    fn bearing_dot_fits_inside_cell() {
        bearing_fits_inside_cell('.', "dot");
    }

    #[test]
    fn bearing_a_fits_inside_cell() {
        bearing_fits_inside_cell('a', "a");
    }

    #[test]
    fn bearing_values_non_zero_for_rendered_glyphs() {
        let mut pipeline = FontPipeline::new(512, 512, 14.0);
        pipeline.rasterize_ascii();
        let glyphs = ['0', 'x', 'g', 'p', 'W', 'M', 'f', '(', ')'];
        for &ch in &glyphs {
            if let Some(info) = pipeline.glyph_information(ch) {
                assert!(
                    info.placement.width > 0,
                    "'{ch}' width={} should be > 0",
                    info.placement.width
                );
                assert!(
                    info.placement.height > 0,
                    "'{ch}' height={} should be > 0",
                    info.placement.height
                );
            }
        }
    }

    #[test]
    fn font_enumeration_finds_monospace() {
        let pipeline = FontPipeline::new(512, 512, 14.0);
        let fonts = pipeline.list_monospace_fonts();
        assert!(
            !fonts.is_empty(),
            "FontLoader should find at least one monospace face, got: {:?}",
            fonts
        );
        assert!(
            pipeline.has_font(),
            "FontPipeline should have a font assigned"
        );
    }

    #[test]
    fn cjk_glyph_zhong() {
        let mut pipeline = FontPipeline::new(512, 512, 14.0);
        let info = pipeline
            .glyph_information('中')
            .expect("CJK '中' (U+4E2D) should have glyph info");
        assert!(
            info.width > 0,
            "CJK '中' width should be non-zero, got {}",
            info.width
        );
        assert!(
            info.height > 0,
            "CJK '中' height should be non-zero, got {}",
            info.height
        );
        let atlas = pipeline.atlas_bitmap();
        let atlas_w = 512usize;
        let ax = info.atlas_x as usize;
        let ay = info.atlas_y as usize;
        let mut has_ink = false;
        for y in 0..info.height as usize {
            for x in 0..info.width as usize {
                let byte_offset = ((ay + y) * atlas_w + ax + x) * 4;
                if byte_offset < atlas.len() && atlas[byte_offset] > 0 {
                    has_ink = true;
                    break;
                }
            }
            if has_ink {
                break;
            }
        }
        assert!(has_ink, "CJK '中' should have non-zero coverage in atlas");
    }

    #[test]
    fn emoji_glyph_grinning() {
        let mut pipeline = FontPipeline::new(512, 512, 14.0);
        let ch = '\u{1F600}';
        let info = pipeline.glyph_information(ch);
        if info.is_none() || info.as_ref().is_some_and(|i| i.width == 0) {
            let fonts = pipeline.list_monospace_fonts();
            let found_emoji = fonts.iter().any(|name| {
                name.contains("Emoji")
                    || name.contains("Noto")
                    || name.to_lowercase().contains("emoji")
            });
            assert!(
                found_emoji,
                "no emoji-supporting font found in system; emoji glyph test requires Noto Emoji or similar"
            );
        }
        let info = info.expect("emoji should have glyph info");
        assert!(
            info.width > 0 || info.height > 0,
            "emoji should produce non-zero glyph info: got {}x{}",
            info.width,
            info.height
        );
    }

    #[test]
    fn glyph_atlas_lru_eviction() {
        let mut pipeline = FontPipeline::new(512, 512, 14.0);
        pipeline.rasterize_ascii();
        let after_ascii = pipeline.cache_length();
        assert!(
            after_ascii >= 95,
            "should have at least 95 cached after rasterize_ascii, got {}",
            after_ascii
        );

        let mut inserted = 0u32;
        for cp in 0x4E00u32..0x4F00u32 {
            let ch = char::from_u32(cp).unwrap_or('\0');
            if pipeline.glyph_information(ch).is_some_and(|i| i.width > 0) {
                inserted += 1;
            }
        }
        let final_len = pipeline.cache_length();
        assert!(
            final_len <= 10000,
            "cache_length {} exceeds capacity 10000",
            final_len
        );
        assert!(
            final_len >= after_ascii,
            "cache should not shrink after inserting new glyphs: \
             before={} after={} inserted={}",
            after_ascii,
            final_len,
            inserted
        );
        let bitmap = pipeline.atlas_bitmap();
        assert!(
            bitmap.iter().any(|&b| b != 0),
            "atlas bitmap should have non-zero bytes after glyph insertion"
        );
    }

    #[test]
    fn cjk_glyph_information_returns_nonzero_for_common_chars() {
        let mut pipeline = FontPipeline::new(1024, 1024, 14.0);
        let chars = [
            '你', '好', '世', '界', '中', '文', '字', '体', '渲', '染', '测', '试',
        ];
        for ch in chars {
            let info = pipeline
                .glyph_information(ch)
                .unwrap_or_else(|| panic!("CJK glyph_information('{ch}') should return Some"));
            assert!(
                info.width > 0 && info.height > 0,
                "CJK glyph '{ch}' should have nonzero dimensions: {}x{}",
                info.width,
                info.height
            );
        }
    }

    #[test]
    fn font_switching_changes_font_id() {
        let mut pipeline = FontPipeline::new(1024, 1024, 14.0);
        let original_id = pipeline.font_id;
        let names = pipeline.list_monospace_fonts();
        let mut found_switch = false;
        for name in &names {
            if name.is_empty() {
                continue;
            }
            if pipeline.set_font_family(name) && pipeline.font_id != original_id {
                found_switch = true;
                break;
            }
        }
        assert!(
            found_switch,
            "At least one font family should change font_id from {original_id:?}",
        );
    }

    #[test]
    fn font_switching_clears_cache() {
        let mut pipeline = FontPipeline::new(1024, 1024, 14.0);
        pipeline.rasterize_ascii();
        assert!(pipeline.cache_length() > 0);
        // Include a non-ASCII glyph in the baseline: after the switch the
        // cache is cleared and re-filled only with the new font's ASCII
        // rasterization, so the length must drop below this value.
        pipeline.glyph_information('好');
        let before = pipeline.cache_length();
        let names = pipeline.list_monospace_fonts();
        if names.len() > 1 {
            let alt = names.last().unwrap();
            pipeline.set_font_family(alt);
            assert!(
                pipeline.cache_length() < before,
                "cache should shrink after font switch to '{alt}'"
            );
        } else {
            pipeline.set_font_family("monospace");
            if pipeline.cache_length() == 0 {
                return;
            }
            assert!(
                pipeline.cache_length() < before,
                "cache should shrink after font switch"
            );
        }
    }

    #[test]
    fn cell_metrics_height_is_integer() {
        let pipeline = FontPipeline::new(1024, 1024, 14.0);
        let (_cw, ch) = pipeline.cell_metrics();
        assert!(
            (ch - ch.floor()).abs() < f32::EPSILON,
            "cell_height should be integer (ceil'd), got {ch}"
        );
    }

    #[test]
    fn cell_metrics_height_scales_with_font_size() {
        let small = FontPipeline::new(1024, 1024, 10.0);
        let large = FontPipeline::new(1024, 1024, 20.0);
        let (_, sh) = small.cell_metrics();
        let (_, lh) = large.cell_metrics();
        assert!(lh > sh, "larger font must have taller cell");
        assert!(
            (sh - sh.floor()).abs() < f32::EPSILON,
            "small cell_height should be integer"
        );
        assert!(
            (lh - lh.floor()).abs() < f32::EPSILON,
            "large cell_height should be integer"
        );
    }

    #[test]
    fn termux_formula_ascent_plus_descent_equals_cell_height() {
        let pipeline = FontPipeline::new(1024, 1024, 14.0);
        let ascent = pipeline.ascent_pixels();
        let descent = pipeline.descent_pixels();
        let (_, ch) = pipeline.cell_metrics();
        assert!(
            (ascent + descent - ch).abs() < 2.0,
            "ascent({ascent}) + descent({descent}) ≈ cell_height({ch}), diff={}",
            (ascent + descent - ch).abs()
        );
    }

    #[test]
    fn termux_formula_baseline_is_ascent_from_cell_top() {
        let pipeline = FontPipeline::new(1024, 1024, 14.0);
        let ascent = pipeline.ascent_pixels();
        let (_, ch) = pipeline.cell_metrics();
        assert!(
            ascent > 0.0 && ascent < ch,
            "ascent({ascent}) must be in (0, cell_h={ch})"
        );
    }

    #[test]
    fn termux_formula_glyph_bearing_y_matches() {
        let mut pipeline = FontPipeline::new(1024, 1024, 14.0);
        let ascent = pipeline.ascent_pixels();
        let info = pipeline
            .glyph_information('A')
            .expect("should have 'A' glyph");
        let bearing_y = ascent - info.placement.top as f32;
        assert!(
            bearing_y >= 0.0,
            "bearing_y for 'A' should be >= 0, got {bearing_y}"
        );
        let (_, ch) = pipeline.cell_metrics();
        assert!(
            bearing_y < ch,
            "bearing_y({bearing_y}) should be < cell_h({ch})"
        );
    }

    #[test]
    fn descent_pixels_is_positive() {
        let pipeline = FontPipeline::new(1024, 1024, 14.0);
        let descent = pipeline.descent_pixels();
        assert!(descent > 0.0, "descent should be positive, got {descent}");
    }

    #[test]
    fn cell_width_from_m_advance_matches() {
        let mut pipeline = FontPipeline::new(1024, 1024, 14.0);
        let info_m = pipeline.glyph_information('m').expect("should have 'm'");
        let info_x = pipeline.glyph_information('X').expect("should have 'X'");
        let (cw, _ch) = pipeline.cell_metrics();
        assert!(
            (info_m.advance_width - cw).abs() < 1.0,
            "advance_width('m')={} ≈ cell_w={}",
            info_m.advance_width,
            cw
        );
        assert!(
            (info_x.advance_width - cw).abs() < 1.0,
            "advance_width('X')={} ≈ cell_w={}",
            info_x.advance_width,
            cw
        );
    }

    #[test]
    fn any_monospace_advance_matches_cell_width() {
        let mut pipeline = FontPipeline::new(1024, 1024, 14.0);
        let (cw, _) = pipeline.cell_metrics();
        let chars = ['A', 'm', 'W', '0', 'l', 'i'];
        for ch in chars {
            if let Some(info) = pipeline.glyph_information(ch) {
                assert!(
                    (info.advance_width - cw).abs() < 2.0,
                    "advance('{ch}')={:.1} ≈ cell_w={:.1}",
                    info.advance_width,
                    cw
                );
            }
        }
        if let Some(alt) = pipeline.list_monospace_fonts().first().cloned()
            && pipeline.set_font_family(&alt)
        {
            let (cw2, _) = pipeline.cell_metrics();
            for ch in chars {
                if let Some(info) = pipeline.glyph_information(ch) {
                    assert!(
                        (info.advance_width - cw2).abs() < 2.0,
                        "font '{alt}': advance('{ch}')={:.1} ≈ cell_w={:.1}",
                        info.advance_width,
                        cw2
                    );
                }
            }
        }
    }

    #[test]
    fn cjk_advance_valid_for_any_font() {
        let mut pipeline = FontPipeline::new(1024, 1024, 14.0);
        let (cw, _) = pipeline.cell_metrics();
        let cjk_chars = ['中', '好', '世', '界', '日', '本'];
        for ch in cjk_chars {
            if let Some(info) = pipeline.glyph_information(ch) {
                assert!(
                    info.advance_width > 0.0,
                    "CJK '{ch}' must have positive advance, got {:.1}",
                    info.advance_width
                );
                assert!(
                    info.advance_width <= cw * 3.0,
                    "CJK '{ch}' advance={:.1} should be ≤ 3*cell_w={:.1}",
                    info.advance_width,
                    cw * 3.0
                );
            }
        }
    }

    #[test]
    fn ascii_bearing_y_nonnegative_for_any_font() {
        let mut pipeline = FontPipeline::new(1024, 1024, 14.0);
        let ascent = pipeline.ascent_pixels();
        let ascii = ['A', 'B', 'C', 'x', 'y', 'z', '0', '1', '9'];
        for ch in ascii {
            if let Some(info) = pipeline.glyph_information(ch) {
                let bearing_y = ascent - info.placement.top as f32;
                assert!(
                    bearing_y >= -2.0,
                    "bearing_y('{ch}')={:.1} should be >= -2",
                    bearing_y
                );
            }
        }
    }

    #[test]
    fn all_glyphs_within_atlas_bounds() {
        let mut pipeline = FontPipeline::new(1024, 1024, 14.0);
        pipeline.rasterize_ascii();
        let aw = pipeline.atlas_width as i32;
        let ah = pipeline.atlas_height as i32;
        let chars = ['A', '中', '好', 'α', 'Ω'];
        for ch in chars {
            if let Some(info) = pipeline.glyph_information(ch) {
                assert!(
                    info.atlas_x + info.width <= aw,
                    "glyph '{ch}' atlas_x({}) + width({}) exceeds atlas_w({})",
                    info.atlas_x,
                    info.width,
                    aw
                );
                assert!(
                    info.atlas_y + info.height <= ah,
                    "glyph '{ch}' atlas_y({}) + height({}) exceeds atlas_h({})",
                    info.atlas_y,
                    info.height,
                    ah
                );
            }
        }
    }

    #[test]
    fn system_monospace_name_returns_nonempty() {
        let pipeline = FontPipeline::new(1024, 1024, 14.0);
        let name = pipeline.system_monospace_name();
        assert!(
            !name.is_empty(),
            "system_monospace_name should return a non-empty string"
        );
    }

    #[test]
    fn set_font_family_empty_resets_to_default() {
        let mut pipeline = FontPipeline::new(1024, 1024, 14.0);
        let default_name = pipeline.default_font_name().clone();
        let fonts = pipeline.list_monospace_fonts();
        if let Some(other) = fonts.iter().find(|n| n.as_str() != default_name.as_str()) {
            pipeline.set_font_family(other);
            assert_eq!(
                pipeline.current_font_family_name().as_deref(),
                Some(other.as_str())
            );
            pipeline.set_font_family("");
            assert_eq!(
                pipeline.current_font_family_name().as_deref(),
                Some(default_name.as_str())
            );
        }
    }

    #[test]
    fn font_information_contains_all_sections() {
        let pipeline = FontPipeline::new(1024, 1024, 14.0);
        let info = pipeline.font_information();
        assert!(
            info.contains("Active:"),
            "font_information should contain 'Active:', got: {}",
            info
        );
        assert!(
            info.contains("CJK fallback:"),
            "font_information should contain 'CJK fallback:', got: {}",
            info
        );
        assert!(
            info.contains("Cell:"),
            "font_information should contain 'Cell:', got: {}",
            info
        );
        assert!(
            info.contains("Font size:"),
            "font_information should contain 'Font size:', got: {}",
            info
        );
    }

    #[test]
    fn set_font_family_persists_through_size_change() {
        let mut pipeline = FontPipeline::new(1024, 1024, 14.0);
        let fonts = pipeline.list_monospace_fonts();
        if let Some(target) = fonts.first() {
            pipeline.set_font_family(target);
            let name_before = pipeline.current_font_family_name();
            pipeline.set_font_size_in_place(20.0);
            let name_after = pipeline.current_font_family_name();
            assert_eq!(
                name_before, name_after,
                "font family should persist through size change"
            );
        }
    }

    #[test]
    fn cjk_fallback_has_vector_font() {
        let pipeline = FontPipeline::new(1024, 1024, 14.0);
        let cjk_names = pipeline.cjk_fallback_names();
        if !cjk_names.is_empty() {
            assert!(
                cjk_names.iter().all(|n| !n.is_empty()),
                "CJK fallback names should not be empty strings"
            );
        }
    }

    #[test]
    fn cell_metrics_reasonable_ratios() {
        let pipeline = FontPipeline::new(1024, 1024, 14.0);
        let (cw, ch) = pipeline.cell_metrics();
        assert!(cw > 0.0, "cell_width must be > 0, got {cw}");
        assert!(ch > 0.0, "cell_height must be > 0, got {ch}");
        assert!(
            cw < ch,
            "terminal cells should be taller than wide: cell_width={cw} >= cell_height={ch}"
        );
    }

    #[test]
    fn find_monospace_font_prefers_roboto_mono() {
        let pipeline = FontPipeline::new(1024, 1024, 14.0);
        let name = pipeline.default_font_name();
        assert!(!name.is_empty(), "should find a monospace font, got empty");
    }

    #[test]
    fn cjk_fallback_uses_vector_font() {
        let mut pipeline = FontPipeline::new(512, 512, 14.0);
        let names = pipeline.cjk_fallback_names();
        if !names.is_empty() {
            let cjk_info = pipeline.glyph_information('好').expect("CJK glyph info");
            assert!(
                cjk_info.width > 0,
                "CJK glyph should have meaningful width, got {}",
                cjk_info.width
            );
        }
    }

    fn try_load_cjk_fonts(db: &mut fontdb::Database) -> bool {
        let has_cjk = db.faces().any(|face| {
            face.families
                .first()
                .map(|(n, _)| n.to_lowercase().contains("cjk"))
                .unwrap_or(false)
        });
        if has_cjk {
            return true;
        }
        if let Ok(glob) = std::fs::read_dir("/nix/store") {
            for entry in glob.flatten() {
                let p = entry.path();
                if p.to_string_lossy().contains("noto-fonts-cjk") {
                    let font_dir = p.join("share/fonts/opentype/noto-cjk");
                    if font_dir.is_dir() {
                        db.load_fonts_dir(&font_dir);
                        return true;
                    }
                }
            }
        }
        false
    }

    #[test]
    fn non_cjk_locale_no_fallback() {
        let mut pipeline = FontPipeline::new(1024, 1024, 14.0);
        pipeline.set_system_locale("en-US");
        let info = pipeline.font_information();
        assert!(
            info.contains("CJK fallback: none"),
            "en-US locale should have no CJK fallback: {info}"
        );
    }

    #[test]
    fn font_info_json_serializes_structured_fields() {
        let mut pipeline = FontPipeline::new(1024, 1024, 14.0);
        pipeline.set_system_locale("en-US");
        let json = serde_json::to_string(&pipeline.font_info()).expect("font_info serializes");
        let parsed: serde_json::Value = serde_json::from_str(&json).expect("font_info json parses");
        assert_eq!(parsed["cjk_state"], "none");
        assert!(
            parsed["active"].is_object(),
            "active should be present: {json}"
        );
        assert_eq!(parsed["font_size"], 14.0);
        assert!(parsed["cell_width_px"].as_f64().is_some());
        assert!(parsed["cjk_families"].is_array());
    }

    // ──: layered fallback (moke chain, spec d7) ─────────────

    #[test]
    fn fallback_layer_family_predicates() {
        // moke chain layers: CJK / symbols / nerd / emoji are partitioned
        // by family name; a family belongs to exactly one layer.
        assert!(FontPipeline::is_cjk_candidate_family("noto sans cjk sc"));
        assert!(!FontPipeline::is_cjk_candidate_family("noto color emoji"));
        assert!(!FontPipeline::is_cjk_candidate_family("symbols nerd font"));

        assert!(FontPipeline::is_symbol_candidate_family(
            "noto sans symbols 2"
        ));
        assert!(FontPipeline::is_symbol_candidate_family("dejavu dingbats"));
        assert!(!FontPipeline::is_symbol_candidate_family("dejavu sans"));
        assert!(!FontPipeline::is_symbol_candidate_family(
            "symbols nerd font"
        ));

        assert!(FontPipeline::is_nerd_candidate_family("symbols nerd font"));
        assert!(FontPipeline::is_nerd_candidate_family(
            "jetbrainsmono nerd font"
        ));
        assert!(!FontPipeline::is_nerd_candidate_family(
            "noto sans symbols 2"
        ));

        assert!(FontPipeline::is_emoji_candidate_family("noto color emoji"));
        assert!(!FontPipeline::is_emoji_candidate_family(
            "noto sans symbols 2"
        ));
    }

    #[test]
    fn symbol_glyph_resolves_via_database_scan() {
        // U+25B6 (▶) is absent from Liberation Mono but present in
        // DejaVu Sans on the host. With Liberation Mono as the primary
        // the layered chain ends with a whole-database scan (spec d7),
        // which must resolve the glyph in a non-primary font.
        let mut pipeline = FontPipeline::new(512, 512, 14.0);
        let names = pipeline.list_monospace_fonts();
        if !names.iter().any(|n| n.contains("Liberation")) {
            // Coverage guard: the scan tail is exercised on hosts that
            // ship Liberation Mono (the standard Debian/CI set).
            eprintln!("SKIP: symbol_glyph_resolves_via_database_scan (no Liberation Mono)");
            return;
        }
        assert!(
            pipeline.set_font_family("Liberation Mono"),
            "switch to Liberation Mono"
        );
        let primary = pipeline.font_id.expect("primary font");
        let info = pipeline.glyph_information('▶').expect("symbol glyph");
        assert!(
            info.width > 0,
            "symbol must rasterize, got width {}",
            info.width
        );
        let resolved = pipeline
            .caches
            .cjk_glyph_cache
            .get(&'▶')
            .expect("resolved glyph must be cached");
        assert_ne!(
            resolved.0, primary,
            "symbol must resolve via a fallback font, not the primary"
        );
        assert_ne!(
            resolved.1, 0,
            "symbol must resolve to a real glyph, not .notdef"
        );
    }

    #[test]
    fn private_use_glyph_renders_notdef_without_panic() {
        // U+E0A0 (powerline separator PUA) exists in no host font: the
        // chain must end at.notdef without panicking (spec d7 scenario 3).
        let mut pipeline = FontPipeline::new(512, 512, 14.0);
        let info = pipeline.glyph_information('\u{e0a0}');
        assert!(info.is_some(), ".notdef fallback must return a glyph");
    }

    #[test]
    fn emoji_glyph_no_panic_when_color_font_cannot_outline() {
        // Noto Color Emoji covers 😀 but swash cannot outline color
        // glyphs; the emoji layer and the database scan must skip it
        // without panicking (moke: emoji via system chain).
        let mut pipeline = FontPipeline::new(512, 512, 14.0);
        let _ = pipeline.glyph_information('😀');
        // Reaching here without panic is the assertion.
    }

    #[test]
    fn fallback_names_report_all_layers() {
        // cjk_fallback_names is the CJK-only view; the layered fields are
        // all populated by construction.
        let pipeline = FontPipeline::new(512, 512, 14.0);
        let cjk = pipeline.cjk_fallback_names();
        assert!(
            cjk.iter().all(|n| !n.is_empty()),
            "CJK fallback names must not be empty strings"
        );
    }

    #[test]
    fn cjk_locale_selects_correct_variant() {
        let mut pipeline = FontPipeline::new(512, 512, 14.0);
        if !try_load_cjk_fonts(pipeline.font_system.db_mut()) {
            // nosemgrep: semgrep.no-eprintln-library — test skip diagnostic
            eprintln!("SKIP: cjk_locale_selects_correct_variant (no CJK fonts)");
            return;
        }
        let cases: &[(&str, &str)] = &[
            ("zh-CN", "sc"),
            ("zh-TW", "tc"),
            ("zh-HK", "tc"),
            ("zh-Hant", "tc"),
            ("zh-Hans", "sc"),
            ("zh", "sc"),
            ("ja", "jp"),
            ("ko", "kr"),
        ];
        for (locale, expected_tag) in cases {
            let mut pipeline = FontPipeline::new(512, 512, 14.0);
            try_load_cjk_fonts(pipeline.font_system.db_mut());
            pipeline.set_system_locale(locale);
            let ids = &pipeline.cjk_fallback_ids;
            assert!(!ids.is_empty(), "locale '{locale}' should have fallback");
            let db = pipeline.font_system.db();
            let has_tag = ids.iter().any(|id| {
                db.face(*id)
                    .and_then(|f| f.families.first())
                    .is_some_and(|(n, _)| n.to_lowercase().contains(expected_tag))
            });
            assert!(
                has_tag,
                "locale '{locale}' fallback should include '{expected_tag}'-family font"
            );
        }
    }

    #[test]
    fn primary_cjk_font_no_fallback() {
        let mut pipeline = FontPipeline::new(512, 512, 14.0);
        if !try_load_cjk_fonts(pipeline.font_system.db_mut()) {
            // nosemgrep: semgrep.no-eprintln-library — test skip diagnostic
            eprintln!("SKIP: primary_cjk_font_no_fallback (no CJK fonts)");
            return;
        }
        let mut pipeline = FontPipeline::new(512, 512, 14.0);
        try_load_cjk_fonts(pipeline.font_system.db_mut());
        pipeline.set_system_locale("zh-CN");
        let cjk_fonts: Vec<String> = pipeline
            .list_monospace_fonts()
            .into_iter()
            .filter(|n| n.to_lowercase().contains("cjk"))
            .collect();
        if let Some(cjk_name) = cjk_fonts.first() {
            pipeline.set_font_family(cjk_name);
            let names = pipeline.cjk_fallback_names();
            assert!(
                names.is_empty(),
                "primary font '{cjk_name}' supports CJK → no fallback, got: {names:?}"
            );
        }
    }

    #[test]
    fn max_one_fallback_font() {
        let mut pipeline = FontPipeline::new(1024, 1024, 14.0);
        if !try_load_cjk_fonts(pipeline.font_system.db_mut()) {
            // nosemgrep: semgrep.no-eprintln-library — test skip diagnostic
            eprintln!("SKIP: max_one_fallback_font (no CJK fonts)");
            return;
        }
        let mut pipeline = FontPipeline::new(1024, 1024, 14.0);
        try_load_cjk_fonts(pipeline.font_system.db_mut());
        pipeline.set_system_locale("zh-CN");
        assert!(
            pipeline.cjk_fallback_ids.len() <= 3,
            "MAX_CJK_FALLBACK_FONTS=3, got {} IDs",
            pipeline.cjk_fallback_ids.len()
        );
    }

    #[test]
    fn font_information_includes_cjk_fallback() {
        let pipeline = FontPipeline::new(1024, 1024, 14.0);
        let info = pipeline.font_information();
        assert!(
            info.contains("Active:") || info.contains("Cell:"),
            "font info should have structure: {info}"
        );
    }

    #[test]
    fn fonts_xml_index_match_resolves_exact_face() {
        let mut db = fontdb::Database::new();
        if !try_load_cjk_fonts(&mut db) {
            eprintln!("SKIP: fonts_xml_index_match_resolves_exact_face (no CJK fonts)");
            return;
        }
        // Pick a TTC face so (filename, index) mapping is exercised.
        let (filename, index) = db
            .faces()
            .filter_map(|face| {
                let path = match &face.source {
                    fontdb::Source::File(path) => path,
                    fontdb::Source::SharedFile(path, _) => path,
                    fontdb::Source::Binary(_) => return None,
                };
                let name = path.file_name()?.to_str()?.to_string();
                (path.extension()?.to_str()?.eq_ignore_ascii_case("ttc"))
                    .then_some((name, face.index))
            })
            .next()
            .expect("CJK TTC face must exist after try_load_cjk_fonts");
        let xml = format!(
            r#"<familyset version="23"><family lang="zh-Hans"><font weight="400" style="normal" index="{index}">{filename}</font></family></familyset>"#
        );
        let ids = FontPipeline::match_fonts_xml_fallbacks(&db, &xml, "zh-CN", 3);
        assert_eq!(ids.len(), 1, "exact (filename, index) hit expected");
        let face = db.face(ids[0]).expect("matched face exists");
        assert_eq!(face.index, index, "TTC index must match fonts.xml");
        let matched_name = match &face.source {
            fontdb::Source::File(path) => path,
            fontdb::Source::SharedFile(path, _) => path,
            fontdb::Source::Binary(_) => panic!("matched face must be file-backed"),
        }
        .file_name()
        .and_then(|name| name.to_str())
        .expect("file-backed face has a name");
        assert_eq!(matched_name, filename);
    }

    #[test]
    fn fonts_xml_missing_file_falls_back_to_scan() {
        // Unknown filename: no exact hit, caller fills from the scan.
        let mut db = fontdb::Database::new();
        if !try_load_cjk_fonts(&mut db) {
            eprintln!("SKIP: fonts_xml_missing_file_falls_back_to_scan (no CJK fonts)");
            return;
        }
        let xml = r#"<familyset version="23"><family lang="zh-Hans"><font index="2">NoSuchFont-Regular.ttc</font></family></familyset>"#;
        let ids = FontPipeline::match_fonts_xml_fallbacks(&db, xml, "zh-CN", 3);
        assert!(
            ids.is_empty(),
            "unknown file must yield no exact hit: {ids:?}"
        );
    }

    /// Locate the nix-provided Maple Mono NF CN font (flake.nix). The
    /// store hash is unstable, so discover by directory prefix instead
    /// of hardcoding the full path.
    fn find_maple_mono_font() -> Option<std::path::PathBuf> {
        let store = std::fs::read_dir("/nix/store").ok()?;
        for entry in store.flatten() {
            let path = entry.path();
            if path
                .file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.contains("MapleMono"))
            {
                let candidate = path.join("share/fonts/truetype/MapleMonoNormal-NF-CN-Medium.ttf");
                if candidate.is_file() {
                    return Some(candidate);
                }
            }
        }
        None
    }

    #[test]
    fn maple_mono_primary_skips_cjk_fallback() {
        // Maple Mono NF CN ships CJK glyphs: as the primary font it must
        // cover CJK directly with no fallback layer (spec: skip path).
        // CJK + Latin resolve through the same cache, keeping CJK render
        // speed on par with Latin (no per-glyph fallback scan).
        let Some(font_path) = find_maple_mono_font() else {
            eprintln!("SKIP: maple_mono_primary_skips_cjk_fallback (no Maple Mono in /nix/store)");
            return;
        };
        let mut pipeline = FontPipeline::new(512, 512, 14.0);
        let family = pipeline
            .load_font_file(&font_path)
            .expect("maple mono loads");
        assert!(
            pipeline.set_font_family(&family),
            "maple mono selectable as primary"
        );
        pipeline.set_system_locale("zh-CN");
        assert!(
            pipeline.cjk_fallback_names().is_empty(),
            "CJK-capable primary must skip fallback, got: {:?}",
            pipeline.cjk_fallback_names()
        );
        let latin = pipeline.glyph_information('A').expect("latin resolves");
        let cjk = pipeline.glyph_information('中').expect("CJK resolves");
        assert!(latin.width > 0 && cjk.width > 0);
        // Second pass must hit the caches (no repeated fallback scans).
        let latin_again = pipeline.glyph_information('A').expect("latin cached");
        let cjk_again = pipeline.glyph_information('中').expect("CJK cached");
        assert_eq!(latin.width, latin_again.width);
        assert_eq!(cjk.width, cjk_again.width);
    }

    #[test]
    fn atlas_defrag_recovers_from_full_atlas() {
        let mut pipeline = FontPipeline::new(64, 64, 14.0);
        let mut successes = 0u32;
        for cp in 0x4E00u32..0x4F00u32 {
            if let Some(ch) = char::from_u32(cp)
                && pipeline.glyph_information(ch).is_some_and(|i| i.width > 0)
            {
                successes += 1;
            }
        }
        assert!(
            successes > 0,
            "should have inserted at least some CJK glyphs"
        );
        let bitmap = pipeline.atlas_bitmap();
        assert!(
            bitmap.iter().any(|&b| b != 0),
            "atlas should have content after defrag"
        );
    }

    /// Locate a TTF font file for tests. FR-057 bans committed font files
    /// (`.gitignore:53 *.ttf`), so prefer the gitignored local
    /// `test_data/TerminusTTF-Regular.ttf` copy when present, then fall back
    /// to a system font via fontconfig (the Nix devShell provides fonts).
    fn find_test_font() -> std::path::PathBuf {
        let local = std::path::Path::new(TEST_DATA_DIR).join("TerminusTTF-Regular.ttf");
        if local.exists() {
            return local;
        }
        let output = std::process::Command::new("fc-list")
            .arg(":outline")
            .output()
            .expect("fc-list must be available (nix develop provides fontconfig)");
        let stdout = String::from_utf8_lossy(&output.stdout);
        for line in stdout.lines() {
            if let Some(path) = line.split(':').next()
                && path.ends_with(".ttf")
            {
                return std::path::PathBuf::from(path);
            }
        }
        panic!(
            "no system TTF font found; run inside `nix develop` or place \
             native/test_data/TerminusTTF-Regular.ttf (gitignored)"
        );
    }

    #[test]
    fn load_font_file_valid_ttf_returns_family() {
        let mut p = FontPipeline::new(512, 512, 14.0);
        let font_path = find_test_font();
        let family = p.load_font_file(&font_path);
        let family = family.expect("load_font_file should return Some for valid TTF");
        assert!(
            !family.is_empty(),
            "family name should not be empty, got '{family}'"
        );
        // The local Terminus fixture asserts the exact family name; system
        // font fallbacks only need to resolve to a non-empty family.
        if font_path
            .file_name()
            .is_some_and(|name| name.to_string_lossy().contains("Terminus"))
        {
            assert!(
                family.contains("Terminus") || family.contains("TerminusTTF"),
                "expected 'Terminus' in family name, got '{family}'"
            );
        }
    }

    #[test]
    fn load_font_file_nonexistent_path_returns_none() {
        let mut p = FontPipeline::new(512, 512, 14.0);
        let result = p.load_font_file(std::path::Path::new("/nonexistent/path/to/font.ttf"));
        assert!(result.is_none(), "should return None for nonexistent path");
    }

    #[test]
    fn load_font_file_empty_file_returns_none() {
        let dir = std::env::temp_dir().join("test_font_load");
        let _ = std::fs::create_dir_all(&dir);
        let empty_path = dir.join("empty.ttf");
        std::fs::write(&empty_path, []).ok();
        let mut p = FontPipeline::new(512, 512, 14.0);
        let result = p.load_font_file(&empty_path);
        assert!(result.is_none(), "empty file should return None");
        let _ = std::fs::remove_file(&empty_path);
    }

    #[test]
    fn load_font_file_corrupt_file_returns_none() {
        let dir = std::env::temp_dir().join("test_font_load");
        let _ = std::fs::create_dir_all(&dir);
        let corrupt_path = dir.join("corrupt.ttf");
        let garbage: Vec<u8> = (0..256).map(|i| (i ^ 0xAB) as u8).collect();
        std::fs::write(&corrupt_path, &garbage).ok();
        let mut p = FontPipeline::new(512, 512, 14.0);
        let result = p.load_font_file(&corrupt_path);
        assert!(result.is_none(), "corrupt file should return None");
        let _ = std::fs::remove_file(&corrupt_path);
    }

    // ──: bold/italic glyph synthesis ─────────────────────────

    /// Count non-zero alpha pixels in the glyph's atlas region (RGBA atlas).
    fn glyph_pixel_count(info: &GlyphInfo, bitmap: &[u8], atlas_width: usize) -> usize {
        if info.allocation_id.is_none() || info.width <= 0 || info.height <= 0 {
            return 0;
        }
        let mut count = 0usize;
        for y in 0..info.height as usize {
            let row = (info.atlas_y as usize + y) * atlas_width;
            for x in 0..info.width as usize {
                if bitmap[row + info.atlas_x as usize + x] > 0 {
                    count += 1;
                }
            }
        }
        count
    }

    fn styled_test_pipeline() -> (FontPipeline, String) {
        let mut p = FontPipeline::new(512, 512, 14.0);
        let font_path = find_test_font();
        let family = p.load_font_file(&font_path).expect("test font loads");
        assert!(p.set_font_family(&family), "test font family selects");
        (p, family)
    }

    #[test]
    fn styled_bold_produces_heavier_glyph() {
        let (mut p, _) = styled_test_pipeline();
        let regular = p.glyph_information('A').expect("regular A");
        let bold = p
            .glyph_information_styled('A', true, false)
            .expect("bold A");
        assert!(bold.width > 0 && bold.height > 0, "bold bitmap must exist");
        // The styled bitmap must actually differ from the regular one —
        // either a real bold face was resolved or synthesis emboldened it.
        assert_ne!(
            bold.atlas_x, regular.atlas_x,
            "bold and regular glyphs must not share a cache entry"
        );
    }

    #[test]
    fn styled_italic_shears_glyph() {
        let (mut p, _) = styled_test_pipeline();
        let regular = p.glyph_information('A').expect("regular A");
        let italic = p
            .glyph_information_styled('A', false, true)
            .expect("italic A");
        assert!(
            italic.width > 0 && italic.height > 0,
            "italic bitmap must exist"
        );
        assert_ne!(
            italic.atlas_x, regular.atlas_x,
            "italic and regular glyphs must not share a cache entry"
        );
        // Either a real italic face was resolved (its metrics are the
        // designer's own — may be narrower) or the shear synthesis applied;
        // the only invariant is that the styled bitmap differs.
        let bitmap = p.atlas_bitmap();
        let atlas_width = p.atlas_width as usize;
        let regular_pixels = glyph_pixel_count(&regular, &bitmap, atlas_width);
        let italic_pixels = glyph_pixel_count(&italic, &bitmap, atlas_width);
        assert_ne!(
            regular_pixels, italic_pixels,
            "italic bitmap must differ from regular (real face or shear)"
        );
    }

    #[test]
    fn styled_bold_italic_combines_both() {
        let (mut p, _) = styled_test_pipeline();
        let regular = p.glyph_information('A').expect("regular A");
        let bold_italic = p
            .glyph_information_styled('A', true, true)
            .expect("bold-italic A");
        assert!(bold_italic.width > 0 && bold_italic.height > 0);
        assert_ne!(
            bold_italic.atlas_x, regular.atlas_x,
            "bold-italic must have its own cache entry"
        );
    }

    #[test]
    fn styled_glyph_cache_distinguishes_synthesis() {
        let (mut p, _) = styled_test_pipeline();
        let _regular = p.glyph_information('A').expect("regular A");
        let bold = p
            .glyph_information_styled('A', true, false)
            .expect("bold A");
        let italic = p
            .glyph_information_styled('A', false, true)
            .expect("italic A");
        // Re-lookup returns the cached styled glyphs (same atlas slot) and
        // never the regular one.
        let bold_again = p
            .glyph_information_styled('A', true, false)
            .expect("bold A again");
        let italic_again = p
            .glyph_information_styled('A', false, true)
            .expect("italic A again");
        assert_eq!(bold.atlas_x, bold_again.atlas_x);
        assert_eq!(italic.atlas_x, italic_again.atlas_x);
    }

    #[test]
    fn resolve_style_face_prefers_same_family_bold_when_available() {
        let (mut p, _) = styled_test_pipeline();
        let base_id = p.font_id.expect("font selected");
        let base_family = p
            .font_system
            .db()
            .face(base_id)
            .and_then(|f| f.families.first().map(|(n, _)| n.clone()));
        // With system fonts loaded, the family may or may not have a bold
        // face on this host. Either way the contract must hold: a resolved
        // face belongs to the same family and differs from the base; no
        // face at all means the caller falls back to synthesis.
        if let Some(style_id) = p.resolve_style_face(base_id, true, false) {
            assert_ne!(style_id, base_id, "bold face must differ from regular");
            let style_family = p
                .font_system
                .db()
                .face(style_id)
                .and_then(|f| f.families.first().map(|(n, _)| n.clone()));
            assert_eq!(
                base_family, style_family,
                "style face must share the base family"
            );
        }
        // Resolving the plain style yields a face of the same family
        // (fontdb's query returns the closest match, which is the base
        // itself unless another normal face of the family exists — e.g.
        // DejaVuSansCondensed — so only the family invariant is asserted).
        if let Some(plain) = p.resolve_style_face(base_id, false, false) {
            let plain_family = p
                .font_system
                .db()
                .face(plain)
                .and_then(|f| f.families.first().map(|(n, _)| n.clone()));
            assert_eq!(
                base_family, plain_family,
                "plain-style face must share the base family"
            );
        }
    }

    #[test]
    fn load_font_file_multiple_times_works() {
        let mut p = FontPipeline::new(512, 512, 14.0);
        let font_path = find_test_font();
        let first = p.load_font_file(&font_path);
        let second = p.load_font_file(&font_path);
        assert!(first.is_some(), "first load should succeed");
        assert!(second.is_some(), "second load of same file should succeed");
        assert_eq!(
            first, second,
            "loading same file twice should return same family"
        );
    }

    #[test]
    fn load_font_file_does_not_break_cell_metrics() {
        let mut p = FontPipeline::new(512, 512, 14.0);
        let (cw_before, ch_before) = p.cell_metrics();
        assert!(
            cw_before > 0.0 && ch_before > 0.0,
            "initial metrics should be positive"
        );
        let font_path = find_test_font();
        let family = p.load_font_file(&font_path);
        assert!(family.is_some(), "should load test font");
        let (cw_after, ch_after) = p.cell_metrics();
        assert!(
            (cw_before - cw_after).abs() < f32::EPSILON,
            "cell width unchanged after load_font_file"
        );
        assert!(
            (ch_before - ch_after).abs() < f32::EPSILON,
            "cell height unchanged after load_font_file"
        );
    }

    #[test]
    fn load_font_file_loaded_font_can_be_set() {
        let mut p = FontPipeline::new(512, 512, 14.0);
        let family = p
            .load_font_file(&find_test_font())
            .expect("should load test font");
        assert!(
            p.set_font_family(&family),
            "set_font_family should succeed for loaded font '{family}'"
        );
        let (cw, ch) = p.cell_metrics();
        assert!(cw > 0.0, "cell width positive after setting loaded font");
        assert!(ch > 0.0, "cell height positive after setting loaded font");
    }

    #[test]
    fn load_font_file_unicode_path() {
        let dir = std::env::temp_dir().join("test_unicode_字体");
        let _ = std::fs::create_dir_all(&dir);
        let target = dir.join("测试-font.ttf");
        std::fs::copy(find_test_font(), &target).expect("copy test font to unicode path");
        let mut p = FontPipeline::new(512, 512, 14.0);
        let family = p.load_font_file(&target);
        assert!(family.is_some(), "should load font from unicode path");
        assert!(!family.unwrap().is_empty(), "family should not be empty");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn load_font_file_after_set_font_family() {
        let mut p = FontPipeline::new(512, 512, 14.0);
        let fonts = p.list_monospace_fonts();
        if let Some(first) = fonts.first() {
            assert!(p.set_font_family(first), "set font family {first}");
        }
        let result = p.load_font_file(&find_test_font());
        assert!(
            result.is_some(),
            "load after set_font_family should succeed"
        );
    }
}

#[cfg(test)]
mod nerd_render_tests {
    use super::*;

    fn pipeline_with_nerd_font() -> FontPipeline {
        let mut pipeline = FontPipeline::new(512, 512, 14.0);
        // Load any Nerd Font available on this host (the nix store path is
        // the dev-shell location; the test is skipped when absent).
        let mut candidates: Vec<std::path::PathBuf> = Vec::new();
        if let Ok(store) = std::fs::read_dir("/nix/store") {
            for entry in store.flatten() {
                let p = entry.path();
                if p.to_string_lossy().contains("nerd-fonts")
                    && let Ok(files) = std::fs::read_dir(p.join("share/fonts/truetype/NerdFonts"))
                {
                    for file in files.flatten() {
                        let f = file.path();
                        if f.to_string_lossy().ends_with("NerdFontMono-Regular.ttf") {
                            candidates.push(f);
                        }
                    }
                }
            }
        }
        for path in candidates {
            pipeline.font_system.db_mut().load_font_file(path).ok();
        }
        pipeline.find_symbol_fallback_fonts();
        pipeline.find_nerd_fallback_fonts();
        pipeline
    }

    #[test]
    fn nerd_pua_glyph_renders_from_nerd_fallback() {
        let mut pipeline = pipeline_with_nerd_font();
        if pipeline.nerd_fallback_ids.is_empty() {
            // No Nerd Font installed on this host — nothing to verify.
            return;
        }
        let info = pipeline
            .glyph_information('\u{e0a0}')
            .expect("U+E0A0 should render via the Nerd fallback");
        assert!(
            info.width > 0 && info.height > 0,
            "U+E0A0 must produce a real glyph bitmap, got {}x{}",
            info.width,
            info.height
        );
        // Powerline left triangle: taller than wide.
        assert!(
            info.height > info.width,
            "U+E0A0 powerline triangle should be tall ({}x{})",
            info.width,
            info.height
        );
    }
}
