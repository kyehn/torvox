//! GPU 渲染管线：着色器编译、绑定组与绘制调用。
use crate::render::Renderer;

pub(crate) const QUAD_VERTEX_COUNT: u32 = 6;

/// 终端单元着色器源码，管线创建与着色器校验测试共用。
pub(crate) const CELL_SHADER: &str = include_str!("../../shaders/cell.wgsl");

/// Kitty 图形协议着色器源码。
pub(crate) const KGP_SHADER: &str = include_str!("../../shaders/kitty_graphics.wgsl");

pub(crate) const QUAD_CORNERS: &[[f32; 2]; 6] = &[
    [-1.0, -1.0],
    [1.0, -1.0],
    [-1.0, 1.0],
    [-1.0, 1.0],
    [1.0, -1.0],
    [1.0, 1.0],
];

pub(crate) fn quad_corner_buffer_layout() -> wgpu::VertexBufferLayout<'static> {
    wgpu::VertexBufferLayout {
        array_stride: std::mem::size_of::<[f32; 2]>() as wgpu::BufferAddress,
        step_mode: wgpu::VertexStepMode::Vertex,
        attributes: &[wgpu::VertexAttribute {
            format: wgpu::VertexFormat::Float32x2,
            offset: 0,
            shader_location: 0,
        }],
    }
}

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct GpuUniforms {
    pub projection: [[f32; 4]; 4],
    pub atlas_size: [f32; 2],
    pub raster_scale: f32,
    /// std140 尾部对齐：uniform 结构体大小须为 16 的倍数（76 → 80）。否则着色器
    /// 越界读取，wgpu 丢弃所有实例绘制，字形渲染结果全为 0。
    pub _padding: f32,
}

impl Renderer {
    /// cell 与 KGP 管线共用的绑定组布局：0 = uniforms，1 = 采样 RGBA 纹理，
    /// 2 = 过滤采样器。单点构造以防两条文本管线走偏。
    pub(crate) fn text_bind_group_layout(
        device: &wgpu::Device,
        label: &str,
    ) -> wgpu::BindGroupLayout {
        device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some(label),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        })
    }

    /// 两条文本管线只差实例顶点布局与混合方式；入口点、图元状态、无深度全部
    /// 一致，单点构造以防走偏。
    fn create_text_pipeline(
        device: &wgpu::Device,
        label: &str,
        bind_group_layout: &wgpu::BindGroupLayout,
        shader: &wgpu::ShaderModule,
        instance_layout: wgpu::VertexBufferLayout<'static>,
        blend: Option<wgpu::BlendState>,
        format: wgpu::TextureFormat,
    ) -> wgpu::RenderPipeline {
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some(&format!("{label} Layout")),
            bind_group_layouts: &[Some(bind_group_layout)],
            immediate_size: 0,
        });
        device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some(label),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: shader,
                entry_point: Some("vs_main"),
                buffers: &[Some(quad_corner_buffer_layout()), Some(instance_layout)],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None,
                polygon_mode: wgpu::PolygonMode::Fill,
                unclipped_depth: false,
                conservative: false,
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        })
    }

    pub(crate) fn create_cell_pipeline(
        device: &wgpu::Device,
        format: wgpu::TextureFormat,
    ) -> wgpu::RenderPipeline {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Cell Shader"),
            source: wgpu::ShaderSource::Wgsl(std::borrow::Cow::Borrowed(CELL_SHADER)),
        });
        let bind_group_layout = Self::text_bind_group_layout(device, "Cell Bind Group Layout");
        Self::create_text_pipeline(
            device,
            "Cell Pipeline",
            &bind_group_layout,
            &shader,
            crate::render::CellInstance::buffer_layout(),
            // 图集是预乘 alpha 的覆盖率遮罩，采样后须按 alpha 混合而非覆盖写。
            Some(wgpu::BlendState::ALPHA_BLENDING),
            format,
        )
    }

    pub(crate) fn create_kgp_pipeline(
        device: &wgpu::Device,
        format: wgpu::TextureFormat,
    ) -> (wgpu::RenderPipeline, wgpu::BindGroupLayout) {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("KGP Shader"),
            source: wgpu::ShaderSource::Wgsl(std::borrow::Cow::Borrowed(KGP_SHADER)),
        });
        let bind_group_layout = Self::text_bind_group_layout(device, "KGP Bind Group Layout");
        let pipeline = Self::create_text_pipeline(
            device,
            "KGP Pipeline",
            &bind_group_layout,
            &shader,
            crate::render::KittyGraphicsInstance::buffer_layout(),
            // KGP 图像可能带 alpha（半透明 PNG）；REPLACE 会在透明区域绘出黑边。
            Some(wgpu::BlendState::ALPHA_BLENDING),
            format,
        );
        (pipeline, bind_group_layout)
    }

    pub(crate) fn ensure_kgp_pipeline(&mut self, config_width: u32, config_height: u32) {
        if self.kgp_texture.is_none() {
            return;
        }
        let format = self
            .surface_config
            .as_ref()
            .map_or(wgpu::TextureFormat::Rgba8Unorm, |surface_config| {
                surface_config.format
            });

        if self.kgp_pipeline.is_none() || self.kgp_pipeline_format != Some(format) {
            let (pipeline, layout) = Self::create_kgp_pipeline(&self.device, format);
            self.kgp_pipeline = Some(pipeline);
            self.kgp_pipeline_format = Some(format);
            self.kgp_bind_group_layout = Some(layout);
            // 旧管线/布局派生的绑定组不可复用。
            self.kgp_bind_group = None;
        }

        if self.kgp_sampler.is_none() {
            self.kgp_sampler = Some(self.device.create_sampler(&wgpu::SamplerDescriptor {
                address_mode_u: wgpu::AddressMode::ClampToEdge,
                address_mode_v: wgpu::AddressMode::ClampToEdge,
                mag_filter: wgpu::FilterMode::Linear,
                min_filter: wgpu::FilterMode::Linear,
                ..Default::default()
            }));
        }
        let sampler = match self.kgp_sampler.as_ref() {
            Some(s) => s,
            None => return,
        };

        if self.kgp_uniform_buffer.is_none() {
            self.kgp_uniform_buffer = Some(self.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("KGP Uniform Buffer"),
                size: std::mem::size_of::<GpuUniforms>() as u64,
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }));
        }
        let uniform_buffer = match self.kgp_uniform_buffer.as_ref() {
            Some(b) => b,
            None => return,
        };

        let uniforms = self.cell_uniforms(
            config_width as f32,
            config_height as f32,
            self.kgp_atlas_width as f32,
            self.kgp_atlas_height as f32,
        );
        self.queue
            .write_buffer(uniform_buffer, 0, bytemuck::cast_slice(&[uniforms]));

        let texture = match self.kgp_texture.as_ref() {
            Some(t) => t,
            None => return,
        };
        let pipeline = match self.kgp_pipeline.as_ref() {
            Some(p) => p,
            None => return,
        };

        // 绑定组只在其依赖的图集纹理变化时重建。
        //
        // 三项依赖里 uniform 内容每帧变（但它是缓冲绑定，不影响绑定组本身），
        // sampler 与 buffer 都在本函数里惰性创建一次；纹理则由 `set_kgp_atlas` 在
        // 新图集到达时替换并把本字段置空。此前无条件重建，等于每帧多一次
        // `create_view` 与 `create_bind_group`——终端图像常驻（ssh -t、htop、动画）
        // 时这就是每帧两次 GPU 对象分配 + 一处驱动内对象表增长。
        if self.kgp_bind_group.is_some() {
            return;
        }
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        self.kgp_bind_group = Some(self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("KGP Bind Group"),
            layout: &pipeline.get_bind_group_layout(0),
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: uniform_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(sampler),
                },
            ],
        }));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wgpu::naga::front::wgsl;
    use wgpu::naga::valid::{Capabilities, ValidationFlags, Validator};

    /// 解析、校验与入口点一并断言，使语法错误、语义错误与入口缺失分别可见。
    fn assert_shader(name: &str, source: &str, expected_entry_points: &[&str]) {
        let module = wgsl::parse_str(source)
            .unwrap_or_else(|error| panic!("{name} 解析失败：{}", error.emit_to_string(source)));
        let mut validator = Validator::new(ValidationFlags::all(), Capabilities::default());
        validator
            .validate(&module)
            .unwrap_or_else(|error| panic!("{name} 校验失败：{}", error.emit_to_string(source)));
        let mut declared: Vec<&str> = module
            .entry_points
            .iter()
            .map(|entry| entry.name.as_str())
            .collect();
        declared.sort_unstable();
        assert_eq!(declared, expected_entry_points, "{name} 入口点不符");
    }

    #[test]
    fn cell_shader_declares_vertex_and_fragment_entry_points() {
        assert_shader("cell.wgsl", CELL_SHADER, &["fs_main", "vs_main"]);
    }

    #[test]
    fn kgp_shader_declares_vertex_and_fragment_entry_points() {
        assert_shader("kitty_graphics.wgsl", KGP_SHADER, &["fs_main", "vs_main"]);
    }
}
