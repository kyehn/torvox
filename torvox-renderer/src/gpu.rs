use thiserror::Error;
use wgpu::util::DeviceExt;

#[derive(Debug, Error)]
pub enum GpuError {
    #[error("wgpu request adapter failed")]
    NoAdapter,
    #[error("wgpu request device failed: {0}")]
    DeviceRequest(String),
    #[error("surface creation failed: {0}")]
    Surface(String),
    #[error("shader compilation failed: {0}")]
    Shader(String),
    #[error("buffer creation failed: {0}")]
    Buffer(String),
}

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct CellInstance {
    pub cell_pos: [f32; 2],
    pub atlas_offset: [f32; 2],
    pub atlas_size: [f32; 2],
    pub fg_color: [f32; 4],
    pub bg_color: [f32; 4],
    pub flags: f32,
    pub _pad: [f32; 3],
}

impl CellInstance {
    pub const ATTRIBS: [wgpu::VertexAttribute; 6] = wgpu::vertex_attr_array![
        1 => Float32x2,
        2 => Float32x2,
        3 => Float32x2,
        4 => Float32x4,
        5 => Float32x4,
        6 => Float32,
    ];

    pub fn buffer_layout() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<CellInstance>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: &Self::ATTRIBS,
        }
    }
}

const QUAD_CORNERS: &[[f32; 2]; 6] = &[
    [-1.0, -1.0],
    [1.0, -1.0],
    [-1.0, 1.0],
    [-1.0, 1.0],
    [1.0, -1.0],
    [1.0, 1.0],
];

fn quad_corner_buffer_layout() -> wgpu::VertexBufferLayout<'static> {
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
pub struct CursorInstance {
    pub cursor_pos: [f32; 2],
    pub cursor_size: [f32; 2],
    pub color: [f32; 4],
}

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct GpuUniforms {
    pub projection: [[f32; 4]; 4],
    pub cell_size: [f32; 2],
    pub atlas_size: [f32; 2],
}

pub struct GpuContext {
    pub instance: wgpu::Instance,
    pub adapter: wgpu::Adapter,
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pub surface: Option<wgpu::Surface<'static>>,
    pub surface_config: Option<wgpu::SurfaceConfiguration>,
    pub cell_pipeline: wgpu::RenderPipeline,
    pub quad_vertex_buffer: wgpu::Buffer,
    pub cell_bind_group: Option<wgpu::BindGroup>,
    pub cell_uniform_buffer: Option<wgpu::Buffer>,
    pub instance_buffer: Option<wgpu::Buffer>,
    pub atlas_texture: Option<wgpu::Texture>,
    pub atlas_view: Option<wgpu::TextureView>,
    pub atlas_sampler: Option<wgpu::Sampler>,
}

impl GpuContext {
    async fn init_instance_adapter_device()
    -> Result<(wgpu::Instance, wgpu::Adapter, wgpu::Device, wgpu::Queue), GpuError> {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::VULKAN,
            flags: wgpu::InstanceFlags::default(),
            memory_budget_thresholds: wgpu::MemoryBudgetThresholds::default(),
            backend_options: wgpu::BackendOptions::default(),
            display: None,
        });

        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: None,
                force_fallback_adapter: false,
            })
            .await
            .map_err(|_| GpuError::NoAdapter)?;

        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: Some("Torvox Device"),
                required_features: wgpu::Features::empty(),
                required_limits: wgpu::Limits::default(),
                ..Default::default()
            })
            .await
            .map_err(|e| GpuError::DeviceRequest(e.to_string()))?;

        Ok((instance, adapter, device, queue))
    }

    fn create_cell_pipeline(device: &wgpu::Device) -> wgpu::RenderPipeline {
        let cell_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Cell Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("../shaders/cell.wgsl").into()),
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
                buffers: &[quad_corner_buffer_layout(), CellInstance::buffer_layout()],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &cell_shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: wgpu::TextureFormat::Bgra8UnormSrgb,
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
        })
    }

    pub async fn new() -> Result<Self, GpuError> {
        let (instance, adapter, device, queue) = Self::init_instance_adapter_device().await?;
        let cell_pipeline = Self::create_cell_pipeline(&device);
        let quad_vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Quad Vertex Buffer"),
            contents: bytemuck::cast_slice(QUAD_CORNERS),
            usage: wgpu::BufferUsages::VERTEX,
        });

        Ok(Self {
            instance,
            adapter,
            device,
            queue,
            surface: None,
            surface_config: None,
            cell_pipeline,
            quad_vertex_buffer,
            cell_bind_group: None,
            cell_uniform_buffer: None,
            instance_buffer: None,
            atlas_texture: None,
            atlas_view: None,
            atlas_sampler: None,
        })
    }

    pub fn new_with_no_surface() -> Self {
        let (instance, adapter, device, queue) =
            pollster::block_on(Self::init_instance_adapter_device())
                .expect("no GPU adapter/device found");
        let cell_pipeline = Self::create_cell_pipeline(&device);
        let quad_vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Quad Vertex Buffer"),
            contents: bytemuck::cast_slice(QUAD_CORNERS),
            usage: wgpu::BufferUsages::VERTEX,
        });

        Self {
            instance,
            adapter,
            device,
            queue,
            surface: None,
            surface_config: None,
            cell_pipeline,
            quad_vertex_buffer,
            cell_bind_group: None,
            cell_uniform_buffer: None,
            instance_buffer: None,
            atlas_texture: None,
            atlas_view: None,
            atlas_sampler: None,
        }
    }

    pub fn set_surface_from_native_window(
        &mut self,
        window_ptr: *mut std::ffi::c_void,
    ) -> Result<(), GpuError> {
        use raw_window_handle::{AndroidDisplayHandle, AndroidNdkWindowHandle};

        let non_null = std::ptr::NonNull::new(window_ptr)
            .ok_or_else(|| GpuError::Surface("null window pointer".to_string()))?;

        let android_handle = AndroidNdkWindowHandle::new(non_null);
        let display_handle = AndroidDisplayHandle::new();

        let raw_win_handle = raw_window_handle::RawWindowHandle::AndroidNdk(android_handle);
        let raw_display_handle = raw_window_handle::RawDisplayHandle::Android(display_handle);

        // SAFETY: wgpu requires the window handle to remain valid for the lifetime
        // of the surface. The caller (AndroidSurface) ensures the ANativeWindow
        // outlives GpuContext. The raw handles are constructed from a verified
        // non-null pointer and are only used during this call to create the surface.
        let surface = unsafe {
            self.instance
                .create_surface_unsafe(wgpu::SurfaceTargetUnsafe::RawHandle {
                    raw_window_handle: raw_win_handle,
                    raw_display_handle: Some(raw_display_handle),
                })
        }
        .map_err(|e| GpuError::Surface(e.to_string()))?;

        let caps = surface.get_capabilities(&self.adapter);
        let format = caps
            .formats
            .first()
            .copied()
            .unwrap_or(wgpu::TextureFormat::Bgra8UnormSrgb);

        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            width: 1080,
            height: 1920,
            present_mode: wgpu::PresentMode::AutoVsync,
            alpha_mode: caps
                .alpha_modes
                .first()
                .copied()
                .unwrap_or(wgpu::CompositeAlphaMode::Auto),
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };

        surface.configure(&self.device, &config);

        self.surface = Some(surface);
        self.surface_config = Some(config);

        Ok(())
    }

    #[cfg(feature = "winit")]
    pub fn create_surface(
        &mut self,
        window: std::sync::Arc<winit::window::Window>,
    ) -> Result<(), GpuError> {
        let size = window.inner_size();
        let surface = self
            .instance
            .create_surface(window)
            .map_err(|e| GpuError::Surface(e.to_string()))?;

        let caps = surface.get_capabilities(&self.adapter);
        let format = caps
            .formats
            .first()
            .copied()
            .unwrap_or(wgpu::TextureFormat::Bgra8UnormSrgb);

        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            width: size.width.max(1),
            height: size.height.max(1),
            present_mode: wgpu::PresentMode::AutoVsync,
            alpha_mode: caps
                .alpha_modes
                .first()
                .copied()
                .unwrap_or(wgpu::CompositeAlphaMode::Auto),
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };

        surface.configure(&self.device, &config);

        self.surface = Some(surface);
        self.surface_config = Some(config);

        Ok(())
    }

    /// Create the GPU atlas texture.
    ///
    /// Format: `Rgba8UnormSrgb` — RGBA byte layout matches font.rs CPU bitmap output.
    /// The "sRGB" suffix means the GPU applies sRGB gamma correction during sampling,
    /// which is correct for font rendering (rasterizer produces linear alpha, GPU applies gamma).
    pub fn create_atlas_texture(&mut self, width: u32, height: u32) {
        let texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Atlas Texture"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });

        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let sampler = self.device.create_sampler(&wgpu::SamplerDescriptor {
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });

        self.atlas_texture = Some(texture);
        self.atlas_view = Some(view);
        self.atlas_sampler = Some(sampler);
    }

    pub fn upload_atlas(&self, data: &[u8], width: u32, height: u32) {
        if let Some(texture) = &self.atlas_texture {
            self.queue.write_texture(
                wgpu::TexelCopyTextureInfo {
                    texture,
                    mip_level: 0,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                data,
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(4 * width),
                    rows_per_image: Some(height),
                },
                wgpu::Extent3d {
                    width,
                    height,
                    depth_or_array_layers: 1,
                },
            );
        }
    }

    pub fn update_bind_group(&mut self, atlas_width: f32, atlas_height: f32) {
        let config = match self.surface_config.as_ref() {
            Some(c) => c,
            None => return,
        };

        if self.cell_uniform_buffer.is_none() {
            self.cell_uniform_buffer = Some(self.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("Cell Uniform Buffer"),
                size: std::mem::size_of::<GpuUniforms>() as u64,
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }));
        }

        let proj = orthographic_projection(config.width as f32, config.height as f32);

        let uniforms = GpuUniforms {
            projection: proj,
            cell_size: [8.0, 16.0],
            atlas_size: [atlas_width, atlas_height],
        };

        let uniform_buffer = match self.cell_uniform_buffer.as_ref() {
            Some(buf) => buf,
            None => return,
        };
        self.queue
            .write_buffer(uniform_buffer, 0, bytemuck::cast_slice(&[uniforms]));

        let atlas_view = match self.atlas_view.as_ref() {
            Some(v) => v,
            None => return,
        };
        let atlas_sampler = match self.atlas_sampler.as_ref() {
            Some(s) => s,
            None => return,
        };

        self.cell_bind_group = Some(self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Cell Bind Group"),
            layout: &self.cell_pipeline.get_bind_group_layout(0),
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: uniform_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(atlas_view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(atlas_sampler),
                },
            ],
        }));
    }

    pub fn render_frame(&mut self, instances: &[CellInstance]) -> Result<(), GpuError> {
        let surface = self
            .surface
            .as_ref()
            .ok_or(GpuError::Surface("No surface configured".to_string()))?;

        let output = match surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(tex) => tex,
            wgpu::CurrentSurfaceTexture::Suboptimal(tex) => tex,
            wgpu::CurrentSurfaceTexture::Lost => {
                if let Some(config) = &self.surface_config {
                    surface.configure(&self.device, config);
                }
                return Ok(());
            }
            wgpu::CurrentSurfaceTexture::Timeout
            | wgpu::CurrentSurfaceTexture::Occluded
            | wgpu::CurrentSurfaceTexture::Outdated
            | wgpu::CurrentSurfaceTexture::Validation => return Ok(()),
        };

        let view = output
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Frame Encoder"),
            });

        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Cell Render Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.0,
                            g: 0.0,
                            b: 0.0,
                            a: 1.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                    depth_slice: None,
                })],
                depth_stencil_attachment: None,
                ..Default::default()
            });

            render_pass.set_pipeline(&self.cell_pipeline);

            if let Some(bind_group) = &self.cell_bind_group {
                render_pass.set_bind_group(0, bind_group, &[]);

                if !instances.is_empty() {
                    let needed_size = std::mem::size_of_val(instances) as u64;

                    if self
                        .instance_buffer
                        .as_ref()
                        .is_none_or(|b| b.size() < needed_size)
                    {
                        self.instance_buffer = Some(self.device.create_buffer_init(
                            &wgpu::util::BufferInitDescriptor {
                                label: Some("Instance Buffer"),
                                contents: bytemuck::cast_slice(instances),
                                usage: wgpu::BufferUsages::VERTEX,
                            },
                        ));
                    } else if let Some(ref buf) = self.instance_buffer {
                        self.queue
                            .write_buffer(buf, 0, bytemuck::cast_slice(instances));
                    }

                    render_pass.set_vertex_buffer(0, self.quad_vertex_buffer.slice(..));
                    if let Some(ref buf) = self.instance_buffer {
                        render_pass.set_vertex_buffer(1, buf.slice(..));
                    }
                    render_pass.draw(0..6, 0..instances.len() as u32);
                }
            }
        }

        self.queue.submit(std::iter::once(encoder.finish()));
        output.present();

        Ok(())
    }
}

pub fn orthographic_projection(width: f32, height: f32) -> [[f32; 4]; 4] {
    [
        [2.0 / width, 0.0, 0.0, 0.0],
        [0.0, -2.0 / height, 0.0, 0.0],
        [0.0, 0.0, 1.0, 0.0],
        [-1.0, 1.0, 0.0, 1.0],
    ]
}

pub fn build_cell_instances(
    grid: &dyn torvox_core::grid::GridSnapshot,
    font_pipeline: &mut crate::font::FontPipeline,
    _cell_width: f32,
    _cell_height: f32,
    atlas_width: f32,
    atlas_height: f32,
) -> Vec<CellInstance> {
    let rows = grid.rows();
    let cols = grid.cols();
    let mut instances = Vec::with_capacity((rows * cols) as usize);

    for row in 0..rows {
        if let Some(line) = grid.get(row) {
            for col in 0..cols {
                if let Some(cell) = line.get(col) {
                    let fg = cell.fg;
                    let bg = cell.bg;

                    if cell.char == ' ' {
                        instances.push(CellInstance {
                            cell_pos: [col as f32, row as f32],
                            atlas_offset: [0.0, 0.0],
                            atlas_size: [0.0, 0.0],
                            fg_color: [0.0, 0.0, 0.0, 0.0],
                            bg_color: [
                                bg.r as f32 / 255.0,
                                bg.g as f32 / 255.0,
                                bg.b as f32 / 255.0,
                                1.0,
                            ],
                            flags: 0.0,
                            _pad: [0.0; 3],
                        });
                    } else if let Some(info) = font_pipeline.glyph_info(cell.char) {
                        let uv_x = info.atlas_x as f32 / atlas_width;
                        let uv_y = info.atlas_y as f32 / atlas_height;
                        let uv_w = info.width as f32 / atlas_width;
                        let uv_h = info.height as f32 / atlas_height;

                        instances.push(CellInstance {
                            cell_pos: [col as f32, row as f32],
                            atlas_offset: [uv_x, uv_y],
                            atlas_size: [uv_w, uv_h],
                            fg_color: [
                                fg.r as f32 / 255.0,
                                fg.g as f32 / 255.0,
                                fg.b as f32 / 255.0,
                                1.0,
                            ],
                            bg_color: [
                                bg.r as f32 / 255.0,
                                bg.g as f32 / 255.0,
                                bg.b as f32 / 255.0,
                                1.0,
                            ],
                            flags: 0.0,
                            _pad: [0.0; 3],
                        });
                    }
                }
            }
        }
    }

    instances
}

/// Flat grid data for building cell instances without requiring GridSnapshot.
/// Used by Ghostty VT integration where terminal state is managed externally.
pub struct FlatGrid {
    pub rows: u32,
    pub cols: u32,
    pub chars: Vec<char>,
    pub fg: Vec<[f32; 4]>,
    pub bg: Vec<[f32; 4]>,
}

impl FlatGrid {
    pub fn new(rows: u32, cols: u32) -> Self {
        let len = (rows * cols) as usize;
        Self {
            rows,
            cols,
            chars: vec![' '; len],
            fg: vec![[1.0, 1.0, 1.0, 1.0]; len],
            bg: vec![[0.0, 0.0, 0.0, 1.0]; len],
        }
    }

    pub fn set_cell(&mut self, row: u32, col: u32, ch: char, fg: [f32; 4], bg: [f32; 4]) {
        let idx = (row * self.cols + col) as usize;
        if idx < self.chars.len() {
            self.chars[idx] = ch;
            self.fg[idx] = fg;
            self.bg[idx] = bg;
        }
    }

    pub fn cell(&self, row: u32, col: u32) -> Option<(char, [f32; 4], [f32; 4])> {
        if row >= self.rows || col >= self.cols {
            return None;
        }
        let idx = (row * self.cols + col) as usize;
        if idx < self.chars.len() {
            Some((self.chars[idx], self.fg[idx], self.bg[idx]))
        } else {
            None
        }
    }
}

pub fn build_cell_instances_from_flat(
    flat: &FlatGrid,
    font_pipeline: &mut crate::font::FontPipeline,
    atlas_width: f32,
    atlas_height: f32,
) -> Vec<CellInstance> {
    let mut instances = Vec::with_capacity((flat.rows * flat.cols) as usize);

    for row in 0..flat.rows {
        for col in 0..flat.cols {
            if let Some((ch, fg, bg)) = flat.cell(row, col) {
                if ch == ' ' {
                    instances.push(CellInstance {
                        cell_pos: [col as f32, row as f32],
                        atlas_offset: [0.0, 0.0],
                        atlas_size: [0.0, 0.0],
                        fg_color: [0.0, 0.0, 0.0, 0.0],
                        bg_color: bg,
                        flags: 0.0,
                        _pad: [0.0; 3],
                    });
                } else if let Some(info) = font_pipeline.glyph_info(ch) {
                    let uv_x = info.atlas_x as f32 / atlas_width;
                    let uv_y = info.atlas_y as f32 / atlas_height;
                    let uv_w = info.width as f32 / atlas_width;
                    let uv_h = info.height as f32 / atlas_height;

                    instances.push(CellInstance {
                        cell_pos: [col as f32, row as f32],
                        atlas_offset: [uv_x, uv_y],
                        atlas_size: [uv_w, uv_h],
                        fg_color: fg,
                        bg_color: bg,
                        flags: 0.0,
                        _pad: [0.0; 3],
                    });
                }
            }
        }
    }

    instances
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cell_instance_size() {
        assert_eq!(std::mem::size_of::<CellInstance>(), 72);
    }

    #[test]
    fn orthographic_projection_identity() {
        let proj = orthographic_projection(800.0, 600.0);
        assert!((proj[0][0] - 2.0 / 800.0).abs() < f32::EPSILON);
        assert!((proj[1][1] - (-2.0 / 600.0)).abs() < f32::EPSILON);
    }

    #[test]
    fn cell_instance_buffer_layout() {
        let layout = CellInstance::buffer_layout();
        assert_eq!(layout.step_mode, wgpu::VertexStepMode::Instance);
        assert!(layout.array_stride > 0);
    }

    #[test]
    fn flat_grid_new() {
        let grid = FlatGrid::new(10, 20);
        assert_eq!(grid.rows, 10);
        assert_eq!(grid.cols, 20);
        assert_eq!(grid.chars.len(), 200);
        assert!(grid.chars.iter().all(|&c| c == ' '));
    }

    #[test]
    fn flat_grid_set_and_get_cell() {
        let mut grid = FlatGrid::new(5, 5);
        let fg = [1.0, 0.0, 0.0, 1.0];
        let bg = [0.0, 0.0, 0.0, 1.0];
        grid.set_cell(2, 3, 'A', fg, bg);

        let (ch, fg_out, bg_out) = grid.cell(2, 3).unwrap();
        assert_eq!(ch, 'A');
        assert_eq!(fg_out, fg);
        assert_eq!(bg_out, bg);
    }

    #[test]
    fn flat_grid_out_of_bounds() {
        let grid = FlatGrid::new(3, 3);
        assert!(grid.cell(3, 0).is_none());
        assert!(grid.cell(0, 3).is_none());
    }

    #[test]
    fn build_cell_instances_from_flat_basic() {
        let mut grid = FlatGrid::new(1, 4);
        grid.set_cell(0, 0, 'A', [1.0, 1.0, 1.0, 1.0], [0.0, 0.0, 0.0, 1.0]);
        grid.set_cell(0, 1, ' ', [0.0, 0.0, 0.0, 0.0], [0.5, 0.5, 0.5, 1.0]);
        grid.set_cell(0, 2, 'B', [0.0, 1.0, 0.0, 1.0], [0.2, 0.2, 0.2, 1.0]);
        grid.set_cell(0, 3, 'C', [1.0, 0.0, 1.0, 1.0], [0.3, 0.3, 0.3, 1.0]);

        let mut font_pipeline = crate::font::FontPipeline::new(2048, 2048, 14.0);
        font_pipeline.rasterize_ascii();

        let instances = build_cell_instances_from_flat(&grid, &mut font_pipeline, 2048.0, 2048.0);
        // All 4 cells get instances (space gets bg-only, others get glyph)
        assert_eq!(instances.len(), 4);

        let cell0 = &instances[0];
        assert_eq!(cell0.cell_pos, [0.0, 0.0]);
        assert_eq!(cell0.bg_color, [0.0, 0.0, 0.0, 1.0]);

        let cell1 = &instances[1];
        assert_eq!(cell1.cell_pos, [1.0, 0.0]);
        assert_eq!(cell1.bg_color, [0.5, 0.5, 0.5, 1.0]);
    }
}
