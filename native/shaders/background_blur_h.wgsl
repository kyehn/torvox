struct Uniforms {
    projection: mat4x4<f32>,
    image_size: vec2<f32>,
    blur_radius: f32,
    alpha: f32,
    texel_size: vec2<f32>,
    _padding: vec2<f32>,
};

@group(0) @binding(0) var<uniform> uniforms: Uniforms;
@group(0) @binding(1) var bg_texture: texture_2d<f32>;
@group(0) @binding(2) var bg_sampler: sampler;

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn vs_main(@location(0) pos: vec2<f32>) -> VertexOutput {
    var out: VertexOutput;
    out.position = vec4<f32>(pos, 0.0, 1.0);
    out.uv = pos * 0.5 + vec2<f32>(0.5);
    return out;
}

fn gaussian(x: f32, sigma: f32) -> f32 {
    return exp(-0.5 * x * x / (sigma * sigma));
}

@fragment
fn fs_main(@location(0) uv: vec2<f32>) -> @location(0) vec4<f32> {
    let r = uniforms.blur_radius;
    if r < 0.5 {
        let ds_uv = uv + uniforms.texel_size;
        return textureSample(bg_texture, bg_sampler, ds_uv);
    }
    let sigma = r * 0.5;
    let half_kernel = i32(ceil(r));
    var color_sum = vec3<f32>(0.0);
    var weight_sum = 0.0;
    for (var dx = -half_kernel; dx <= half_kernel; dx++) {
        let x = f32(dx);
        let w = gaussian(x, sigma);
        let offset_uv = uv + vec2<f32>(f32(dx * 2) * uniforms.texel_size.x, 0.0);
        let clamped_uv = clamp(offset_uv, vec2<f32>(0.0), vec2<f32>(1.0));
        color_sum += textureSample(bg_texture, bg_sampler, clamped_uv).rgb * w;
        weight_sum += w;
    }
    return vec4<f32>(color_sum / weight_sum, 1.0);
}
