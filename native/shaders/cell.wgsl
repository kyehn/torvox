struct Uniforms {
    projection: mat4x4<f32>,
    atlas_size: vec2<f32>,
    raster_scale: f32,
    image_active: f32,
    default_bg_lo: vec2<f32>,
    default_bg_hi: vec2<f32>,
};

@group(0) @binding(0) var<uniform> uniforms: Uniforms;
@group(0) @binding(1) var atlas_texture: texture_2d<f32>;
@group(0) @binding(2) var atlas_sampler: sampler;

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) cell_uv: vec2<f32>,
    @location(1) fg_color: vec4<f32>,
    @location(2) bg_color: vec4<f32>,
    @location(3) has_glyph: f32,
    @location(4) bearing: vec2<f32>,
    @location(5) glyph_size_px: vec2<f32>,
    @location(6) quad_size: vec2<f32>,
    @location(7) uv_offset: vec2<f32>,
    @location(8) glyph_advance_w: f32,
    @location(9) flags: f32,
};

@vertex
fn vs_main(
    @location(0) offset: vec2<f32>,
    @location(1) quad_origin: vec2<f32>,
    @location(2) uv_offset: vec2<f32>,
    @location(3) uv_size: vec2<f32>,
    @location(4) fg_color: vec4<f32>,
    @location(5) bg_color: vec4<f32>,
    @location(6) quad_size: vec2<f32>,
    @location(7) flags: f32,
    @location(8) bearing: vec2<f32>,
    @location(9) glyph_advance_w: f32,
) -> VertexOutput {
    let half_quad = quad_size * 0.5;
    let world_pos = quad_origin + half_quad + offset * half_quad;
    let clip_pos = uniforms.projection * vec4<f32>(world_pos, 0.0, 1.0);

    var output: VertexOutput;
    output.position = clip_pos;
    let uv_corner = offset * 0.5 + vec2<f32>(0.5);
    output.cell_uv = uv_corner;
    output.fg_color = fg_color;
    output.bg_color = bg_color;
    output.has_glyph = f32(uv_size.x * uv_size.y > 0.0);
    output.bearing = bearing;
    output.glyph_size_px = uv_size * uniforms.atlas_size;
    output.quad_size = quad_size;
    output.uv_offset = uv_offset;
    output.glyph_advance_w = glyph_advance_w;
    output.flags = flags;
    return output;
}

@fragment
fn fs_main(
    @location(0) cell_uv: vec2<f32>,
    @location(1) fg_color: vec4<f32>,
    @location(2) bg_color: vec4<f32>,
    @location(3) has_glyph: f32,
    @location(4) bearing: vec2<f32>,
    @location(5) glyph_size_px: vec2<f32>,
    @location(6) quad_size: vec2<f32>,
    @location(7) uv_offset: vec2<f32>,
    @location(8) glyph_advance_w: f32,
    @location(9) flags: f32,
) -> @location(0) vec4<f32> {
    var color: vec4<f32>;
    // Glyph coverage (0 outside the glyph bitmap). Used for fix F so a
    // default-background cell can show only the glyph over the wallpaper.
    var glyph_coverage: f32 = 0.0;
    if has_glyph > 0.5 {
        // World-space cell pixel coordinates scaled into the glyph atlas pixel
        // space (rasterized at font_size * raster_scale). This keeps glyph
        // bitmaps 1:1 with the physical surface so text is crisp (fix D).
        let cell_px = cell_uv * quad_size * uniforms.raster_scale;
        // X: scale glyph width to match cell width (Termux canvas.scale equivalent).
        // Narrow glyphs (advance_w < cell_w) are stretched; wide glyphs compressed.
        let scaled_x = cell_px.x * glyph_advance_w / quad_size.x;
        // Y: use natural font metrics. bearing.y positions the glyph relative to
        // the cell top (ascent_px - placement.top). For CJK fallback glyphs where
        // placement.top > ascent_px, bearing.y is negative — the glyph extends
        // above the cell and the in_glyph check clips it naturally.
        // This matches Kitty/Ghostty: glyphs are NOT scaled vertically;
        // oversized glyphs are clipped symmetrically via bearing offset.
        let glyph_px = vec2<f32>(scaled_x - bearing.x, cell_px.y - bearing.y);
        let in_glyph = all(glyph_px >= vec2<f32>(0.0)) && all(glyph_px < glyph_size_px);
        if in_glyph {
            let corrected_uv = uv_offset + glyph_px / uniforms.atlas_size;
            let texel = textureSample(atlas_texture, atlas_sampler, corrected_uv);
            color = mix(bg_color, fg_color, texel.r);
            glyph_coverage = texel.r;
        } else {
            color = bg_color;
        }
    } else {
        color = bg_color;
    }

    let f = u32(flags);
    let deco_color = fg_color;
    let deco_thickness = 0.06;
    if (f & 64u) != 0u && cell_uv.y < deco_thickness {
        color = deco_color;
    }
    if (f & 32u) != 0u && abs(cell_uv.y - 0.5) < deco_thickness * 0.5 {
        color = deco_color;
    }
    if (f & 8u) != 0u && cell_uv.y > 1.0 - deco_thickness {
        color = deco_color;
    }
    if (f & 256u) != 0u && abs(cell_uv.y - 0.92) < deco_thickness * 0.4 {
        color = deco_color;
    }

    if (f & 128u) != 0u {
        color = vec4<f32>(color.rgb * 0.5, color.a);
    }

    // Fix F: when a background image is active, default-background cells are
    // made transparent so the wallpaper shows through. Only the glyph (if any)
    // is drawn over the wallpaper; the cell's background fills nothing.
    if (uniforms.image_active > 0.5) {
        let dbg = vec4<f32>(uniforms.default_bg_lo, uniforms.default_bg_hi);
        let is_default_bg = abs(bg_color.r - dbg.r) < 0.004
            && abs(bg_color.g - dbg.g) < 0.004
            && abs(bg_color.b - dbg.b) < 0.004;
        if (is_default_bg) {
            // Glyph (if present) over a transparent background -> wallpaper shows.
            color = vec4<f32>(fg_color.rgb, glyph_coverage);
        }
    }

    return color;
}
