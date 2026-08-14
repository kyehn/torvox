//! GPU render pipeline — shader compilation, bind groups, and draw calls.
//!
//! # Requirements
//! - FR-050 — surface lifecycle: pipelines rebuilt when the surface is recreated
use crate::render::Renderer;

pub(crate) const QUAD_VERTEX_COUNT: u32 = 6;
pub(crate) const DEFAULT_BG_ALPHA: f32 = 0.8;
/// Bell-flash peak opacity in the 0-255 alpha space: a subtle
/// white flash; 96/255 ≈ 0.38 at the bell's first frame, decaying with the
/// phase driven by Kotlin). Scaled by phase before the shader uniform.
pub(crate) const BELL_FLASH_ALPHA_255: f32 = 96.0;
/// 255 as f32, for converting the 0-255 alpha constant to 0..=1.
pub(crate) const ALPHA_255_MAX: f32 = 255.0;

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
    /// Source image size in pixels — used by vs_main for the cover
    /// (center-crop) UV mapping. Distinct from `surface_size`.
    pub image_size: [f32; 2],
    pub blur_radius: f32,
    pub alpha: f32,
    pub texel_size: [f32; 2],
    /// Surface size in pixels — vs_main needs the surface aspect ratio to
    /// compute the cover crop, and `texel_size` is derived from it too.
    pub surface_size: [f32; 2],
}

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct FlashUniforms {
    /// Pre-scaled overlay alpha in 0..=1 (`BELL_FLASH_ALPHA_255/255 * phase`).
    pub alpha: f32,
    pub _padding: [f32; 3],
}

pub fn image_active_value(bg_bind_group_present: bool) -> f32 {
    if bg_bind_group_present { 1.0 } else { 0.0 }
}

impl Renderer {
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
                buffers: &[Some(quad_corner_buffer_layout())],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &bg_shader,
                entry_point: Some("fs_direct"),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    // Alpha blend so `uniforms.alpha` (background opacity
                    // setting) actually composites the wallpaper over the
                    // cleared bg_color. Was REPLACE: the alpha channel was
                    // written but the RGB never mixed, so the opacity
                    // slider had zero visual effect,
                    // emulator-verified: alpha 0.1 vs 0.8 produced
                    // byte-identical pixels).
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
        });

        (pipeline, bg_bind_group_layout)
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

    /// Full-screen white-overlay pipeline for the bell flash. No textures —
    /// a single uniform float (pre-scaled alpha) is all the fragment shader
    /// needs, so the bind group has one entry only.
    pub(crate) fn create_flash_pipeline(
        device: &wgpu::Device,
        format: wgpu::TextureFormat,
    ) -> wgpu::RenderPipeline {
        let wgsl_source = include_str!("../../shaders/flash.wgsl");
        let flash_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Flash Shader"),
            source: wgpu::ShaderSource::Wgsl(std::borrow::Cow::Borrowed(wgsl_source)),
        });

        let flash_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("Flash Bind Group Layout"),
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                }],
            });

        let flash_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Flash Pipeline Layout"),
                bind_group_layouts: &[Some(&flash_bind_group_layout)],
                immediate_size: 0,
            });

        device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Flash Pipeline"),
            layout: Some(&flash_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &flash_shader,
                entry_point: Some("vs_main"),
                buffers: &[Some(quad_corner_buffer_layout())],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &flash_shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    // SrcAlpha over the already-rendered frame: the flash
                    // dims toward zero alpha as the Kotlin phase decays.
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
        })
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
            let layout = match bg_pipeline_layout.as_ref() {
                Some(l) => l,
                None => return,
            };
            self.blur_h_pipeline = Some(self.device.create_render_pipeline(
                &wgpu::RenderPipelineDescriptor {
                    label: Some("Background Blur H Pipeline"),
                    layout: Some(layout),
                    vertex: wgpu::VertexState {
                        module: &blur_shader,
                        entry_point: Some("vs_main"),
                        buffers: &[Some(quad_corner_buffer_layout())],
                        compilation_options: wgpu::PipelineCompilationOptions::default(),
                    },
                    fragment: Some(wgpu::FragmentState {
                        module: &blur_shader,
                        entry_point: Some("fs_blur_h"),
                        targets: &[Some(wgpu::ColorTargetState {
                            format,
                            blend: Some(wgpu::BlendState::REPLACE),
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
                },
            ));
            self.blur_v_pipeline = Some(self.device.create_render_pipeline(
                &wgpu::RenderPipelineDescriptor {
                    label: Some("Background Blur V Pipeline"),
                    layout: Some(layout),
                    vertex: wgpu::VertexState {
                        module: &blur_shader,
                        entry_point: Some("vs_main"),
                        buffers: &[Some(quad_corner_buffer_layout())],
                        compilation_options: wgpu::PipelineCompilationOptions::default(),
                    },
                    fragment: Some(wgpu::FragmentState {
                        module: &blur_shader,
                        entry_point: Some("fs_blur_v"),
                        targets: &[Some(wgpu::ColorTargetState {
                            format,
                            // Alpha blend: fs_blur_v outputs
                            // alpha = uniforms.alpha, so the opacity
                            // setting composites the blurred wallpaper
                            // over the cleared bg_color.
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
        let sampler = match self.bg_sampler.as_ref() {
            Some(s) => s,
            None => return,
        };

        if self.bg_uniform_buffer.is_none() {
            self.bg_uniform_buffer = Some(self.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("Background Uniform Buffer"),
                size: std::mem::size_of::<BgUniforms>() as u64,
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }));
        }
        let buf = match self.bg_uniform_buffer.as_ref() {
            Some(b) => b,
            None => return,
        };

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

        let proj =
            crate::render::orthographic_projection(surface_width as f32, surface_height as f32);
        let uniforms = BgUniforms {
            projection: proj,
            // Source image size (not surface size): vs_main computes the
            // cover crop from the image aspect ratio.
            image_size: self.bg_image_size,
            blur_radius: blur,
            alpha,
            texel_size: [texel_x, texel_y],
            surface_size: [surface_width as f32, surface_height as f32],
        };
        self.queue
            .write_buffer(buf, 0, bytemuck::cast_slice(&[uniforms]));

        // Bind group is created once: it binds to buffer/texture/sampler
        // objects, not their contents. The uniform buffer is written above
        // every frame via write_buffer, which doesn't invalidate the bind
        // group. Only recreate when bg_image_view, bg_sampler, or
        // bg_uniform_buffer change (e.g., a new background image is loaded).
        if self.bg_bind_group.is_none() {
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

        // Two-pass blur intermediate: the H pass renders into this texture
        // and the V pass samples it (previously both passes wrote the
        // surface, so the H result was overwritten —). Created
        // lazily and recreated when the surface size changes.
        if self.bg_blur_radius >= 0.5 {
            let needs_texture = match &self.bg_blur_texture {
                Some(t) => t.width() != surface_width || t.height() != surface_height,
                None => true,
            };
            if needs_texture {
                self.bg_blur_texture = Some(self.device.create_texture(&wgpu::TextureDescriptor {
                    label: Some("Background Blur Intermediate"),
                    size: wgpu::Extent3d {
                        width: surface_width.max(1),
                        height: surface_height.max(1),
                        depth_or_array_layers: 1,
                    },
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: wgpu::TextureDimension::D2,
                    format,
                    usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                        | wgpu::TextureUsages::TEXTURE_BINDING,
                    view_formats: &[],
                }));
                self.bg_blur_texture_view = self
                    .bg_blur_texture
                    .as_ref()
                    .map(|t| t.create_view(&wgpu::TextureViewDescriptor::default()));
                self.bg_blur_bind_group = None;
            }
            if self.bg_blur_bind_group.is_none()
                && let (Some(blur_view), Some(blur_sampler)) =
                    (self.bg_blur_texture_view.as_ref(), self.bg_sampler.as_ref())
            {
                self.bg_blur_bind_group =
                    Some(self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                        label: Some("Background Blur V Bind Group"),
                        layout: &layout,
                        entries: &[
                            wgpu::BindGroupEntry {
                                binding: 0,
                                resource: buf.as_entire_binding(),
                            },
                            wgpu::BindGroupEntry {
                                binding: 1,
                                resource: wgpu::BindingResource::TextureView(blur_view),
                            },
                            wgpu::BindGroupEntry {
                                binding: 2,
                                resource: wgpu::BindingResource::Sampler(blur_sampler),
                            },
                        ],
                    }));
            }
        } else {
            self.bg_blur_texture = None;
            self.bg_blur_texture_view = None;
            self.bg_blur_bind_group = None;
        }
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

        let proj =
            crate::render::orthographic_projection(surface_width as f32, surface_height as f32);
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

    /// Lazily create the bell-flash pipeline/uniforms/bind group and push
    /// the current phase-scaled alpha. No-op when the flash phase is zero
    /// (nothing to draw — keeps the flash off the hot frame path).
    pub(crate) fn ensure_flash_pipeline(&mut self) {
        if self.flash_phase <= 0.0 {
            return;
        }
        let format = self
            .surface_config
            .as_ref()
            .map_or(wgpu::TextureFormat::Rgba8Unorm, |c| c.format);
        if self.flash_pipeline.is_none() {
            self.flash_pipeline = Some(Self::create_flash_pipeline(&self.device, format));
        }
        let pipeline = match self.flash_pipeline.as_ref() {
            Some(p) => p,
            None => return,
        };
        if self.flash_uniform_buffer.is_none() {
            self.flash_uniform_buffer = Some(self.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("Flash Uniform Buffer"),
                size: std::mem::size_of::<FlashUniforms>() as u64,
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }));
        }
        let buf = match self.flash_uniform_buffer.as_ref() {
            Some(b) => b,
            None => return,
        };
        // phase ∈ 0..=1 (Kotlin drives the decay); 96 is a 0-255 alpha.
        let alpha = (BELL_FLASH_ALPHA_255 / ALPHA_255_MAX) * self.flash_phase;
        let uniforms = FlashUniforms {
            alpha,
            _padding: [0.0; 3],
        };
        self.queue
            .write_buffer(buf, 0, bytemuck::cast_slice(&[uniforms]));
        if self.flash_bind_group.is_none() {
            self.flash_bind_group =
                Some(self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some("Flash Bind Group"),
                    layout: &pipeline.get_bind_group_layout(0),
                    entries: &[wgpu::BindGroupEntry {
                        binding: 0,
                        resource: buf.as_entire_binding(),
                    }],
                }));
        }
    }
}

// ── Off-screen grid verification path (research-wgpu-example.md §6.1) ─────
// Deliberately free functions: the main `Renderer` keeps zero depth
// attachments (2D terminal rendering needs none), so the infinite-LOD grid
// shader + depth texture live on the off-screen render-verification path
// only.

/// Uniform layout for `shaders/grid.wgsl`. Must match the WGSL `Uniform`
/// struct exactly (see grid.wgsl:8-17); `_padding` keeps the struct 16-byte
/// aligned at 112 bytes as required by uniform buffer layout rules.
///
/// `#[cfg(test)]`: only the crate's off-screen render verification path
/// consumes this (see `procedural_geometry`); the production pipeline never
/// builds a grid pipeline.
#[cfg(test)]
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct GridUniforms {
    pub view_proj: [[f32; 4]; 4],
    pub camera_world_pos: [f32; 4],
    pub grid_size: f32,
    pub grid_min_pixels: f32,
    pub grid_cell_size: f32,
    pub orthographic_scale: f32,
    pub is_orthographic: f32,
    /// WGSL `Uniform` struct rounds to 112 bytes (16-byte align); Rust side
    /// must match exactly: 64 (view_proj) + 16 (camera_world_pos) + 20
    /// (five f32) + 12 padding.
    pub _padding: [f32; 3],
}

#[cfg(test)]
impl GridUniforms {
    pub fn perspective(
        view_proj: [[f32; 4]; 4],
        camera_world_pos: [f32; 3],
        grid_size: f32,
        grid_min_pixels: f32,
        grid_cell_size: f32,
    ) -> Self {
        Self {
            view_proj,
            camera_world_pos: [
                camera_world_pos[0],
                camera_world_pos[1],
                camera_world_pos[2],
                1.0,
            ],
            grid_size,
            grid_min_pixels,
            grid_cell_size,
            orthographic_scale: 1.0,
            is_orthographic: 0.0,
            _padding: [0.0; 3],
        }
    }
}

/// Create the grid render pipeline for a given color target format. Uses a
/// `Depth32Float` depth attachment with `Less` compare and depth write
/// enabled — the pipeline must be paired with a depth view from
/// `crate::render::procedural_geometry::create_depth_texture`.
#[cfg(test)]
pub(crate) fn create_grid_pipeline(
    device: &wgpu::Device,
    format: wgpu::TextureFormat,
) -> (wgpu::RenderPipeline, wgpu::BindGroupLayout) {
    let wgsl_source = include_str!("../../shaders/grid.wgsl");
    let grid_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("Grid Shader"),
        source: wgpu::ShaderSource::Wgsl(std::borrow::Cow::Borrowed(wgsl_source)),
    });

    let grid_bind_group_layout =
        device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Grid Bind Group Layout"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });

    let grid_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("Grid Pipeline Layout"),
        bind_group_layouts: &[Some(&grid_bind_group_layout)],
        immediate_size: 0,
    });

    let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("Grid Pipeline"),
        layout: Some(&grid_pipeline_layout),
        vertex: wgpu::VertexState {
            module: &grid_shader,
            entry_point: Some("vertex_main"),
            buffers: &[],
            compilation_options: wgpu::PipelineCompilationOptions::default(),
        },
        fragment: Some(wgpu::FragmentState {
            module: &grid_shader,
            entry_point: Some("fragment_main"),
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
        depth_stencil: Some(wgpu::DepthStencilState {
            format: wgpu::TextureFormat::Depth32Float,
            depth_write_enabled: Some(true),
            depth_compare: Some(wgpu::CompareFunction::Less),
            stencil: wgpu::StencilState::default(),
            bias: wgpu::DepthBiasState::default(),
        }),
        multisample: wgpu::MultisampleState::default(),
        multiview_mask: None,
        cache: None,
    });

    (pipeline, grid_bind_group_layout)
}
