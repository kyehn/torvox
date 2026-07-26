//! GPU render pipeline — shader compilation, bind groups, and draw calls.
use super::GpuContext;

pub(crate) const QUAD_VERTEX_COUNT: u32 = 6;
pub(crate) const DEFAULT_BG_ALPHA: f32 = 0.8;

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
    pub image_active: f32,
    pub default_bg: [f32; 4],
}

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct BgUniforms {
    pub projection: [[f32; 4]; 4],
    pub image_size: [f32; 2],
    pub blur_radius: f32,
    pub alpha: f32,
    pub texel_size: [f32; 2],
    pub _padding: [f32; 2],
}

pub fn image_active_value(bg_bind_group_present: bool) -> f32 {
    if bg_bind_group_present { 1.0 } else { 0.0 }
}

impl GpuContext {
    pub(crate) fn create_cell_pipeline(
        device: &wgpu::Device,
        format: wgpu::TextureFormat,
    ) -> wgpu::RenderPipeline {
        let wgsl_source = include_str!("../../shaders/cell.wgsl");
        let cell_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Cell Shader"),
            source: wgpu::ShaderSource::Wgsl(std::borrow::Cow::Borrowed(wgsl_source)),
        });

        let cell_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("Cell Bind Group Layout"),
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
            });

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
                    quad_corner_buffer_layout(),
                    super::CellInstance::buffer_layout(),
                ],
                compilation_options: Default::default(),
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
                compilation_options: Default::default(),
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

    pub(crate) fn create_bg_pipeline(
        device: &wgpu::Device,
        format: wgpu::TextureFormat,
    ) -> (wgpu::RenderPipeline, wgpu::BindGroupLayout) {
        let wgsl_source = include_str!("../../shaders/background.wgsl");
        let bg_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Background Shader"),
            source: wgpu::ShaderSource::Wgsl(std::borrow::Cow::Borrowed(wgsl_source)),
        });

        let bg_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("Background Bind Group Layout"),
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
            });

        let bg_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Background Pipeline Layout"),
            bind_group_layouts: &[Some(&bg_bind_group_layout)],
            immediate_size: 0,
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Background Pipeline"),
            layout: Some(&bg_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &bg_shader,
                entry_point: Some("vs_main"),
                buffers: &[quad_corner_buffer_layout()],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &bg_shader,
                entry_point: Some("fs_direct"),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
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

        (pipeline, bg_bind_group_layout)
    }

    pub(crate) fn create_kgp_pipeline(
        device: &wgpu::Device,
        format: wgpu::TextureFormat,
    ) -> (wgpu::RenderPipeline, wgpu::BindGroupLayout) {
        let wgsl_source = include_str!("../../shaders/kgp.wgsl");
        let kgp_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("KGP Shader"),
            source: wgpu::ShaderSource::Wgsl(std::borrow::Cow::Borrowed(wgsl_source)),
        });

        let kgp_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("KGP Bind Group Layout"),
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
            });

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
                    quad_corner_buffer_layout(),
                    super::KgpInstance::buffer_layout(),
                ],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &kgp_shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
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

    pub(crate) fn ensure_bg_pipeline(&mut self, surface_width: u32, surface_height: u32) {
        if self.bg_image_view.is_none() {
            return;
        }
        let format = self
            .surface_config
            .as_ref()
            .map_or(wgpu::TextureFormat::Rgba8Unorm, |c| c.format);

        if self.bg_pipeline.is_none() {
            let (pipeline, layout) = Self::create_bg_pipeline(&self.device, format);
            self.bg_pipeline = Some(pipeline);
            self.bg_bind_group_layout = Some(layout);
        }

        if self.blur_h_pipeline.is_none() {
            let blur_wgsl_source = include_str!("../../shaders/background.wgsl");
            let blur_shader = self
                .device
                .create_shader_module(wgpu::ShaderModuleDescriptor {
                    label: Some("Background Blur Shader"),
                    source: wgpu::ShaderSource::Wgsl(std::borrow::Cow::Borrowed(blur_wgsl_source)),
                });
            let bg_pipeline_layout = self.bg_bind_group_layout.as_ref().map(|layout| {
                self.device
                    .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                        label: Some("Blur Pipeline Layout"),
                        bind_group_layouts: &[Some(layout)],
                        immediate_size: 0,
                    })
            });
            let layout = bg_pipeline_layout
                .as_ref()
                .expect("blur pipeline layout created");
            self.blur_h_pipeline = Some(self.device.create_render_pipeline(
                &wgpu::RenderPipelineDescriptor {
                    label: Some("Background Blur H Pipeline"),
                    layout: Some(layout),
                    vertex: wgpu::VertexState {
                        module: &blur_shader,
                        entry_point: Some("vs_main"),
                        buffers: &[quad_corner_buffer_layout()],
                        compilation_options: Default::default(),
                    },
                    fragment: Some(wgpu::FragmentState {
                        module: &blur_shader,
                        entry_point: Some("fs_blur_h"),
                        targets: &[Some(wgpu::ColorTargetState {
                            format,
                            blend: Some(wgpu::BlendState::REPLACE),
                            write_mask: wgpu::ColorWrites::ALL,
                        })],
                        compilation_options: Default::default(),
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
                },
            ));
            self.blur_v_pipeline = Some(self.device.create_render_pipeline(
                &wgpu::RenderPipelineDescriptor {
                    label: Some("Background Blur V Pipeline"),
                    layout: Some(layout),
                    vertex: wgpu::VertexState {
                        module: &blur_shader,
                        entry_point: Some("vs_main"),
                        buffers: &[quad_corner_buffer_layout()],
                        compilation_options: Default::default(),
                    },
                    fragment: Some(wgpu::FragmentState {
                        module: &blur_shader,
                        entry_point: Some("fs_blur_v"),
                        targets: &[Some(wgpu::ColorTargetState {
                            format,
                            blend: Some(wgpu::BlendState::REPLACE),
                            write_mask: wgpu::ColorWrites::ALL,
                        })],
                        compilation_options: Default::default(),
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
                },
            ));
        }

        let pipeline = match self.bg_pipeline.as_ref() {
            Some(p) => p,
            None => return,
        };
        let layout = pipeline.get_bind_group_layout(0);

        let view = match self.bg_image_view.as_ref() {
            Some(v) => v,
            None => return,
        };

        if self.bg_sampler.is_none() {
            self.bg_sampler = Some(self.device.create_sampler(&wgpu::SamplerDescriptor {
                address_mode_u: wgpu::AddressMode::ClampToEdge,
                address_mode_v: wgpu::AddressMode::ClampToEdge,
                mag_filter: wgpu::FilterMode::Linear,
                min_filter: wgpu::FilterMode::Linear,
                ..Default::default()
            }));
        }
        let sampler = self
            .bg_sampler
            .as_ref()
            .expect("background sampler initialized in ensure_bg_pipeline");

        if self.bg_uniform_buffer.is_none() {
            self.bg_uniform_buffer = Some(self.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("Background Uniform Buffer"),
                size: std::mem::size_of::<BgUniforms>() as u64,
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }));
        }
        let buf = self
            .bg_uniform_buffer
            .as_ref()
            .expect("background uniform buffer initialized in ensure_bg_pipeline");

        let (blur, alpha) = (self.bg_blur_radius, self.bg_alpha);
        let texel_x = if surface_width > 0 {
            1.0 / surface_width as f32
        } else {
            0.0
        };
        let texel_y = if surface_height > 0 {
            1.0 / surface_height as f32
        } else {
            0.0
        };

        let proj = super::orthographic_projection(surface_width as f32, surface_height as f32);
        let uniforms = BgUniforms {
            projection: proj,
            image_size: [surface_width as f32, surface_height as f32],
            blur_radius: blur,
            alpha,
            texel_size: [texel_x, texel_y],
            _padding: [0.0; 2],
        };
        self.queue
            .write_buffer(buf, 0, bytemuck::cast_slice(&[uniforms]));

        self.bg_bind_group = Some(self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Background Bind Group"),
            layout: &layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: buf.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(sampler),
                },
            ],
        }));
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

        let proj = super::orthographic_projection(surface_width as f32, surface_height as f32);
        let uniforms = GpuUniforms {
            projection: proj,
            atlas_size: [self.kgp_atlas_width as f32, self.kgp_atlas_height as f32],
            raster_scale: self.raster_scale,
            image_active: image_active_value(self.bg_bind_group.is_some()),
            default_bg: [
                self.bg_color.r as f32,
                self.bg_color.g as f32,
                self.bg_color.b as f32,
                1.0,
            ],
        };
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
