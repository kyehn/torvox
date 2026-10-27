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
    @location(0) uv_local: vec2<f32>,
    @location(1) texel_rect: vec4<f32>,
    @location(2) alpha: f32,
};

@vertex
fn vs_main(input: VertexInput) -> VertexOutput {
    let cell_center = input.quad_origin + input.quad_size * 0.5;
    let world_pos = cell_center + input.quad_corner * input.quad_size * 0.5;
    let uv_local = input.quad_corner * 0.5 + 0.5;

    var output: VertexOutput;
    output.position = uniforms.projection * vec4(world_pos, 0.0, 1.0);
    output.uv_local = uv_local;
    // 本条目在图集里的纹素矩形（起点与尺寸）：图集里并排着多幅图像，
    // 采样必须知道当前条目自己的边界。
    output.texel_rect = vec4<f32>(
        input.atlas_offset * uniforms.atlas_size,
        input.atlas_region * uniforms.atlas_size,
    );
    output.alpha = input.alpha;
    return output;
}

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    let origin = input.texel_rect.xy;
    let extent = input.texel_rect.zw;
    // 双线性过滤在纹素边界之外会取邻纹素，而图集里相邻条目是**别的图像**：
    // 越过边界即串色——放大显示时左缘混入前一幅图的颜色，条目比图集矮时
    // 底缘混入图集里的透明行，图像出现彩边或发虚的半透明边。
    //
    // 把采样坐标钳到本条目的首末纹素中心即「按条目 clamp-to-edge」：边缘取
    // 本条目自己的边缘纹素，与在图集里给每个条目复制一圈边缘像素等价，
    // 且不占额外显存。
    let texel = clamp(
        origin + input.uv_local * extent,
        origin + vec2<f32>(0.5, 0.5),
        origin + extent - vec2<f32>(0.5, 0.5),
    );
    let color = textureSample(kgp_atlas, kgp_sampler, texel / uniforms.atlas_size);
    return vec4(color.rgb, color.a * input.alpha);
}
