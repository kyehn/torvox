struct Uniforms {
    projection: mat4x4<f32>,
    atlas_size: vec2<f32>,
    _padding: vec2<f32>,
};

@group(0) @binding(0) var<uniform> uniforms: Uniforms;
@group(0) @binding(1) var kgp_atlas: texture_2d<f32>;
@group(0) @binding(2) var kgp_sampler: sampler;

struct VertexInput {
    @location(0) quad_corner: vec2<f32>,
    @location(1) quad_origin: vec2<f32>,
    @location(2) quad_size: vec2<f32>,
    @location(3) atlas_offset: vec2<f32>,
    @location(4) atlas_region: vec2<f32>,
    @location(5) alpha: f32,
};

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) alpha: f32,
};

@vertex
fn vs_main(input: VertexInput) -> VertexOutput {
    let cell_center = input.quad_origin + input.quad_size * 0.5;
    let world_pos = cell_center + input.quad_corner * input.quad_size * 0.5;
    let uv = input.quad_corner * 0.5 + 0.5;

    var output: VertexOutput;
    output.position = uniforms.projection * vec4(world_pos, 0.0, 1.0);
    output.uv = input.atlas_offset + uv * input.atlas_region;
    output.alpha = input.alpha;
    return output;
}

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    let texel = textureSample(kgp_atlas, kgp_sampler, input.uv);
    return vec4(texel.rgb, texel.a * input.alpha);
}
