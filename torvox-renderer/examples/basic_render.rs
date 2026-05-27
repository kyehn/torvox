use std::sync::Arc;
use winit::{
    application::ApplicationHandler,
    event::{ElementState, KeyEvent, WindowEvent},
    event_loop::EventLoop,
    keyboard::{Key, NamedKey},
    window::Window,
};

struct App {
    window: Option<Arc<Window>>,
    gpu: Option<torvox_renderer::gpu::GpuContext>,
    grid: torvox_core::grid::Grid,
    font_pipeline: torvox_renderer::font::FontPipeline,
    atlas_width: u32,
    atlas_height: u32,
}

impl App {
    fn new() -> Self {
        let mut grid = torvox_core::grid::Grid::new(24, 80);
        grid.fill_cells(0, 'H', 0, 5);
        grid.fill_cells(0, 'e', 1, 5);
        grid.fill_cells(0, 'l', 2, 5);
        grid.fill_cells(0, 'l', 3, 5);
        grid.fill_cells(0, 'o', 4, 5);

        let atlas_width = 2048;
        let atlas_height = 2048;
        let mut font_pipeline =
            torvox_renderer::font::FontPipeline::new(atlas_width as i32, atlas_height as i32, 14.0);
        font_pipeline.rasterize_ascii();

        Self {
            window: None,
            gpu: None,
            grid,
            font_pipeline,
            atlas_width,
            atlas_height,
        }
    }

    fn handle_resize(&mut self, size: winit::dpi::PhysicalSize<u32>) {
        let gpu = match self.gpu.as_mut() {
            Some(g) => g,
            None => return,
        };
        let window = match self.window.as_ref() {
            Some(w) => w,
            None => return,
        };
        if let Some(config) = &mut gpu.surface_config {
            config.width = size.width.max(1);
            config.height = size.height.max(1);
            if let Some(surface) = &gpu.surface {
                surface.configure(&gpu.device, config);
            }
            let proj = torvox_renderer::gpu::orthographic_projection(
                config.width as f32,
                config.height as f32,
            );
            let uniforms = torvox_renderer::gpu::GpuUniforms {
                projection: proj,
                cell_size: [8.0, 16.0],
                atlas_size: [self.atlas_width as f32, self.atlas_height as f32],
            };
            if let Some(buf) = &gpu.cell_uniform_buffer {
                gpu.queue
                    .write_buffer(buf, 0, bytemuck::cast_slice(&[uniforms]));
            }
            window.request_redraw();
        }
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &winit::event_loop::ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }

        let attrs = Window::default_attributes()
            .with_title("Torvox Terminal")
            .with_inner_size(winit::dpi::LogicalSize::new(800.0, 600.0));

        let window = Arc::new(event_loop.create_window(attrs).unwrap());
        self.window = Some(window.clone());

        let mut gpu = pollster::block_on(torvox_renderer::gpu::GpuContext::new()).unwrap();
        gpu.create_surface(window).unwrap();
        gpu.create_atlas_texture(self.atlas_width, self.atlas_height);

        let (aw, ah) = self.font_pipeline.atlas_dimensions();
        gpu.update_bind_group(aw as f32, ah as f32);

        let atlas_data = self.font_pipeline.atlas_bitmap().to_vec();
        gpu.upload_atlas(&atlas_data, aw, ah);

        self.gpu = Some(gpu);
    }

    fn window_event(
        &mut self,
        event_loop: &winit::event_loop::ActiveEventLoop,
        _window_id: winit::window::WindowId,
        event: WindowEvent,
    ) {
        match event {
            WindowEvent::CloseRequested => {
                event_loop.exit();
            }
            WindowEvent::KeyboardInput {
                event:
                    KeyEvent {
                        logical_key: Key::Named(NamedKey::Escape),
                        state: ElementState::Pressed,
                        ..
                    },
                ..
            } => {
                event_loop.exit();
            }
            WindowEvent::RedrawRequested => {
                if let (Some(gpu), Some(window)) = (&mut self.gpu, &self.window) {
                    let instances = torvox_renderer::gpu::build_cell_instances(
                        &self.grid,
                        &mut self.font_pipeline,
                        8.0,
                        16.0,
                        self.atlas_width as f32,
                        self.atlas_height as f32,
                    );

                    if let Err(e) = gpu.render_frame(&instances) {
                        eprintln!("Render error: {}", e);
                    }

                    window.request_redraw();
                }
            }
            WindowEvent::Resized(size) => {
                self.handle_resize(size);
            }
            _ => {}
        }
    }
}

fn main() {
    let event_loop = EventLoop::new().unwrap();
    let mut app = App::new();
    event_loop.run_app(&mut app).unwrap();
}
