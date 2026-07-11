use std::process::{Command, Stdio};

use swash::scale::image::Content;
use swash::scale::{Render, ScaleContext, Source};

struct RenderResult {
    glyphs: Vec<GlyphImage>,
    total_width: f32,
}

struct GlyphImage {
    pixels: Vec<u8>,
    width: u32,
    height: u32,
    x_offset: f32,
    y_offset: f32,
}

/// Render "Hello Torvox" using swash, save PNG, OCR-verify with RapidOCR.
/// Tests the same glyph pipeline as the terminal renderer end-to-end.
#[test]
fn text_render_ocr_verify() {
    let text = "Hello Torvox";
    let font_size = 48.0;

    let render = render_text_with_swash(text, font_size)
        .expect("no monospace font found — add dejavu_fonts or liberation to flake.nix");
    let glyphs = render.glyphs;
    let total_width_f32 = render.total_width;

    if glyphs.is_empty() {
        panic!("No glyphs rendered from '{text}' — font may not contain ASCII");
    }

    let total_width = total_width_f32.ceil() as u32 + 10;
    let total_height = (font_size * 2.0).ceil() as u32;
    let mut buf = vec![0u8; (total_width * total_height * 4) as usize];

    for glyph in &glyphs {
        for y in 0..glyph.height {
            for x in 0..glyph.width {
                let img_x = (glyph.x_offset + x as f32) as i32;
                let img_y = (glyph.y_offset + y as f32) as i32;
                if img_x >= 0
                    && img_x < total_width as i32
                    && img_y >= 0
                    && img_y < total_height as i32
                {
                    let src = ((y * glyph.width + x) * 4) as usize;
                    let dst = ((img_y as u32 * total_width + img_x as u32) * 4) as usize;
                    if dst + 3 < buf.len() && src + 3 < glyph.pixels.len() {
                        let a = glyph.pixels[src + 3];
                        if a > 0 {
                            // White glyph over black background: result = glyph * alpha
                            buf[dst] = a; // R
                            buf[dst + 1] = a; // G
                            buf[dst + 2] = a; // B
                            buf[dst + 3] = 255;
                        }
                    }
                }
            }
        }
    }

    let screenshot_dir = std::path::Path::new("screenshots");
    let _ = std::fs::create_dir_all(screenshot_dir);
    let png_path = screenshot_dir.join("text_ocr_test.png");

    let img = image::RgbaImage::from_raw(total_width, total_height, buf).expect("create RgbaImage");
    img.save(&png_path).expect("save PNG");

    // Preprocess: invert + threshold for better OCR
    let preprocess_path = screenshot_dir.join("text_ocr_test_pre.png");
    let _ = Command::new("magick")
        .args([
            &png_path.to_string_lossy(),
            "-negate",
            "-threshold",
            "50%",
            &preprocess_path.to_string_lossy(),
        ])
        .output();

    let ocr_input = if preprocess_path.exists() {
        &preprocess_path
    } else {
        &png_path
    };

    let output = Command::new("rapidocr")
        .arg("-img")
        .arg(ocr_input.as_os_str())
        .stdin(Stdio::null())
        .output()
        .expect("failed to run rapidocr CLI");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let cleaned: String = stdout
        .lines()
        .filter(|l| !l.contains("onnxruntime") && !l.contains("pci_bus_id"))
        .collect::<Vec<_>>()
        .join("")
        .trim()
        .to_string();

    assert!(
        cleaned != "NO_TEXT_FOUND" && !cleaned.is_empty(),
        "RapidOCR detected no text in rendered image. stdout: {stdout}"
    );

    assert!(
        cleaned.contains("Hello") || cleaned.contains("Torvox"),
        "Expected 'Hello Torvox' in OCR output, got: {cleaned:?}"
    );

    eprintln!("✓ Text OCR test passed: detected {cleaned:?}");
}

/// Render text with the swash pipeline (same as terminal renderer).
fn render_text_with_swash(text: &str, font_size: f32) -> Option<RenderResult> {
    // Try direct paths first (most reliable)
    let candidates = [
        "/usr/share/fonts/truetype/liberation/LiberationMono-Regular.ttf",
        "/usr/share/fonts/truetype/dejavu/DejaVuSansMono.ttf",
    ];
    for path in &candidates {
        let p = std::path::Path::new(path);
        if p.exists() {
            if let Ok(data) = std::fs::read(p) {
                if let Some(font_ref) = swash::FontRef::from_index(&data, 0) {
                    return Some(render_from_font_ref(font_ref, text, font_size));
                }
            }
        }
    }

    // Fallback: fontdb with system fonts
    let mut db = fontdb::Database::new();
    db.load_system_fonts();
    let query = fontdb::Query {
        families: &[fontdb::Family::Monospace],
        ..Default::default()
    };
    if let Some(font_id) = db.query(&query) {
        let result = db.with_face_data(font_id, |font_data, face_index| {
            render_with_swash(font_data, face_index as usize, text, font_size)
        });
        if let Some(Some(r)) = result {
            return Some(r);
        }
    }

    None
}

fn render_with_swash(
    font_data: &[u8],
    face_index: usize,
    text: &str,
    font_size: f32,
) -> Option<RenderResult> {
    let font_ref = swash::FontRef::from_index(font_data, face_index)?;
    Some(render_from_font_ref(font_ref, text, font_size))
}

fn render_from_font_ref(font_ref: swash::FontRef<'_>, text: &str, font_size: f32) -> RenderResult {
    let mut scale_ctx = ScaleContext::new();
    let mut scaler = scale_ctx
        .builder(font_ref)
        .size(font_size)
        .hint(false)
        .build();

    let charmap = font_ref.charmap();
    let metrics = font_ref.metrics(&[]);
    let upem = metrics.units_per_em as f32;
    let scale = if upem > 0.0 {
        font_size / upem
    } else {
        font_size
    };
    let baseline = (metrics.ascent * scale).abs();

    let mut glyphs = Vec::new();
    let mut cursor_x: f32 = 0.0;

    for ch in text.chars() {
        let gid = charmap.map(ch);
        let advance = font_ref.glyph_metrics(&[]).advance_width(gid) * scale;

        if let Some(img) = Render::new(&[Source::Outline]).render(&mut scaler, gid) {
            let w = img.placement.width as u32;
            let h = img.placement.height as u32;
            if w > 0 && h > 0 && matches!(img.content, Content::Mask) {
                let mut pixels = Vec::with_capacity((w * h * 4) as usize);
                for y in 0..h {
                    for x in 0..w {
                        let alpha = img.data.get((y * w + x) as usize).copied().unwrap_or(0);
                        pixels.extend_from_slice(&[255, 255, 255, alpha]);
                    }
                }
                glyphs.push(GlyphImage {
                    pixels,
                    width: w,
                    height: h,
                    x_offset: cursor_x + img.placement.left as f32,
                    y_offset: baseline - img.placement.top as f32,
                });
            }
        }
        cursor_x += advance;
    }

    RenderResult {
        glyphs,
        total_width: cursor_x,
    }
}
