// Cell rendering shader - instanced quads for terminal cells
// Each instance represents one cell in the terminal grid

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) fg_color: vec4<f32>,
    @location(2) bg_color: vec4<f32>,
    @location(3) flags: f32,
};

struct Uniforms {
    projection: mat4x4<f32>,
    cell_size: vec2<f32>,
    atlas_size: vec2<f32>,
};

@group(0) @binding(0)
var<uniform> uniforms: Uniforms;

@group(0) @binding(1)
var atlas_texture: texture_2d<f32>;

@group(0) @binding(2)
var atlas_sampler: sampler;

// Vertex attributes for each quad corner
// Position is in grid coordinates (will be transformed to screen space)
struct VertexInput {
    @location(0) corner: vec2<f32>,      // -1 to 1 quad corner
    @location(1) cell_pos: vec2<f32>,    // grid position (x, y)
    @location(2) atlas_offset: vec2<f32>, // UV offset into atlas
    @location(3) atlas_size: vec2<f32>,   // size in atlas
    @location(4) fg_color: vec4<f32>,     // foreground color
    @location(5) bg_color: vec4<f32>,     // background color
    @location(6) flags: f32,              // cell flags (bold, italic, etc.)
};

@vertex
fn vs_main(in: VertexInput) -> VertexOutput {
    var out: VertexOutput;

    // Transform grid position to screen space
    let screen_pos = (in.cell_pos + in.corner * 0.5) * uniforms.cell_size;

    // Apply projection matrix
    out.position = uniforms.projection * vec4<f32>(screen_pos, 0.0, 1.0);

    // Calculate UV coordinates into atlas
    // Corner is -1 to 1, map to 0 to 1
    let uv_corner = (in.corner + 1.0) * 0.5;
    out.uv = in.atlas_offset + uv_corner * in.atlas_size;

    out.fg_color = in.fg_color;
    out.bg_color = in.bg_color;
    out.flags = in.flags;

    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    // Sample atlas texture
    let atlas_sample = textureSample(atlas_texture, atlas_sampler, in.uv);

    // Alpha is the glyph coverage
    let glyph_alpha = atlas_sample.a;

    // Blend: background where no glyph, foreground where glyph exists
    let color = mix(in.bg_color.rgb, in.fg_color.rgb, glyph_alpha);

    return vec4<f32>(color, 1.0);
}
