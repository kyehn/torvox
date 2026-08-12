struct Uniforms {
    projection: mat4x4<f32>,
    image_size: vec2<f32>,
    blur_radius: f32,
    alpha: f32,
    texel_size: vec2<f32>,
    surface_size: vec2<f32>,
};

@group(0) @binding(0) var<uniform> uniforms: Uniforms;
@group(0) @binding(1) var bg_texture: texture_2d<f32>;
@group(0) @binding(2) var bg_sampler: sampler;

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) uv_surface: vec2<f32>,
};

@vertex
fn vs_main(@location(0) pos: vec2<f32>) -> VertexOutput {
    var out: VertexOutput;
    out.position = vec4<f32>(pos, 0.0, 1.0);
    // NDC y=+1 is the top of the screen; texture v=0 is the first (top)
    // row. Flipping v keeps the wallpaper upright (round-211, verified by
    // quadrant-color pixel checks on the emulator).
    let uv_surface = vec2<f32>(pos.x * 0.5 + 0.5, 0.5 - pos.y * 0.5);
    // Cover (center-crop) mapping: scale the source image so it covers the
    // surface, cropping the overflow (round-211 stretch -> center-crop).
    // Both sizes are pixels, normalized to the surface height (=1) before
    // comparing aspect ratios. A zero image_size (no image set) degenerates
    // to a plain stretch so sampling stays defined.
    let has_image = (uniforms.image_size.x > 0.0) && (uniforms.image_size.y > 0.0);
    let surface_aspect = uniforms.surface_size.x / uniforms.surface_size.y;
    let image_aspect = uniforms.image_size.x / uniforms.image_size.y;
    let cover_scale = max(surface_aspect / image_aspect, 1.0);
    let image_w = image_aspect * cover_scale; // on-screen width (height = 1)
    let image_h = cover_scale;
    let offset_x = (image_w - surface_aspect) * 0.5;
    let offset_y = (image_h - 1.0) * 0.5;
    let screen = vec2<f32>(uv_surface.x * surface_aspect, uv_surface.y);
    let uv_cover = vec2<f32>((screen.x + offset_x) / image_w, (screen.y + offset_y) / image_h);
    out.uv = select(uv_surface, uv_cover, has_image);
    // Surface-normalized UV: the H->V blur intermediate texture is exactly
    // the surface size, so the V pass must sample it with a plain stretch.
    out.uv_surface = uv_surface;
    return out;
}

fn gaussian(x: f32, sigma: f32) -> f32 {
    // sigma = 0 (blur_radius = 0) must degenerate to a pure center sample:
    // exp(-0.5*x*x/0) is NaN in WGSL, which makes the whole blur output
    // undefined (wallpaper invisible — emulator-verified, round-203).
    if (sigma < 0.001) {
        return select(0.0, 1.0, abs(x) < 0.5);
    }
    return exp(-0.5 * x * x / (sigma * sigma));
}

// Horizontal blur pass: samples along X axis, outputs to intermediate texture
@fragment
fn fs_blur_h(@location(0) uv: vec2<f32>) -> @location(0) vec4<f32> {
    let r = uniforms.blur_radius;
    let sigma = max(r * 0.5, 0.001);
    let half_kernel = i32(ceil(r));
    var color_sum = vec3<f32>(0.0);
    var weight_sum = 0.0;
    for (var dx = -half_kernel; dx <= half_kernel; dx++) {
        let x = f32(dx);
        let w = gaussian(x, sigma);
        let offset_uv = uv + vec2<f32>(f32(dx) * uniforms.texel_size.x, 0.0);
        let clamped_uv = clamp(offset_uv, vec2<f32>(0.0), vec2<f32>(1.0));
        color_sum += textureSample(bg_texture, bg_sampler, clamped_uv).rgb * w;
        weight_sum += w;
    }
    return vec4<f32>(color_sum / weight_sum, 1.0);
}

// Vertical blur pass: reads the H-pass intermediate texture (which is
// surface-sized), samples along Y axis, composites with alpha. The V pass
// samples the intermediate with a surface-normalized UV, NOT the cover UV:
// the intermediate already holds the cover-cropped image at surface size.
@fragment
fn fs_blur_v(@location(1) uv_surface: vec2<f32>) -> @location(0) vec4<f32> {
    let r = uniforms.blur_radius;
    let sigma = max(r * 0.5, 0.001);
    let half_kernel = i32(ceil(r));
    var color_sum = vec3<f32>(0.0);
    var weight_sum = 0.0;
    for (var dy = -half_kernel; dy <= half_kernel; dy++) {
        let y = f32(dy);
        let w = gaussian(y, sigma);
        let offset_uv = uv_surface + vec2<f32>(0.0, f32(dy) * uniforms.texel_size.y);
        let clamped_uv = clamp(offset_uv, vec2<f32>(0.0), vec2<f32>(1.0));
        color_sum += textureSample(bg_texture, bg_sampler, clamped_uv).rgb * w;
        weight_sum += w;
    }
    return vec4<f32>(color_sum / weight_sum, uniforms.alpha);
}

// No-blur pass: direct sample with alpha (cover UV into the source image)
@fragment
fn fs_direct(@location(0) uv: vec2<f32>) -> @location(0) vec4<f32> {
    let color = textureSample(bg_texture, bg_sampler, uv);
    return vec4<f32>(color.rgb, uniforms.alpha);
}
