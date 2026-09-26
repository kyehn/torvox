struct Uniforms {
    projection: mat4x4<f32>,
    atlas_size: vec2<f32>,
    raster_scale: f32,
    _padding: f32,
};

@group(0) @binding(0) var<uniform> uniforms: Uniforms;
@group(0) @binding(1) var atlas_texture: texture_2d<f32>;
@group(0) @binding(2) var atlas_sampler: sampler;

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) cell_uv: vec2<f32>,
    @location(1) foreground: vec4<f32>,
    @location(2) background: vec4<f32>,
    @location(10) deco_color: vec4<f32>,
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
    @location(4) foreground: vec4<f32>,
    @location(5) background: vec4<f32>,
    @location(10) deco_color: vec4<f32>,
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
    output.foreground = foreground;
    output.background = background;
    output.deco_color = deco_color;
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
    @location(1) foreground: vec4<f32>,
    @location(2) background: vec4<f32>,
    @location(10) deco_color: vec4<f32>,
    @location(3) has_glyph: f32,
    @location(4) bearing: vec2<f32>,
    @location(5) glyph_size_px: vec2<f32>,
    @location(6) quad_size: vec2<f32>,
    @location(7) uv_offset: vec2<f32>,
    @location(8) glyph_advance_w: f32,
    @location(9) flags: f32,
) -> @location(0) vec4<f32> {
    var color: vec4<f32>;
    // 字形覆盖率（字形位图之外为 0）。Fix F 用它判断默认背景单元格只绘制字形。
    var glyph_coverage: f32 = 0.0;
    if has_glyph > 0.5 {
        // World-space cell pixel coordinates in PHYSICAL surface pixels:
        // quad_size is already physical (surface/rows), so multiplying by
        // raster_scale again would shrink the glyph to font_size logical
        // pixels (glyphs rendered 25px instead of 66px at 420dpi).
        let cell_px = cell_uv * quad_size;
        // X: sample the atlas bitmap at its natural physical size. The quad
        // spans cell_w × cell_span physical px; the rasterized bitmap
        // (glyph_size_px, generated at font_size × raster_scale) sits at
        // bearing.x and is clipped by the in_glyph check below. Scaling by
        // glyph_advance/quad_size is 1.0 only for monospace Latin (advance ==
        // cell width); CJK quads (2 cells ≈ 44px) exceed the ~37px advance,
        // which stretched every CJK glyph ~1.19× horizontally (发虚, 与英文
        // 宽度不协调). 1:1 sampling keeps glyphs exactly at raster size.
        let scaled_x = cell_px.x;
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
            color = mix(background, foreground, texel.r);
            glyph_coverage = texel.r;
        } else {
            color = background;
        }
    } else {
        color = background;
    }

    let f = u32(flags);
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

    return color;
}
