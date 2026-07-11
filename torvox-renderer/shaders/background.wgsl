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

@fragment
fn fs_main(@location(0) uv: vec2<f32>) -> @location(0) vec4<f32> {
    let r = uniforms.blur_radius;
    if r < 0.5 {
        let color = textureSample(bg_texture, bg_sampler, uv);
        return vec4<f32>(color.rgb, uniforms.alpha);
    }
    let texel = uniforms.texel_size;
    var color_sum = vec3<f32>(0.0);
    var weight_sum = 0.0;
    let ir = i32(ceil(r));
    for (var dy = -ir; dy <= ir; dy++) {
        for (var dx = -ir; dx <= ir; dx++) {
            let offset = vec2<f32>(f32(dx), f32(dy)) * texel;
            let sample_uv = clamp(uv + offset, vec2<f32>(0.0), vec2<f32>(1.0));
            let dist = sqrt(f32(dx * dx + dy * dy));
            let weight = max(0.0, r + 0.5 - dist);
            color_sum += textureSample(bg_texture, bg_sampler, sample_uv).rgb * weight;
            weight_sum += weight;
        }
    }
    if weight_sum > 0.0 {
        color_sum /= weight_sum;
    }
    return vec4<f32>(color_sum, uniforms.alpha);
}
