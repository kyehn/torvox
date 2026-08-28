//! Render loop — frame submission, synchronization, and error recovery.
//!
//! # Requirements
//! - FR-050 — surface lifecycle: frame submission recovers from surface recreation
use crate::render::GpuError;
use crate::render::Renderer;
use crate::render::context::MIN_ATLAS_BUFFER_SIZE;
use crate::render::pipeline::QUAD_VERTEX_COUNT;
use std::sync::OnceLock;
use std::sync::mpsc::SyncSender;

const GPU_POLL_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(2);
const ACQUIRE_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(2);

type AcquireResult = Result<wgpu::CurrentSurfaceTexture, Box<dyn std::any::Any + Send>>;

struct AcquireRequest {
    surface: std::sync::Arc<wgpu::Surface<'static>>,
    response: std::sync::mpsc::SyncSender<AcquireResult>,
}

fn spawn_acquire_worker() -> SyncSender<AcquireRequest> {
    let (tx, rx) = std::sync::mpsc::sync_channel::<AcquireRequest>(1);
    std::thread::Builder::new()
        .name("gpu-acquire".into())
        .spawn(move || {
            for request in rx {
                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    request.surface.get_current_texture()
                }));
                let _ = request.response.send(result);
            }
        })
        .unwrap_or_else(|e| {
            panic!(
                "FATAL: cannot spawn gpu-acquire worker thread: {e}. \
                 This is required for safe surface texture acquisition. \
                 Check RLIMIT_NPROC / RLIMIT_THREAD."
            )
        });
    tx
}

fn acquire_worker_tx() -> &'static SyncSender<AcquireRequest> {
    static WORKER_TX: OnceLock<SyncSender<AcquireRequest>> = OnceLock::new();
    WORKER_TX.get_or_init(spawn_acquire_worker)
}

impl Renderer {
    pub fn warmup(&self) {
        let surface = match self.surface.as_ref() {
            Some(s) => s,
            None => return,
        };

        let output = match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            surface.get_current_texture()
        })) {
            Ok(
                wgpu::CurrentSurfaceTexture::Success(tex)
                | wgpu::CurrentSurfaceTexture::Suboptimal(tex),
            ) => tex,
            Ok(_) => return,
            Err(_) => {
                log::warn!("warmup: get_current_texture panicked (SwiftShader compat)");
                return;
            }
        };
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Warmup Encoder"),
            });
        let view = output
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        {
            let _render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Warmup Pass"),
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
        }
        self.queue.submit(std::iter::once(encoder.finish()));
        self.queue.present(output);
    }

    pub(crate) fn acquire_texture(
        &self,
        surface: &std::sync::Arc<wgpu::Surface<'static>>,
        _cfg_width: u32,
        _cfg_height: u32,
    ) -> Option<wgpu::SurfaceTexture> {
        // Mali-G57 (Unisoc SoCs) can hang vkAcquireNextImageKHR indefinitely when
        // SURFACE_VIEW_FORMATS is missing. Use a persistent worker thread with a
        // timeout to prevent blocking the render thread forever.
        //
        // Reference (wgpu-in-app app-surface/src/lib.rs:210-235): acquire retry
        // pattern — Outdated/Lost → surface.configure → retry once.  Our worker
        // thread handles Lost and Outdated inline; wgpu-in-app also handles
        // Timeout but we treat a hung acquire as permanent (Mali-G57-specific),
        // while Outdated is transient (surface resized or recreated by the
        // window system) and recovers via reconfigure — emulator-verified:
        // SwiftShader dequeueBuffer timeouts and SurfaceFlinger resize races
        // surface as Outdated, and without the reconfigure the render thread
        // spins on begin_frame failures forever after switching apps.
        //
        // Reference (zelland WGPU_FIXES.md Fix 1): atlas format must equal
        // surface format; wgpu-in-app notes Android view_formats must be
        // vec![format] (downlevel SURFACE_VIEW_FORMATS not supported).
        // The worker thread
        // is created once (via OnceLock) and reused across all frames, avoiding the
        // ~1ms per-frame overhead of std::thread::spawn on Android.
        let (resp_tx, resp_rx) = std::sync::mpsc::sync_channel::<AcquireResult>(1);
        let request = AcquireRequest {
            surface: std::sync::Arc::clone(surface),
            response: resp_tx,
        };
        if let Err(e) = acquire_worker_tx().try_send(request) {
            // Worker channel full or thread died (panic in catch_unwind).
            // Fall back to inline acquire so the render thread never blocks.
            log::warn!("acquire_texture: worker {e:?}, acquiring inline");
            return match surface.get_current_texture() {
                wgpu::CurrentSurfaceTexture::Success(tex)
                | wgpu::CurrentSurfaceTexture::Suboptimal(tex) => Some(tex),
                wgpu::CurrentSurfaceTexture::Lost | wgpu::CurrentSurfaceTexture::Outdated => {
                    if let Some(config) = &self.surface_config {
                        surface.configure(&self.device, config);
                    }
                    None
                }
                _ => None,
            };
        }

        match resp_rx.recv_timeout(ACQUIRE_TIMEOUT) {
            Ok(Ok(result)) => match result {
                wgpu::CurrentSurfaceTexture::Success(tex)
                | wgpu::CurrentSurfaceTexture::Suboptimal(tex) => Some(tex),
                wgpu::CurrentSurfaceTexture::Lost | wgpu::CurrentSurfaceTexture::Outdated => {
                    if let Some(config) = &self.surface_config {
                        surface.configure(&self.device, config);
                    }
                    None
                }
                _ => None,
            },
            Ok(Err(_)) => {
                log::warn!("acquire_texture: get_current_texture panicked");
                None
            }
            Err(_) => {
                log::warn!(
                    "acquire_texture: get_current_texture timed out after {}ms (slow SurfaceFlinger/SwiftShader; frame skipped, retried next frame)",
                    ACQUIRE_TIMEOUT.as_millis()
                );
                None
            }
        }
    }

    /// Grow-if-needed and upload cell instance data to the instance buffer.
    /// Associated function on the exact fields (not `&mut self`) so callers
    /// can invoke it while holding other field borrows (e.g. the cell
    /// pipeline) for the rest of the frame. Shared by the surface frame and
    /// the readback (screenshot) paths; `label` distinguishes them in debug
    /// tooling.
    fn upload_cell_instances(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        instance_buffer: &mut Option<wgpu::Buffer>,
        instances: &[crate::render::CellInstance],
        label: &str,
    ) {
        if instances.is_empty() {
            return;
        }
        let instance_data = bytemuck::cast_slice(instances);
        let needed_size = instance_data.len() as u64;
        let resize_buffer = instance_buffer
            .as_ref()
            .is_none_or(|buf| buf.size() < needed_size);
        if resize_buffer {
            *instance_buffer = Some(device.create_buffer(&wgpu::BufferDescriptor {
                label: Some(label),
                size: needed_size.max(MIN_ATLAS_BUFFER_SIZE),
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }));
        }
        if let Some(buf) = instance_buffer.as_ref() {
            queue.write_buffer(buf, 0, instance_data);
        }
    }

    /// Grow-if-needed and upload KGP instance data, mirroring
    /// [`Self::upload_cell_instances`].
    fn upload_kgp_instances(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        kgp_instance_buffer: &mut Option<wgpu::Buffer>,
        kgp_instances: &[crate::render::KittyGraphicsInstance],
        label: &str,
    ) {
        if kgp_instances.is_empty() {
            return;
        }
        let kgp_instance_data = bytemuck::cast_slice(kgp_instances);
        let needed_size = kgp_instance_data.len() as u64;
        let resize_buffer = kgp_instance_buffer
            .as_ref()
            .is_none_or(|buf| buf.size() < needed_size);
        if resize_buffer {
            *kgp_instance_buffer = Some(device.create_buffer(&wgpu::BufferDescriptor {
                label: Some(label),
                size: needed_size.max(MIN_ATLAS_BUFFER_SIZE),
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }));
        }
        if let Some(buf) = kgp_instance_buffer.as_ref() {
            queue.write_buffer(buf, 0, kgp_instance_data);
        }
    }

    /// Begin the background render pass: clear with the configured bg color
    /// and draw the fullscreen background quad. Shared by the surface frame
    /// and the readback path.
    fn draw_background_pass(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        view: &wgpu::TextureView,
        width: u32,
        height: u32,
    ) {
        let (Some(bg_pipeline), Some(bg_bind_group)) =
            (self.bg_pipeline.as_ref(), self.bg_bind_group.as_ref())
        else {
            return;
        };
        let mut bg_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Background Render Pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(self.bg_color),
                    store: wgpu::StoreOp::Store,
                },
                depth_slice: None,
            })],
            depth_stencil_attachment: None,
            ..Default::default()
        });
        bg_pass.set_pipeline(bg_pipeline);
        bg_pass.set_bind_group(0, bg_bind_group, &[]);
        bg_pass.set_viewport(0.0, 0.0, width as f32, height as f32, 0.0, 1.0);
        bg_pass.set_scissor_rect(0, 0, width, height);
        bg_pass.set_vertex_buffer(0, self.quad_vertex_buffer.slice(..));
        bg_pass.draw(0..QUAD_VERTEX_COUNT, 0..1);
    }

    /// Decision for the partial (dirty-band) render path. Pure function —
    /// table-driven unit tested. Partial is only valid when the frame's
    /// ONLY changes are the flagged bands' cell instances; any full-screen
    /// overlay (wallpaper, blur, kitty graphics, bell flash) or an
    /// invalidated accumulator forces a full redraw.
    fn should_render_partial(
        accumulator_ready: bool,
        frame_invalidated: bool,
        band_count: usize,
        bg_image_active: bool,
        blur_active: bool,
        kgp_present: bool,
        flash_active: bool,
    ) -> bool {
        accumulator_ready
            && !frame_invalidated
            && band_count > 0
            && !bg_image_active
            && !blur_active
            && !kgp_present
            && !flash_active
    }

    pub fn render_frame(
        &mut self,
        instances: &[crate::render::CellInstance],
        kgp_instances: &[crate::render::KittyGraphicsInstance],
    ) -> Result<(), GpuError> {
        self.render_frame_with_plan(
            instances,
            kgp_instances,
            &crate::render::cell_builder::FramePatch::default(),
        )
    }

    /// Build one flat-fill instance per dirty band: a has_glyph=0 quad the
    /// cell shader paints with `bg_color` verbatim, covering the band's full
    /// pixel rect (all grid columns × the band's row span).
    fn band_clear_instances(
        &self,
        dirty_bands: &[crate::render::cell_builder::DirtyBand],
        cell_h_px: f32,
        surface_width: u32,
        surface_height: u32,
    ) -> Vec<crate::render::CellInstance> {
        if cell_h_px <= 0.0 || surface_width == 0 || surface_height == 0 {
            return Vec::new();
        }
        let bg = [
            self.bg_color.r as f32,
            self.bg_color.g as f32,
            self.bg_color.b as f32,
            self.bg_color.a as f32,
        ];
        let mut instances = Vec::with_capacity(dirty_bands.len());
        for band in dirty_bands {
            let y0 = (band.start_row as f32 * cell_h_px).floor().max(0.0) as u32;
            let y1 =
                ((band.end_row_exclusive as f32 * cell_h_px).ceil() as u32).min(surface_height);
            if y1 > y0 {
                instances.push(crate::render::CellInstance {
                    quad_origin: [0.0, y0 as f32],
                    atlas_offset: [0.0; 2],
                    atlas_size: [0.0; 2],
                    fg_color: bg,
                    bg_color: bg,
                    quad_size: [surface_width as f32, (y1 - y0) as f32],
                    flags: 0.0,
                    bearing: [0.0; 2],
                    glyph_advance_width: 0.0,
                });
            }
        }
        instances
    }

    /// Render one frame (see [`Self::render_frame_with_plan`] for the
    /// merged-pass / dirty-band architecture notes).
    pub fn render_frame_with_plan(
        &mut self,
        instances: &[crate::render::CellInstance],
        kgp_instances: &[crate::render::KittyGraphicsInstance],
        plan: &crate::render::cell_builder::FramePatch,
    ) -> Result<(), GpuError> {
        let dirty_bands = &plan.bands[..];
        if self.render_paused {
            return Ok(());
        }
        // Surface and config must be available when not paused.
        if self.surface.is_none() || self.surface_config.is_none() {
            return Err(GpuError::Surface("no surface configured".to_string()));
        }

        let mut frame_ctx = self
            .begin_frame()
            .ok_or_else(|| GpuError::Surface("begin_frame failed".to_string()))?;

        let cfg_width = frame_ctx.cfg_width;
        let cfg_height = frame_ctx.cfg_height;
        // Owned clone so it outlives &mut self calls below.
        let swapchain_view = frame_ctx.view.clone();
        let encoder = &mut frame_ctx.encoder;

        // ── Target selection (must run before `pipeline` borrows self) ──
        let format = self.pipeline_format;
        let accumulator_view = if self.swapchain_copy_supported {
            self.ensure_frame_texture(cfg_width, cfg_height, format)
        } else {
            None
        };

        let pipeline = self
            .cell_pipeline
            .as_ref()
            .ok_or(GpuError::Surface("No render pipeline".to_string()))?;

        log::trace!(
            "render_frame: {} instances, surface={}, pipeline={}, bind_group={}",
            instances.len(),
            self.surface.is_some(),
            self.cell_pipeline.is_some(),
            self.cell_bind_group.is_some(),
        );

        // ── Overlay / partial-path state (must precede the instance
        // upload: band-clear instances are concatenated into it) ─────
        let bg_image_active = self.bg_image_view.is_some();
        let blur_active = bg_image_active
            && self.bg_blur_radius >= 0.5
            && self.blur_h_pipeline.is_some()
            && self.blur_v_pipeline.is_some()
            && self.bg_blur_texture_view.is_some()
            && self.bg_blur_bind_group.is_some();
        let flash_active = self.flash_phase > 0.0;
        let kgp_present = !kgp_instances.is_empty();
        let partial = Self::should_render_partial(
            accumulator_view.is_some(),
            self.frame_invalidated,
            dirty_bands.len(),
            bg_image_active,
            blur_active,
            kgp_present,
            flash_active,
        );
        // Band clear instances (partial frames only): empty cells emit no
        // covering quads, so a band redraw over LoadOp::Load left stale
        // pixels in place — most visibly old cursor blocks that never
        // disappeared ( emulator evidence: blocks accumulated at
        // every previous cursor column). The cell shader paints
        // has_glyph=0 quads with bg_color verbatim, so one clear instance
        // per band wipes the band's stale pixels before the redraw — no
        // extra pipeline needed.
        let clear_instances = if partial {
            self.band_clear_instances(dirty_bands, plan.cell_h_px, cfg_width, cfg_height)
        } else {
            Vec::new()
        };
        let clear_count = clear_instances.len() as u32;
        let mut upload_buffer = clear_instances;
        if clear_count > 0 {
            upload_buffer.extend_from_slice(instances);
        }

        Self::upload_cell_instances(
            &self.device,
            &self.queue,
            &mut self.instance_buffer,
            if clear_count > 0 {
                &upload_buffer
            } else {
                instances
            },
            "Instance Buffer",
        );
        Self::upload_kgp_instances(
            &self.device,
            &self.queue,
            &mut self.kgp_instance_buffer,
            kgp_instances,
            "KGP Instance Buffer",
        );
        // Passes write into the accumulator when available (its content is
        // what gets presented); otherwise straight into the swapchain.
        let view = match accumulator_view.as_ref() {
            Some(acc) => acc,
            None => &swapchain_view,
        };

        // ── Scroll blit: shift existing accumulator content up ────────
        // A pure vertical scroll only needs unchanged glyph pixels MOVED,
        // not re-shaded. Chunked same-texture copies execute in encoder
        // order, top-first, so each chunk reads source rows still intact
        // below the write cursor. The bottom shift_px band stays stale —
        // it is redrawn by this frame's bands.
        if partial
            && let Some(shift_rows) = plan.scroll_up_rows
            && plan.cell_h_px > 0.0
            && let Some(acc_texture) = self.frame_texture.as_ref()
        {
            let sh = (shift_rows as f32 * plan.cell_h_px).round() as i32;
            if sh > 0 && sh < cfg_height as i32 {
                let mut dst_y = 0i32;
                while dst_y + sh <= cfg_height as i32 {
                    encoder.copy_texture_to_texture(
                        wgpu::TexelCopyTextureInfo {
                            texture: acc_texture,
                            mip_level: 0,
                            origin: wgpu::Origin3d {
                                x: 0,
                                y: (dst_y + sh) as u32,
                                z: 0,
                            },
                            aspect: wgpu::TextureAspect::All,
                        },
                        wgpu::TexelCopyTextureInfo {
                            texture: acc_texture,
                            mip_level: 0,
                            origin: wgpu::Origin3d {
                                x: 0,
                                y: dst_y as u32,
                                z: 0,
                            },
                            aspect: wgpu::TextureAspect::All,
                        },
                        wgpu::Extent3d {
                            width: cfg_width,
                            height: sh as u32,
                            depth_or_array_layers: 1,
                        },
                    );
                    dst_y += sh;
                }
            }
        }

        // Background / blur pass
        if let (
            Some(bg_bind_group),
            Some(blur_h),
            Some(blur_v),
            Some(blur_view),
            Some(blur_v_bind_group),
        ) = (
            self.bg_bind_group.as_ref(),
            self.blur_h_pipeline.as_ref(),
            self.blur_v_pipeline.as_ref(),
            self.bg_blur_texture_view.as_ref(),
            self.bg_blur_bind_group.as_ref(),
        ) && blur_active
        {
            {
                let mut h_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("Blur H Pass"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: blur_view,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Clear(self.bg_color),
                            store: wgpu::StoreOp::Store,
                        },
                        depth_slice: None,
                    })],
                    depth_stencil_attachment: None,
                    ..Default::default()
                });
                h_pass.set_pipeline(blur_h);
                h_pass.set_bind_group(0, bg_bind_group, &[]);
                h_pass.set_viewport(0.0, 0.0, cfg_width as f32, cfg_height as f32, 0.0, 1.0);
                h_pass.set_scissor_rect(0, 0, cfg_width, cfg_height);
                h_pass.set_vertex_buffer(0, self.quad_vertex_buffer.slice(..));
                h_pass.draw(0..QUAD_VERTEX_COUNT, 0..1);
            }
            {
                let mut v_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("Blur V Pass"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Clear(self.bg_color),
                            store: wgpu::StoreOp::Store,
                        },
                        depth_slice: None,
                    })],
                    depth_stencil_attachment: None,
                    ..Default::default()
                });
                v_pass.set_pipeline(blur_v);
                v_pass.set_bind_group(0, blur_v_bind_group, &[]);
                v_pass.set_viewport(0.0, 0.0, cfg_width as f32, cfg_height as f32, 0.0, 1.0);
                v_pass.set_scissor_rect(0, 0, cfg_width, cfg_height);
                v_pass.set_vertex_buffer(0, self.quad_vertex_buffer.slice(..));
                v_pass.draw(0..QUAD_VERTEX_COUNT, 0..1);
            }
        } else if self.bg_pipeline.is_some() && self.bg_bind_group.is_some() {
            self.draw_background_pass(&mut *encoder, view, cfg_width, cfg_height);
        }

        // ── Main merged pass: background → cells → KGP → flash ──────
        // Load rules:
        // - partial: always Load (bands composite over previous output)
        // - blur already filled the target: Load
        // - wallpaper without blur: Load (the bg quad below covers all)
        // - plain background: Clear(bg_color) replaces the old dedicated
        //   background pass entirely
        let load = if partial || blur_active || bg_image_active {
            wgpu::LoadOp::Load
        } else {
            wgpu::LoadOp::Clear(self.bg_color)
        };
        let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Main Render Pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load,
                    store: wgpu::StoreOp::Store,
                },
                depth_slice: None,
            })],
            depth_stencil_attachment: None,
            ..Default::default()
        });
        render_pass.set_viewport(0.0, 0.0, cfg_width as f32, cfg_height as f32, 0.0, 1.0);
        render_pass.set_scissor_rect(0, 0, cfg_width, cfg_height);

        // Background image quad (full frames only).
        if bg_image_active
            && let (Some(bg_pipeline), Some(bg_bind_group)) =
                (self.bg_pipeline.as_ref(), self.bg_bind_group.as_ref())
        {
            render_pass.set_pipeline(bg_pipeline);
            render_pass.set_bind_group(0, bg_bind_group, &[]);
            render_pass.set_vertex_buffer(0, self.quad_vertex_buffer.slice(..));
            render_pass.draw(0..QUAD_VERTEX_COUNT, 0..1);
        }

        // Cells: either just the dirty bands or everything.
        {
            if let Some(bind_group) = &self.cell_bind_group {
                render_pass.set_pipeline(pipeline);
                render_pass.set_bind_group(0, bind_group, &[]);

                if !instances.is_empty() || partial {
                    render_pass.set_vertex_buffer(0, self.quad_vertex_buffer.slice(..));
                    if let Some(ref instance_buffer) = self.instance_buffer {
                        render_pass.set_vertex_buffer(1, instance_buffer.slice(..));
                        if partial {
                            if clear_count > 0 {
                                render_pass.draw(0..QUAD_VERTEX_COUNT, 0..clear_count);
                            }
                            for band in dirty_bands {
                                let start = band.instance_start as u32 + clear_count;
                                let count = (band.instance_end - band.instance_start) as u32;
                                if count > 0 {
                                    render_pass.draw(0..QUAD_VERTEX_COUNT, start..start + count);
                                }
                            }
                        } else {
                            render_pass.draw(0..QUAD_VERTEX_COUNT, 0..instances.len() as u32);
                        }
                    }
                }
            }
        }

        // Kitty graphics (full frames only — overlays arbitrary regions).
        if kgp_present
            && let (Some(kgp_pipeline), Some(kgp_bind_group)) =
                (&self.kgp_pipeline, &self.kgp_bind_group)
        {
            render_pass.set_pipeline(kgp_pipeline);
            render_pass.set_bind_group(0, kgp_bind_group, &[]);
            render_pass.set_vertex_buffer(0, self.quad_vertex_buffer.slice(..));
            if let Some(ref ib) = self.kgp_instance_buffer {
                render_pass.set_vertex_buffer(1, ib.slice(..));
            }
            render_pass.draw(0..QUAD_VERTEX_COUNT, 0..kgp_instances.len() as u32);
        }

        // Bell-flash overlay (full frames only — covers the screen).
        if flash_active {
            self.ensure_flash_pipeline();
            if let (Some(flash_pipeline), Some(flash_bind_group)) =
                (&self.flash_pipeline, &self.flash_bind_group)
            {
                render_pass.set_pipeline(flash_pipeline);
                render_pass.set_bind_group(0, flash_bind_group, &[]);
                render_pass.set_vertex_buffer(0, self.quad_vertex_buffer.slice(..));
                render_pass.draw(0..QUAD_VERTEX_COUNT, 0..1);
            }
        }
        drop(render_pass);

        // ── Present: one copy accumulator → swapchain ─────────────
        if let Some(acc_texture) = self.frame_texture.as_ref() {
            encoder.copy_texture_to_texture(
                acc_texture.as_image_copy(),
                frame_ctx.texture.texture.as_image_copy(),
                wgpu::Extent3d {
                    width: cfg_width.min(frame_ctx.texture.texture.width()),
                    height: cfg_height.min(frame_ctx.texture.texture.height()),
                    depth_or_array_layers: 1,
                },
            );
        }
        // A completed frame leaves the accumulator coherent.
        if accumulator_view.is_some() {
            self.frame_invalidated = false;
        }

        // Submit + present
        let encoder = frame_ctx.encoder;
        let texture = frame_ctx.texture;
        self.queue.submit(std::iter::once(encoder.finish()));
        self.queue.present(texture);

        log::debug!(
            "render_frame: presented {} instances (partial={partial}, bands={})",
            instances.len(),
            dirty_bands.len(),
        );

        Ok(())
    }

    /// Render a frame from `Vec<CellData>` (new thread-split data path).
    ///
    /// Converts CellData to CellInstance (atlas lookup + positioning), then
    /// submits to GPU. This is the entry point for the render thread.
    ///
    /// `atlas_width`/`atlas_height` come from the font pipeline's atlas
    /// texture dimensions (typically passed alongside the CellData).
    ///
    /// When `dirty_rows` is `Some`, only those rows are rebuilt and clean
    /// rows are copied from `self.cell_cache` (FR-013 / NFR-010); `None`
    /// forces a full rebuild (and drops the stale cache).
    #[allow(clippy::too_many_arguments)]
    pub fn render_cell_data(
        &mut self,
        cell_data: &[crate::terminal::ghostty_terminal::CellData],
        rows: u32,
        cols: u32,
        cursor: crate::render::CellCursor,
        font_pipeline: &mut crate::render::font::FontPipeline,
        atlas_width: f32,
        atlas_height: f32,
        selection: Option<crate::render::cell_builder::SelectionRange>,
        search_highlights: &[crate::render::cell_builder::SearchHighlight],
        dirty_rows: Option<&[bool]>,
        scroll_up_rows: Option<u32>,
    ) -> Result<(), GpuError> {
        // Grid cell dimensions from the attached surface: quads must cover
        // the full grid (surface_w/cols x surface_h/rows), not the font
        // cell metrics — otherwise rows show gaps of the clear color.
        // quad geometry uses the FONT cell size (logical cell
        // metrics × raster_scale, i.e. the same physical values the Kotlin
        // side computes as cellWidth/cellHeight), NOT surface/rows. The
        // Kotlin grid derives rows from the CONTENT area (surface minus IME
        // and ModifierBar), so surface/rows would stretch each quad to the
        // full surface height whenever the IME is open (2209/14 = 157.8px
        // apx glyphs — reported as "row spacing way too large" and
        // "content overflows without scrolling"). With font-cell quads the
        // glyph fills the quad regardless of how many rows fit on screen.
        let (font_w, font_h) = font_pipeline.cell_metrics();
        let scale = font_pipeline.get_raster_scale();
        let grid_cell_w = if font_w > 0.0 { font_w * scale } else { 0.0 };
        let grid_cell_h = if font_h > 0.0 { font_h * scale } else { 0.0 };
        // Row-level dirty caching (FR-013 / NFR-010): with a dirty mask,
        // only flagged rows are rebuilt through the font atlas; clean rows
        // are copied from the cross-frame cache. `None` (caller has no
        // baseline, e.g. first frame) forces a full rebuild.
        let converted = match dirty_rows {
            Some(mask) => {
                let cache = self.cell_cache.get_or_insert_with(|| {
                    crate::render::cell_builder::CachedInstances::new(rows, cols)
                });
                let effective_mask: &[bool] = if cache.is_compatible(rows, cols) {
                    mask
                } else {
                    // Cache no longer matches the grid (resize): the new
                    // cache starts EMPTY, so serving "clean" rows from it
                    // would copy 0 instances and drop rows. Force a full
                    // rebuild for this frame regression: a
                    // cols-only change kept the diff path alive but the
                    // rebuilt cache had no data).
                    *cache = crate::render::cell_builder::CachedInstances::new(rows, cols);
                    self.cell_full_mask_cache.resize(rows as usize, true);
                    &self.cell_full_mask_cache
                };
                crate::render::cell_builder::build_instances_cached(
                    cell_data,
                    crate::render::cell_builder::CellInstanceConfig {
                        rows,
                        cols,
                        grid_cell_w,
                        grid_cell_h,
                        cursor,
                        atlas_width,
                        atlas_height,
                        selection,
                        search_highlights,
                    },
                    font_pipeline,
                    effective_mask,
                    cache,
                    &mut self.cpu_instances,
                )
            }
            None => {
                // No baseline: drop any stale cache (grid may have changed
                // out from under it) and rebuild every row.
                self.cell_cache = None;
                crate::render::build_instances_from_cell_data(
                    cell_data,
                    crate::render::cell_builder::CellInstanceConfig {
                        rows,
                        cols,
                        grid_cell_w,
                        grid_cell_h,
                        cursor,
                        atlas_width,
                        atlas_height,
                        selection,
                        search_highlights,
                    },
                    font_pipeline,
                    &mut self.cpu_instances,
                )
            }
        };
        if converted.is_none() {
            return Err(GpuError::Surface("CellData conversion failed".into()));
        }
        // Take the buffer out of self so render_frame can borrow it
        // without aliasing the &mut self call (NLL cannot split these
        // borrows because both flow through the same receiver).
        let cpu_instances = std::mem::take(&mut self.cpu_instances);
        // Resolve the dirty-row mask into contiguous instance-slice bands
        // for the GPU dirty-band path. Only valid when the cache is
        // coherent (incremental build actually happened); otherwise the
        // empty band list forces a full redraw.
        let bands = self.cell_cache.as_ref().and_then(|cache| {
            let mask = dirty_rows?;
            if !cache.is_compatible(rows, cols) {
                return None;
            }
            Some(
                crate::render::cell_builder::compute_dirty_bands(mask)
                    .into_iter()
                    .map(|(start_row, end_row_exclusive)| {
                        let (instance_start, instance_end) =
                            cache.band_slice(start_row, end_row_exclusive);
                        crate::render::cell_builder::DirtyBand {
                            start_row,
                            end_row_exclusive,
                            instance_start,
                            instance_end,
                        }
                    })
                    .collect::<Vec<_>>(),
            )
        });
        // Scroll-blit geometry guard: only safe when the grid exactly fills
        // the target vertically (otherwise shifting would smear margins).
        let scroll_up_rows = scroll_up_rows.filter(|_| {
            self.surface_config
                .as_ref()
                .is_some_and(|c| (rows as f32 * grid_cell_h - c.height as f32).abs() <= 2.0)
        });
        let plan = crate::render::cell_builder::FramePatch {
            bands: bands.unwrap_or_default(),
            scroll_up_rows,
            cell_h_px: grid_cell_h,
        };
        let result = self.render_frame_with_plan(&cpu_instances, &[], &plan);
        self.cpu_instances = cpu_instances;
        result
    }

    pub fn render_to_buffer(
        &mut self,
        instances: &[crate::render::CellInstance],
        kgp_instances: &[crate::render::KittyGraphicsInstance],
    ) -> Result<Vec<u8>, GpuError> {
        let (w, h) = self
            .surface_config
            .as_ref()
            .map_or((0, 0), |c| (c.width, c.height));
        if w == 0 || h == 0 {
            return Err(GpuError::Surface("No surface config".to_string()));
        }

        self.ensure_bg_pipeline(w, h);
        self.ensure_kgp_pipeline(w, h);

        let tex_size = wgpu::Extent3d {
            width: w,
            height: h,
            depth_or_array_layers: 1,
        };
        // the readback texture must match the pipeline
        // format (the cell/bg/kgp pipelines are created against the surface
        // format, which is usually Bgra8Unorm on Android) — a hardcoded
        // Rgba8Unorm triggered a wgpu validation error when used as the
        // render attachment for those pipelines.
        let pipeline_format = self.pipeline_format;
        let needs_new = match &self.readback_texture {
            Some(t) => t.width() != w || t.height() != h || t.format() != pipeline_format,
            None => true,
        };
        if needs_new {
            self.readback_texture = Some(self.device.create_texture(&wgpu::TextureDescriptor {
                label: Some("Readback Texture"),
                size: tex_size,
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: pipeline_format,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
                view_formats: &[],
            }));
        }
        let texture = self
            .readback_texture
            .as_ref()
            .ok_or_else(|| GpuError::Surface("readback_texture creation failed".to_string()))?;
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());

        let bytes_per_row_padded = ((w * 4) + (wgpu::COPY_BYTES_PER_ROW_ALIGNMENT - 1))
            & !(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT - 1);
        let buf_size = (bytes_per_row_padded * h) as u64;
        let needs_buf_new = match &self.readback_buffer {
            Some(b) => b.size() < buf_size,
            None => true,
        };
        if needs_buf_new {
            self.readback_buffer = Some(self.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("Readback Buffer"),
                size: buf_size,
                usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
                mapped_at_creation: false,
            }));
        }

        let pipeline = self
            .cell_pipeline
            .as_ref()
            .ok_or_else(|| GpuError::Surface("No render pipeline".to_string()))?;

        let has_bg = self.bg_bind_group.is_some();

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Readback Encoder"),
            });

        Self::upload_cell_instances(
            &self.device,
            &self.queue,
            &mut self.instance_buffer,
            instances,
            "Instance Buffer (readback)",
        );
        Self::upload_kgp_instances(
            &self.device,
            &self.queue,
            &mut self.kgp_instance_buffer,
            kgp_instances,
            "KGP Instance Buffer (readback)",
        );

        self.draw_background_pass(&mut encoder, &view, w, h);

        if let (Some(kgp_pipeline), Some(kgp_bind_group)) =
            (self.kgp_pipeline.as_ref(), self.kgp_bind_group.as_ref())
            && !kgp_instances.is_empty()
        {
            let mut kgp_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("KGP Render Pass (readback)"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    },
                    depth_slice: None,
                })],
                depth_stencil_attachment: None,
                ..Default::default()
            });
            kgp_pass.set_pipeline(kgp_pipeline);
            kgp_pass.set_bind_group(0, kgp_bind_group, &[]);
            kgp_pass.set_viewport(0.0, 0.0, w as f32, h as f32, 0.0, 1.0);
            kgp_pass.set_scissor_rect(0, 0, w, h);
            kgp_pass.set_vertex_buffer(0, self.quad_vertex_buffer.slice(..));
            if let Some(ref ib) = self.kgp_instance_buffer {
                kgp_pass.set_vertex_buffer(1, ib.slice(..));
            }
            kgp_pass.draw(0..QUAD_VERTEX_COUNT, 0..kgp_instances.len() as u32);
        }

        {
            let mut rp = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Readback Render Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: if has_bg {
                            wgpu::LoadOp::Load
                        } else {
                            wgpu::LoadOp::Clear(self.bg_color)
                        },
                        store: wgpu::StoreOp::Store,
                    },
                    depth_slice: None,
                })],
                depth_stencil_attachment: None,
                ..Default::default()
            });
            let wf = w as f32;
            let hf = h as f32;
            rp.set_pipeline(pipeline);
            rp.set_viewport(0.0, 0.0, wf, hf, 0.0, 1.0);
            rp.set_scissor_rect(0, 0, w, h);
            if let Some(bind_group) = &self.cell_bind_group {
                rp.set_bind_group(0, bind_group, &[]);
                if !instances.is_empty() {
                    rp.set_vertex_buffer(0, self.quad_vertex_buffer.slice(..));
                    if let Some(ref ib) = self.instance_buffer {
                        rp.set_vertex_buffer(1, ib.slice(..));
                    }
                    rp.draw(0..QUAD_VERTEX_COUNT, 0..instances.len() as u32);
                }
            }
        }

        let dst = self
            .readback_buffer
            .as_ref()
            .ok_or_else(|| GpuError::Surface("readback_buffer creation failed".to_string()))?;
        encoder.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: dst,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(bytes_per_row_padded),
                    rows_per_image: Some(h),
                },
            },
            tex_size,
        );

        self.queue.submit(std::iter::once(encoder.finish()));

        if let Err(error) = self.device.poll(wgpu::PollType::Wait {
            submission_index: None,
            timeout: Some(GPU_POLL_TIMEOUT),
        }) {
            log::warn!("render_to_buffer: device poll error: {error}");
        }

        let slice = dst.slice(..);
        // Use a oneshot channel to reliably detect map completion.
        let (map_tx, map_rx) = std::sync::mpsc::channel();
        slice.map_async(wgpu::MapMode::Read, move |r| {
            let _ = map_tx.send(r);
        });
        // Poll repeatedly until the map completes or timeout expires.
        let poll_start = std::time::Instant::now();
        let map_result;
        loop {
            if let Err(error) = self.device.poll(wgpu::PollType::Wait {
                submission_index: None,
                timeout: Some(std::time::Duration::from_millis(10)),
            }) {
                log::warn!("render_to_buffer (map wait): device poll error: {error}");
            }
            match map_rx.try_recv() {
                Ok(result) => {
                    map_result = result;
                    break;
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => {}
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    return Err(GpuError::Readback("map channel disconnected".into()));
                }
            }
            if poll_start.elapsed() > std::time::Duration::from_millis(100) {
                return Err(GpuError::Readback("map_async timed out".into()));
            }
        }
        // Propagate map_async errors (e.g. buffer too large, device lost).
        map_result.map_err(|e| GpuError::Readback(format!("map_async failed: {e:?}")))?;
        let data = slice
            .get_mapped_range()
            .map_err(|e| GpuError::Readback(e.to_string()))?
            .to_vec();
        dst.unmap();

        let pixel_bytes = (w * h * 4) as usize;
        let stride = bytes_per_row_padded as usize;
        let trimmed = if data.len() > pixel_bytes && stride > (w as usize * 4) {
            let mut flat = Vec::with_capacity(pixel_bytes);
            for row in 0..h as usize {
                let row_start = row * stride;
                let row_end = row_start + (w as usize * 4);
                if row_end <= data.len() {
                    flat.extend_from_slice(&data[row_start..row_end]);
                }
            }
            flat
        } else {
            data
        };

        Ok(trimmed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn acquire_worker_is_singleton() {
        let tx1 = acquire_worker_tx();
        let tx2 = acquire_worker_tx();
        assert!(
            std::ptr::eq(tx1, tx2),
            "acquire_worker must return the same sender each call"
        );
    }

    // ── should_render_partial decision table (render-vulkan-performance) ──

    /// `(ready, invalidated, bands, bg, blur, kgp, flash) -> partial`
    fn partial(args: (bool, bool, usize, bool, bool, bool, bool)) -> bool {
        Renderer::should_render_partial(args.0, args.1, args.2, args.3, args.4, args.5, args.6)
    }

    /// Band clear instances: one flat quad per band, covering the band's
    /// full pixel rect (: stale cursor pixels persisted because
    /// empty cells emit no covering quads over LoadOp::Load).
    #[test]
    fn band_clear_instances_cover_band_rect() {
        let renderer = Renderer::new_with_no_surface();
        let bands = vec![
            crate::render::cell_builder::DirtyBand {
                start_row: 0,
                end_row_exclusive: 1,
                instance_start: 0,
                instance_end: 80,
            },
            crate::render::cell_builder::DirtyBand {
                start_row: 3,
                end_row_exclusive: 5,
                instance_start: 240,
                instance_end: 400,
            },
        ];
        let cell_h = 44.0;
        let clears = renderer.band_clear_instances(&bands, cell_h, 1080, 2400);
        assert_eq!(clears.len(), 2, "one clear quad per band");
        assert_eq!(clears[0].quad_origin, [0.0, 0.0]);
        assert_eq!(clears[0].quad_size, [1080.0, cell_h]);
        assert_eq!(clears[1].quad_origin, [0.0, 3.0 * cell_h]);
        assert_eq!(clears[1].quad_size, [1080.0, 2.0 * cell_h]);
        // Flat fill: no glyph, bg carries the clear color.
        assert_eq!(clears[0].atlas_size, [0.0; 2]);
        assert_eq!(clears[0].bg_color[3], 1.0);
        // Degenerate geometry produces no clears.
        assert!(
            renderer
                .band_clear_instances(&bands, 0.0, 1080, 2400)
                .is_empty()
        );
    }

    #[test]
    fn partial_happy_path() {
        assert!(partial((true, false, 1, false, false, false, false)));
    }

    #[test]
    fn partial_requires_accumulator() {
        assert!(!partial((false, false, 1, false, false, false, false)));
    }

    #[test]
    fn partial_requires_invalidated_false() {
        assert!(!partial((true, true, 1, false, false, false, false)));
    }

    #[test]
    fn partial_requires_bands() {
        assert!(!partial((true, false, 0, false, false, false, false)));
    }

    #[test]
    fn partial_rejected_by_overlays() {
        // Any full-screen overlay forces a full redraw.
        assert!(!partial((true, false, 1, true, false, false, false))); // bg image
        assert!(!partial((true, false, 1, false, true, false, false))); // blur
        assert!(!partial((true, false, 1, false, false, true, false))); // kgp
        assert!(!partial((true, false, 1, false, false, false, true))); // flash
    }
}
