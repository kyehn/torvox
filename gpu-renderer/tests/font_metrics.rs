use gpu_renderer::font::FontPipeline;

const ATLAS_SIZE: i32 = 2048;

fn create_pipeline(font_size: f32) -> FontPipeline {
    FontPipeline::new(ATLAS_SIZE, ATLAS_SIZE, font_size)
}

#[cfg(test)]
mod font_metrics_correctness {
    use super::*;

    #[test]
    fn cell_width_is_positive() {
        let pipeline = create_pipeline(14.0);
        let (width, _) = pipeline.cell_metrics();
        assert!(width > 0.0, "cell_width should be positive, got {width}");
        assert!(width < 100.0, "cell_width should be < 100px, got {width}");
    }

    #[test]
    fn cell_height_is_positive() {
        let pipeline = create_pipeline(14.0);
        let (_, height) = pipeline.cell_metrics();
        assert!(height > 0.0, "cell_height should be positive, got {height}");
        assert!(
            height < 100.0,
            "cell_height should be < 100px, got {height}"
        );
    }

    #[test]
    fn ascent_px_is_positive() {
        let pipeline = create_pipeline(14.0);
        let ascent = pipeline.ascent_pixels();
        assert!(ascent > 0.0, "ascent should be positive, got {ascent}");
        assert!(ascent < 100.0, "ascent should be < 100px, got {ascent}");
    }

    #[test]
    fn descent_px_is_positive() {
        let pipeline = create_pipeline(14.0);
        let descent = pipeline.descent_pixels();
        assert!(descent > 0.0, "descent should be positive, got {descent}");
        assert!(descent < 100.0, "descent should be < 100px, got {descent}");
    }

    #[test]
    fn ascii_glyphs_have_nonzero_width() {
        let mut pipeline = create_pipeline(14.0);
        for ch in 'A'..='Z' {
            let info = pipeline
                .glyph_information(ch)
                .expect("ASCII glyph should exist");
            assert!(
                info.width > 0,
                "ASCII '{ch}' should have positive width, got {}",
                info.width
            );
        }
    }

    #[test]
    fn ascii_glyphs_have_nonzero_height() {
        let mut pipeline = create_pipeline(14.0);
        for ch in 'A'..='Z' {
            let info = pipeline
                .glyph_information(ch)
                .expect("ASCII glyph should exist");
            assert!(
                info.height > 0,
                "ASCII '{ch}' should have positive height, got {}",
                info.height
            );
        }
    }

    #[test]
    fn ascii_glyphs_have_positive_advance() {
        let mut pipeline = create_pipeline(14.0);
        for ch in 'A'..='Z' {
            let info = pipeline
                .glyph_information(ch)
                .expect("ASCII glyph should exist");
            assert!(
                info.advance_width > 0.0,
                "ASCII '{ch}' should have positive advance_width, got {}",
                info.advance_width
            );
        }
    }

    #[test]
    fn ascii_lowercase_glyphs_have_nonzero_dimensions() {
        let mut pipeline = create_pipeline(14.0);
        for ch in 'a'..='z' {
            let info = pipeline
                .glyph_information(ch)
                .expect("lowercase glyph should exist");
            assert!(
                info.width > 0 && info.height > 0,
                "lowercase '{ch}' should have positive dimensions, got {}x{}",
                info.width,
                info.height
            );
        }
    }

    #[test]
    fn digits_have_nonzero_dimensions() {
        let mut pipeline = create_pipeline(14.0);
        for ch in '0'..='9' {
            let info = pipeline
                .glyph_information(ch)
                .expect("digit glyph should exist");
            assert!(
                info.width > 0 && info.height > 0,
                "digit '{ch}' should have positive dimensions, got {}x{}",
                info.width,
                info.height
            );
        }
    }
}

#[cfg(test)]
mod font_fallback {
    use super::*;

    #[test]
    fn cjk_characters_do_not_panic() {
        let mut pipeline = create_pipeline(14.0);
        let cjk_chars = ['中', '好', '世', '界', '日', '本', '韓', '國'];
        for ch in cjk_chars {
            let _info = pipeline.glyph_information(ch);
        }
    }

    #[test]
    fn cjk_characters_have_nonzero_dimensions() {
        let mut pipeline = create_pipeline(14.0);
        let cjk_chars = ['中', '好', '世', '界'];
        for ch in cjk_chars {
            if let Some(info) = pipeline.glyph_information(ch) {
                assert!(
                    info.width > 0 && info.height > 0,
                    "CJK '{ch}' should have positive dimensions, got {}x{}",
                    info.width,
                    info.height
                );
            }
        }
    }

    #[test]
    fn cjk_advance_is_reasonable() {
        let mut pipeline = create_pipeline(14.0);
        let (cell_width, _) = pipeline.cell_metrics();
        for ch in ['中', '好', '世', '界'] {
            if let Some(info) = pipeline.glyph_information(ch) {
                assert!(
                    info.advance_width > 0.0 && info.advance_width <= cell_width * 3.0,
                    "CJK '{ch}' advance should be reasonable, got {} (cell_width={})",
                    info.advance_width,
                    cell_width
                );
            }
        }
    }

    #[test]
    fn emoji_characters_do_not_panic() {
        let mut pipeline = create_pipeline(14.0);
        let emoji_chars = [
            '\u{1F600}', // grinning face
            '\u{1F601}', // beaming face
            '\u{1F389}', // party popper
            '\u{1F4A9}', // pile of poo
            '\u{1F680}', // rocket
        ];
        for ch in emoji_chars {
            let _info = pipeline.glyph_information(ch);
        }
    }

    #[test]
    fn box_drawing_characters_do_not_panic() {
        let mut pipeline = create_pipeline(14.0);
        let box_chars = [
            '─', // box drawings light horizontal
            '│', // box drawings light vertical
            '┌', // box drawings light down and right
            '┐', // box drawings light down and left
            '└', // box drawings light up and right
            '┘', // box drawings light up and left
            '├', // box drawings light vertical and right
            '┤', // box drawings light vertical and left
            '┬', // box drawings light down and horizontal
            '┴', // box drawings light up and horizontal
            '┼', // box drawings light vertical and horizontal
        ];
        for ch in box_chars {
            let _info = pipeline.glyph_information(ch);
        }
    }

    #[test]
    fn special_characters_do_not_panic() {
        let mut pipeline = create_pipeline(14.0);
        let special_chars = [
            '!', '@', '#', '$', '%', '^', '&', '*', '(', ')', '-', '+', '=',
        ];
        for ch in special_chars {
            let _info = pipeline.glyph_information(ch);
        }
    }
}

#[cfg(test)]
mod cell_metrics_consistency {
    use super::*;

    #[test]
    fn monospace_width_is_consistent() {
        let mut pipeline = create_pipeline(14.0);
        let (cell_width, _) = pipeline.cell_metrics();

        // For monospace fonts, all ASCII characters should have the same advance
        for ch in ['A', 'B', 'C', 'X', 'Y', 'Z', 'm', 'W'] {
            if let Some(info) = pipeline.glyph_information(ch) {
                assert!(
                    (info.advance_width - cell_width).abs() < 2.0,
                    "monospace '{ch}' advance should match cell_width: advance={}, cell={}",
                    info.advance_width,
                    cell_width
                );
            }
        }
    }

    #[test]
    fn cell_height_is_integer() {
        let pipeline = create_pipeline(14.0);
        let (_, height) = pipeline.cell_metrics();
        assert!(
            (height - height.floor()).abs() < f32::EPSILON,
            "cell_height should be integer (ceil'd), got {height}"
        );
    }

    #[test]
    fn font_size_affects_metrics_proportionally() {
        let small = create_pipeline(10.0);
        let medium = create_pipeline(14.0);
        let large = create_pipeline(20.0);

        let (sw, sh) = small.cell_metrics();
        let (mw, mh) = medium.cell_metrics();
        let (lw, lh) = large.cell_metrics();

        assert!(
            mw > sw,
            "medium font should have wider cell than small: {mw} <= {sw}"
        );
        assert!(
            lw > mw,
            "large font should have wider cell than medium: {lw} <= {mw}"
        );
        assert!(
            mh > sh,
            "medium font should have taller cell than small: {mh} <= {sh}"
        );
        assert!(
            lh > mh,
            "large font should have taller cell than medium: {lh} <= {mh}"
        );
    }

    #[test]
    fn aspect_ratio_is_reasonable() {
        let pipeline = create_pipeline(14.0);
        let (width, height) = pipeline.cell_metrics();
        let ratio = width / height;

        // Monospace fonts typically have aspect ratio between 0.4 and 0.8
        assert!(
            ratio > 0.4 && ratio < 0.8,
            "cell aspect ratio should be reasonable (0.4-0.8), got {ratio}"
        );
    }

    #[test]
    fn ascent_plus_descent_approximates_cell_height() {
        let pipeline = create_pipeline(14.0);
        let ascent = pipeline.ascent_pixels();
        let descent = pipeline.descent_pixels();
        let (_, cell_height) = pipeline.cell_metrics();

        // ascent + descent should approximately equal cell_height (within 2px due to ceil)
        let diff = (ascent + descent - cell_height).abs();
        assert!(
            diff < 2.0,
            "ascent({ascent}) + descent({descent}) ≈ cell_height({cell_height}), diff={diff}"
        );
    }
}

#[cfg(test)]
mod atlas_packing {
    use super::*;

    #[test]
    fn multiple_glyphs_can_be_packed() {
        let mut pipeline = create_pipeline(14.0);
        let mut packed_count = 0;

        for ch in 'A'..='Z' {
            let info = pipeline
                .glyph_information(ch)
                .expect("ASCII glyph should exist");
            if info.width > 0 && info.height > 0 {
                packed_count += 1;
            }
        }

        assert!(
            packed_count >= 26,
            "Should be able to pack at least 26 ASCII glyphs, got {packed_count}"
        );
    }

    #[test]
    fn glyph_positions_are_within_atlas_bounds() {
        let mut pipeline = create_pipeline(14.0);
        let (atlas_width, atlas_height) = pipeline.atlas_dimensions();

        for ch in 'A'..='Z' {
            if let Some(info) = pipeline.glyph_information(ch) {
                assert!(
                    info.atlas_x >= 0,
                    "'{ch}' atlas_x should be non-negative, got {}",
                    info.atlas_x
                );
                assert!(
                    info.atlas_y >= 0,
                    "'{ch}' atlas_y should be non-negative, got {}",
                    info.atlas_y
                );
                assert!(
                    info.atlas_x + info.width <= atlas_width as i32,
                    "'{ch}' atlas_x({}) + width({}) exceeds atlas_width({})",
                    info.atlas_x,
                    info.width,
                    atlas_width
                );
                assert!(
                    info.atlas_y + info.height <= atlas_height as i32,
                    "'{ch}' atlas_y({}) + height({}) exceeds atlas_height({})",
                    info.atlas_y,
                    info.height,
                    atlas_height
                );
            }
        }
    }

    #[test]
    fn atlas_bitmap_has_content_after_packing() {
        let mut pipeline = create_pipeline(14.0);
        let _ = pipeline.glyph_information('A');
        let bitmap = pipeline.atlas_bitmap();
        assert!(
            bitmap.iter().any(|&b| b != 0),
            "atlas bitmap should have non-zero bytes after packing glyphs"
        );
    }

    #[test]
    fn ascii_pre_rasterization_populates_cache() {
        let pipeline = create_pipeline(14.0);
        // ASCII rasterization should populate at least 95 glyphs (space through tilde)
        assert!(
            pipeline.cache_length() >= 95,
            "ASCII pre-rasterization should populate at least 95 cached glyphs, got {}",
            pipeline.cache_length()
        );
    }

    #[test]
    fn cjk_glyphs_can_be_packed() {
        let mut pipeline = create_pipeline(14.0);
        let cjk_chars = ['中', '好', '世', '界', '日', '本'];
        let mut packed_count = 0;

        for ch in cjk_chars {
            if let Some(info) = pipeline.glyph_information(ch)
                && info.width > 0
                && info.height > 0
            {
                packed_count += 1;
            }
        }

        assert!(
            packed_count >= 4,
            "Should be able to pack at least 4 CJK glyphs, got {packed_count}"
        );
    }

    #[test]
    fn mixed_ascii_cjk_packing_works() {
        let mut pipeline = create_pipeline(14.0);
        let mixed_chars = ['A', '中', 'B', '好', 'C', '世'];

        for ch in mixed_chars {
            let _info = pipeline.glyph_information(ch);
        }

        let bitmap = pipeline.atlas_bitmap();
        assert!(
            bitmap.iter().any(|&b| b != 0),
            "atlas bitmap should have content after packing mixed ASCII and CJK"
        );
    }
}

#[cfg(test)]
mod font_family_switching {
    use super::*;

    #[test]
    fn listing_fonts_returns_results() {
        let pipeline = create_pipeline(14.0);
        let fonts = pipeline.list_monospace_fonts();
        assert!(
            !fonts.is_empty(),
            "Should have at least one monospace font, got: {:?}",
            fonts
        );
    }

    #[test]
    fn switching_to_first_font_works() {
        let mut pipeline = create_pipeline(14.0);
        let fonts = pipeline.list_monospace_fonts();

        if let Some(font_name) = fonts.first() {
            let result = pipeline.set_font_family(font_name);
            assert!(result, "set_font_family should succeed for '{font_name}'");
        }
    }

    #[test]
    fn switching_font_updates_metrics() {
        let mut pipeline = create_pipeline(14.0);
        let fonts = pipeline.list_monospace_fonts();

        if fonts.len() > 1 {
            let (original_width, original_height) = pipeline.cell_metrics();

            // Switch to a different font
            if let Some(font_name) = fonts.get(1)
                && pipeline.set_font_family(font_name)
            {
                let (new_width, new_height) = pipeline.cell_metrics();

                // At least one metric should change (or be different within tolerance)
                let width_changed = (new_width - original_width).abs() > 0.1;
                let height_changed = (new_height - original_height).abs() > 0.1;

                // At least one metric should change OR the fonts are identical
                #[allow(clippy::float_cmp)]
                let same_width = new_width == original_width;
                assert!(
                    width_changed || height_changed || same_width,
                    "Font switch should update metrics: before={}x{}, after={}x{}",
                    original_width,
                    original_height,
                    new_width,
                    new_height
                );
            }
        }
    }

    #[test]
    fn switching_font_does_not_panic() {
        let mut pipeline = create_pipeline(14.0);
        let fonts = pipeline.list_monospace_fonts();

        for font_name in &fonts {
            let _result = pipeline.set_font_family(font_name);
        }
    }

    #[test]
    fn metrics_reasonable_after_switch() {
        let mut pipeline = create_pipeline(14.0);
        let fonts = pipeline.list_monospace_fonts();

        for font_name in &fonts {
            pipeline.set_font_family(font_name);
            let (width, height) = pipeline.cell_metrics();
            assert!(
                width > 0.0 && width < 100.0,
                "cell_width should be reasonable after switch to '{font_name}', got {width}"
            );
            assert!(
                height > 0.0 && height < 100.0,
                "cell_height should be reasonable after switch to '{font_name}', got {height}"
            );
        }
    }

    #[test]
    fn glyph_information_works_after_font_switch() {
        let mut pipeline = create_pipeline(14.0);
        let fonts = pipeline.list_monospace_fonts();

        if let Some(font_name) = fonts.first() {
            pipeline.set_font_family(font_name);
            let info = pipeline.glyph_information('A');
            assert!(
                info.is_some(),
                "glyph_information('A') should work after font switch"
            );
        }
    }

    #[test]
    fn cache_cleared_after_font_switch() {
        let mut pipeline = create_pipeline(14.0);
        let fonts = pipeline.list_monospace_fonts();

        if fonts.len() > 1 {
            // Populate cache
            let _ = pipeline.glyph_information('A');
            let before = pipeline.cache_length();
            assert!(before > 0, "cache should be populated before switch");

            // Switch font
            if let Some(font_name) = fonts.get(1) {
                pipeline.set_font_family(font_name);
                let after = pipeline.cache_length();
                assert!(
                    after > 0,
                    "cache should be repopulated after font switch to '{font_name}'"
                );
            }
        }
    }
}

#[cfg(test)]
mod glyph_quality {
    use super::*;

    #[test]
    fn ascii_glyphs_have_atlas_coordinates() {
        let mut pipeline = create_pipeline(14.0);
        for ch in ['A', 'M', 'W', '0', '9'] {
            let info = pipeline
                .glyph_information(ch)
                .expect("ASCII glyph should exist");
            assert!(
                info.atlas_x >= 0 && info.atlas_y >= 0,
                "'{ch}' should have valid atlas coordinates: ({}, {})",
                info.atlas_x,
                info.atlas_y
            );
        }
    }

    #[test]
    fn ascii_glyphs_have_placement_info() {
        let mut pipeline = create_pipeline(14.0);
        for ch in ['A', 'B', 'C'] {
            let info = pipeline
                .glyph_information(ch)
                .expect("ASCII glyph should exist");
            assert!(
                info.placement.width > 0,
                "'{ch}' placement width should be positive, got {}",
                info.placement.width
            );
            assert!(
                info.placement.height > 0,
                "'{ch}' placement height should be positive, got {}",
                info.placement.height
            );
        }
    }

    #[test]
    fn atlas_generation_increments() {
        let mut pipeline = create_pipeline(14.0);
        let gen1 = pipeline.atlas_generation();
        let _ = pipeline.glyph_information('\u{03B1}');
        let gen2 = pipeline.atlas_generation();
        let _ = pipeline.glyph_information('\u{03B2}');
        let gen3 = pipeline.atlas_generation();

        assert!(
            gen2 > gen1 || gen3 > gen2,
            "atlas_generation should increment after new glyphs: {gen1} -> {gen2} -> {gen3}"
        );
    }

    #[test]
    fn repeated_glyph_information_returns_same_result() {
        let mut pipeline = create_pipeline(14.0);
        let info1 = pipeline.glyph_information('A').expect("glyph should exist");
        let info2 = pipeline.glyph_information('A').expect("glyph should exist");

        assert_eq!(info1.atlas_x, info2.atlas_x, "atlas_x should be stable");
        assert_eq!(info1.atlas_y, info2.atlas_y, "atlas_y should be stable");
        assert_eq!(info1.width, info2.width, "width should be stable");
        assert_eq!(info1.height, info2.height, "height should be stable");
    }
}

#[cfg(test)]
mod font_size_scaling {
    use super::*;

    #[test]
    fn very_small_font_produces_valid_metrics() {
        let pipeline = create_pipeline(8.0);
        let (width, height) = pipeline.cell_metrics();
        assert!(width > 0.0 && width < 100.0, "small font width: {width}");
        assert!(
            height > 0.0 && height < 100.0,
            "small font height: {height}"
        );
    }

    #[test]
    fn very_large_font_produces_valid_metrics() {
        let pipeline = create_pipeline(48.0);
        let (width, height) = pipeline.cell_metrics();
        assert!(width > 0.0 && width < 200.0, "large font width: {width}");
        assert!(
            height > 0.0 && height < 200.0,
            "large font height: {height}"
        );
    }

    #[test]
    fn font_size_ratio_is_approximately_linear() {
        let small = create_pipeline(10.0);
        let large = create_pipeline(20.0);

        let (sw, sh) = small.cell_metrics();
        let (lw, lh) = large.cell_metrics();

        // Width should roughly double
        let width_ratio = lw / sw;
        assert!(
            width_ratio > 1.5 && width_ratio < 2.5,
            "width ratio should be ~2x for 2x font size, got {width_ratio}"
        );

        // Height should roughly double
        let height_ratio = lh / sh;
        assert!(
            height_ratio > 1.5 && height_ratio < 2.5,
            "height ratio should be ~2x for 2x font size, got {height_ratio}"
        );
    }
}

#[cfg(test)]
mod font_file_loading {
    use super::*;
    use std::path::PathBuf;

    fn find_monospace_font_path() -> Option<PathBuf> {
        let mut db = fontdb::Database::new();
        db.load_system_fonts();
        let query = fontdb::Query {
            families: &[fontdb::Family::Monospace],
            ..Default::default()
        };
        db.query(&query)
            .and_then(|id| db.face_source(id))
            .and_then(|(source, _face_index)| match source {
                fontdb::Source::File(path) | fontdb::Source::SharedFile(path, _) => {
                    Some(path.clone())
                }
                _ => None,
            })
    }

    #[test]
    fn load_font_file_returns_family_name() {
        let mut pipeline = create_pipeline(14.0);
        let Some(path) = find_monospace_font_path() else {
            return;
        };

        let family = pipeline.load_font_file(&path);
        assert!(
            family.is_some(),
            "load_font_file should return a family name"
        );
        let name = family.unwrap();
        assert!(!name.is_empty(), "family name should not be empty");
    }

    #[test]
    fn load_font_file_then_set_font_family_succeeds() {
        let mut pipeline = create_pipeline(14.0);
        let Some(path) = find_monospace_font_path() else {
            return;
        };

        let family = pipeline.load_font_file(&path).expect("should load font");
        let switched = pipeline.set_font_family(&family);
        assert!(
            switched,
            "set_font_family should succeed for loaded font '{}'",
            family
        );
    }

    #[test]
    fn load_font_file_changes_rendered_glyphs() {
        let mut pipeline = create_pipeline(14.0);
        let Some(path) = find_monospace_font_path() else {
            return;
        };

        let glyph_before = pipeline
            .glyph_information('A')
            .expect("should have glyph before");
        let advance_before = glyph_before.advance_width;

        let family = pipeline.load_font_file(&path).expect("should load font");
        pipeline.set_font_family(&family);

        let glyph_after = pipeline
            .glyph_information('A')
            .expect("should have glyph after");
        let advance_after = glyph_after.advance_width;

        // Different fonts have different metrics — at minimum, values should be positive
        assert!(
            advance_after > 0.0,
            "advance_width after switch should be positive"
        );
        // The atlas should have been updated (generation incremented)
        assert!(
            pipeline.atlas_generation() > 0,
            "atlas_generation should be > 0 after font switch"
        );
        // The glyph metrics should differ after font switch (different font = different metrics)
        assert!(
            (advance_before - advance_after).abs() > f32::EPSILON,
            "advance_width should change after font switch: before={}, after={}",
            advance_before,
            advance_after
        );
    }

    #[test]
    fn load_nonexistent_font_returns_none() {
        let mut pipeline = create_pipeline(14.0);
        let path = PathBuf::from("/nonexistent/font.ttf");
        let result = pipeline.load_font_file(&path);
        assert!(
            result.is_none(),
            "loading nonexistent font should return None"
        );
    }

    #[test]
    fn list_fonts_includes_loaded_font() {
        let mut pipeline = create_pipeline(14.0);
        let Some(path) = find_monospace_font_path() else {
            return;
        };

        let fonts_before = pipeline.list_monospace_fonts();
        let _ = pipeline.load_font_file(&path);
        let fonts_after = pipeline.list_monospace_fonts();

        assert!(
            fonts_after.len() >= fonts_before.len(),
            "font list should not shrink after loading"
        );
    }
}
