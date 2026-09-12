//! GPU render pipeline — shader compilation, bind groups, and draw calls.
//!
//! # Requirements
//! - FR-050 — surface lifecycle: pipelines rebuilt when the surface is recreated
use crate::render::Renderer;

pub(crate) const QUAD_VERTEX_COUNT: u32 = 6;

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
    /// std140 trailing padding: uniform struct size must be a multiple of
    /// 16 (76 -> 80). Without it the shader reads past the buffer, wgpu
    /// drops every instance draw, and glyph renders come back all zeros.
    pub _padding: f32,
}

impl Renderer {
    /// Bind-group layout shared by the cell and KGP pipelines: binding 0 =
    /// uniforms, 1 = sampled RGBA texture, 2 = filtering sampler. One
    /// construction site so the two text pipelines cannot drift apart.
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

    pub(crate) fn create_cell_pipeline(
        device: &wgpu::Device,
        format: wgpu::TextureFormat,
    ) -> wgpu::RenderPipeline {
        let wgsl_source = include_str!("../../shaders/cell.wgsl");
        let cell_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Cell Shader"),
            source: wgpu::ShaderSource::Wgsl(std::borrow::Cow::Borrowed(wgsl_source)),
        });

        let cell_bind_group_layout = Self::text_bind_group_layout(device, "Cell Bind Group Layout");

        let cell_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Cell Pipeline Layout"),
            bind_group_layouts: &[Some(&cell_bind_group_layout)],
            immediate_size: 0,
        });

        device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Cell Pipeline"),
            layout: Some(&cell_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &cell_shader,
                entry_point: Some("vs_main"),
                buffers: &[
                    Some(quad_corner_buffer_layout()),
                    Some(crate::render::CellInstance::buffer_layout()),
                ],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &cell_shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: Some(wgpu::BlendState {
                        color: wgpu::BlendComponent {
                            src_factor: wgpu::BlendFactor::SrcAlpha,
                            dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                            operation: wgpu::BlendOperation::Add,
                        },
                        alpha: wgpu::BlendComponent {
                            src_factor: wgpu::BlendFactor::One,
                            dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                            operation: wgpu::BlendOperation::Add,
                        },
                    }),
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

    pub(crate) fn create_kgp_pipeline(
        device: &wgpu::Device,
        format: wgpu::TextureFormat,
    ) -> (wgpu::RenderPipeline, wgpu::BindGroupLayout) {
        let wgsl_source = include_str!("../../shaders/kitty_graphics.wgsl");
        let kgp_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("KGP Shader"),
            source: wgpu::ShaderSource::Wgsl(std::borrow::Cow::Borrowed(wgsl_source)),
        });

        let kgp_bind_group_layout = Self::text_bind_group_layout(device, "KGP Bind Group Layout");

        let kgp_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("KGP Pipeline Layout"),
            bind_group_layouts: &[Some(&kgp_bind_group_layout)],
            immediate_size: 0,
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("KGP Pipeline"),
            layout: Some(&kgp_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &kgp_shader,
                entry_point: Some("vs_main"),
                buffers: &[
                    Some(quad_corner_buffer_layout()),
                    Some(crate::render::KittyGraphicsInstance::buffer_layout()),
                ],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &kgp_shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    // kitty graphics protocol images may
                    // carry alpha (semi-transparent PNG); REPLACE painted
                    // the image RGB over the background, producing black
                    // fringes on transparent areas. SrcAlpha blend lets
                    // opacity apply correctly.
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
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
        });

        (pipeline, kgp_bind_group_layout)
    }

    pub(crate) fn ensure_kgp_pipeline(&mut self, surface_width: u32, surface_height: u32) {
        if self.kgp_texture.is_none() {
            return;
        }
        let format = self
            .surface_config
            .as_ref()
            .map_or(wgpu::TextureFormat::Rgba8Unorm, |c| c.format);

        if self.kgp_pipeline.is_none() {
            let (pipeline, layout) = Self::create_kgp_pipeline(&self.device, format);
            self.kgp_pipeline = Some(pipeline);
            self.kgp_bind_group_layout = Some(layout);
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
        let buf = match self.kgp_uniform_buffer.as_ref() {
            Some(b) => b,
            None => return,
        };

        let uniforms = self.cell_uniforms(
            surface_width as f32,
            surface_height as f32,
            self.kgp_atlas_width as f32,
            self.kgp_atlas_height as f32,
        );
        self.queue
            .write_buffer(buf, 0, bytemuck::cast_slice(&[uniforms]));

        let view = match self.kgp_texture.as_ref() {
            Some(t) => t.create_view(&wgpu::TextureViewDescriptor::default()),
            None => return,
        };

        let pipeline = match self.kgp_pipeline.as_ref() {
            Some(p) => p,
            None => return,
        };

        self.kgp_bind_group = Some(self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("KGP Bind Group"),
            layout: &pipeline.get_bind_group_layout(0),
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: buf.as_entire_binding(),
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
