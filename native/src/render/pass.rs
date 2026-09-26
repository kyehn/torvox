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
    /// Present one background-colored frame immediately (启动黑屏防护、
    /// 渲染稳定性 spec §4)：首个内容帧要等 shell 输出 + 冷启动
    /// （SwiftShader 上实测数百毫秒），期间交换链一帧未提交，屏幕保持
    /// 空黑；静默 shell 时甚至永远不会来内容帧。由 attach_surface 在
    /// 渲染线程启动前调用（无竞争），用带超时的 acquire worker 取回
    /// 当前纹理、清为背景色并提交，让表面立即可见。
    pub fn warmup(&self) {
        let surface = match self.surface.as_ref() {
            Some(s) => s,
            None => return,
        };
        let Some(config) = self.surface_config.as_ref() else {
            return;
        };
        let Some(output) = self.acquire_texture(surface, config.width, config.height) else {
            return;
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
                        load: wgpu::LoadOp::Clear(self.background),
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
        _config_width: u32,
        _config_height: u32,
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

    /// Decision for the partial (dirty-band) render path. Pure function —
    /// table-driven unit tested. Partial is only valid when the frame's
    /// ONLY changes are the flagged bands' cell instances; any full-screen
    /// overlay (kitty graphics) or an invalidated accumulator forces a full
    /// redraw.
    fn should_render_partial(
        accumulator_ready: bool,
        frame_invalidated: bool,
        band_count: usize,
        kgp_present: bool,
    ) -> bool {
        accumulator_ready && !frame_invalidated && band_count > 0 && !kgp_present
    }

    /// 滚动一致性门控：视口像素偏移非零，或自上一呈现帧发生变化时为 true。
    /// 纹理拷贝无法亚像素平移累加器，这两种情况下脏带部分路径会把旧像素
    /// 与平移后的新几何混叠（滚动残留/撕裂），必须强制全量重绘。
    fn is_scroll_offset_active(
        viewport_scroll_px: f32,
        last_drawn_viewport_scroll_px: f32,
    ) -> bool {
        viewport_scroll_px != 0.0
            || (viewport_scroll_px - last_drawn_viewport_scroll_px).abs() > f32::EPSILON
    }
    /// 部分/全量合成门控：纯函数——脏带部分路径候选与滚动一致性门控的合成。
    /// 滚动激活时一律全量（判定见 is_scroll_offset_active），偏移归零且稳定后恢复部分渲染。
    fn should_render_partial_frame(
        accumulator_ready: bool,
        frame_invalidated: bool,
        band_count: usize,
        kitty_graphics_present: bool,
        scroll_active: bool,
    ) -> bool {
        Self::should_render_partial(
            accumulator_ready,
            frame_invalidated,
            band_count,
            kitty_graphics_present,
        ) && !scroll_active
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
    /// cell shader paints with `background` verbatim, covering the band's full
    /// pixel rect (all grid columns × the band's row span).
    fn band_clear_instances(
        &self,
        dirty_bands: &[crate::render::cell_builder::DirtyBand],
        cell_h_px: f32,
        config_width: u32,
        config_height: u32,
    ) -> Vec<crate::render::CellInstance> {
        if cell_h_px <= 0.0 || config_width == 0 || config_height == 0 {
            return Vec::new();
        }
        let background = [
            self.background.r as f32,
            self.background.g as f32,
            self.background.b as f32,
            self.background.a as f32,
        ];
        let mut instances = Vec::with_capacity(dirty_bands.len());
        for band in dirty_bands {
            let y0 = (band.start_row as f32 * cell_h_px).floor().max(0.0) as u32;
            let y1 = ((band.end_row_exclusive as f32 * cell_h_px).ceil() as u32).min(config_height);
            if y1 > y0 {
                instances.push(crate::render::CellInstance {
                    quad_origin: [0.0, y0 as f32],
                    atlas_offset: [0.0; 2],
                    atlas_size: [0.0; 2],
                    foreground: background,
                    background,
                    underline_color: background,
                    quad_size: [config_width as f32, (y1 - y0) as f32],
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
        // 暂停期不呈现且必须报“未呈现”：调用方以 Ok 即推进 last_frame，
        // 会把从未上屏的新帧记为已呈现，后续 Idle 误判“已最新”不再补刷
        //（IME 弹出时输入不可见、隐藏后才出现的主因）。
        if self.render_paused {
            return Err(GpuError::Surface("render paused".to_string()));
        }
        // Surface and config must be available when not paused.
        if self.surface.is_none() || self.surface_config.is_none() {
            return Err(GpuError::Surface("no surface configured".to_string()));
        }

        let mut frame_ctx = self
            .begin_frame()
            .ok_or_else(|| GpuError::Surface("begin_frame failed".to_string()))?;

        let config_width = frame_ctx.config_width;
        let config_height = frame_ctx.config_height;
        // Owned clone so it outlives &mut self calls below.
        let swapchain_view = frame_ctx.view.clone();
        let encoder = &mut frame_ctx.encoder;

        // ── Target selection (must run before `pipeline` borrows self) ──
        let format = self.pipeline_format;
        let accumulator_view = if self.swapchain_copy_supported {
            self.ensure_frame_texture(config_width, config_height, format)
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
        let kgp_present = !kgp_instances.is_empty();
        // 滚动一致性（scroll-residual）：viewport_scroll_px 只平移当帧新
        // 绘几何，累加器的旧像素不会移动；脏带部分路径以 Load 叠在新内容
        // 上，旧偏移位置与屏幕边缘条带残留旧像素（用户主诉滚动底部残留/
        // 撕裂）。偏移非零或自上一呈现帧发生变化时强制全量自包含重绘
        // （Clear(背景)+全部实例），偏移归零且稳定后恢复部分渲染。
        let scroll_active = Self::is_scroll_offset_active(
            self.viewport_scroll_px,
            self.last_drawn_viewport_scroll_px,
        );
        let partial = Self::should_render_partial_frame(
            accumulator_view.is_some(),
            self.frame_invalidated,
            dirty_bands.len(),
            kgp_present,
            scroll_active,
        );
        // Band clear instances (partial frames only): empty cells emit no
        // covering quads, so a band redraw over LoadOp::Load left stale
        // pixels in place — most visibly old cursor blocks that never
        // disappeared ( emulator evidence: blocks accumulated at
        // every previous cursor column). The cell shader paints
        // has_glyph=0 quads with background verbatim, so one clear instance
        // per band wipes the band's stale pixels before the redraw — no
        // extra pipeline needed.
        let clear_instances = if partial {
            self.band_clear_instances(dirty_bands, plan.cell_h_px, config_width, config_height)
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
            if sh > 0 && sh < config_height as i32 {
                let mut dst_y = 0i32;
                while dst_y + sh <= config_height as i32 {
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
                            width: config_width,
                            height: sh as u32,
                            depth_or_array_layers: 1,
                        },
                    );
                    dst_y += sh;
                }
            }
        }

        // ── Main merged pass: background → cells → KGP ──────
        // Load rules:
        // - partial: always Load (bands composite over previous output)
        // - plain background: Clear(background)
        let load = if partial {
            wgpu::LoadOp::Load
        } else {
            wgpu::LoadOp::Clear(self.background)
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
        render_pass.set_viewport(
            0.0,
            0.0,
            config_width as f32,
            config_height as f32,
            0.0,
            1.0,
        );
        render_pass.set_scissor_rect(0, 0, config_width, config_height);

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

        drop(render_pass);

        // ── Present: one copy accumulator → swapchain ─────────────
        if let Some(acc_texture) = self.frame_texture.as_ref() {
            encoder.copy_texture_to_texture(
                acc_texture.as_image_copy(),
                frame_ctx.texture.texture.as_image_copy(),
                wgpu::Extent3d {
                    width: config_width.min(frame_ctx.texture.texture.width()),
                    height: config_height.min(frame_ctx.texture.texture.height()),
                    depth_or_array_layers: 1,
                },
            );
        }
        // A completed frame leaves the accumulator coherent.
        if accumulator_view.is_some() {
            self.frame_invalidated = false;
        }
        // 本帧已按当前偏移呈现：作为下一帧滚动变化判定基准。
        self.last_drawn_viewport_scroll_px = self.viewport_scroll_px;

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
    // 渲染线程入口：参数由调用帧装配固定，成组改结构体只增间接无收益。
    pub fn render_cell_data(
        &mut self,
        cell_data: &[crate::terminal::ghostty_terminal::CellData],
        rows: u32,
        cols: u32,
        cursor: crate::render::CellCursor,
        font_pipeline: &mut crate::render::font::FontPipeline,
        atlas_width: f32,
        atlas_height: f32,
        search_highlights: &[crate::render::cell_builder::SearchHighlight],
        dirty_rows: Option<&[bool]>,
        scroll_up_rows: Option<u32>,
        kgp_instances: &[crate::render::KittyGraphicsInstance],
    ) -> Result<(), GpuError> {
        // Grid cell dimensions from the attached surface: quads must cover
        // the full grid (surface_width/cols x surface_height/rows), not the font
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
                        search_highlights,
                    },
                    font_pipeline,
                    &mut self.cpu_instances,
                )
            }
        };

        // 字形首帧完整性：实例构建（build_instances_cached/…）期间新光栅化
        // 的字形只写入了 CPU 侧 atlas_bitmap 并登记 dirty_rect，尚未到达
        // GPU 纹理。render_inner 的上传发生在实例构建之前（下一帧才轮到
        // 本帧的脏区），若此处不补传，本帧绘制将按空纹理采样，而 Idle
        // 门控又不会为重绘触发额外帧——斜体/新字形首帧缺失直到下次输出。
        // 必须在 encoder submit 之前调用：write_texture 以调用顺序入队，
        // 先于本帧绘制命令执行。
        if let Some(rect) = font_pipeline.take_dirty_rect() {
            let (atlas_w, atlas_h) = font_pipeline.atlas_dimensions();
            self.upload_atlas(font_pipeline.atlas_bitmap(), atlas_w, atlas_h, Some(rect));
        }
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
        // 代际必须一致：atlas 重建/驱逐搬迁 UV 后实例已全量重建（新 UV），
        // 若 bands 仍稀疏，partial 路径只画 bands，干净行残留 stale UV
        //（`nix --help` 斜体/新字形部分不可见、滑动后部分出现）。
        // 与增量发射的代际门控（cell_builder 428 行）同条件。
        let bands = self.cell_cache.as_ref().and_then(|cache| {
            let mask = dirty_rows?;
            if !cache.is_compatible(rows, cols) {
                return None;
            }
            if cache.built_atlas_generation() != font_pipeline.atlas_generation() {
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
        let result = self.render_frame_with_plan(&cpu_instances, kgp_instances, &plan);
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

        self.ensure_kgp_pipeline(w, h);

        let tex_size = wgpu::Extent3d {
            width: w,
            height: h,
            depth_or_array_layers: 1,
        };
        // the readback texture must match the pipeline
        // format (the cell/kgp pipelines are created against the surface
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

        {
            let mut rp = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Readback Render Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(self.background),
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

    /// `(ready, invalidated, bands, kgp) -> partial`
    fn partial(args: (bool, bool, usize, bool)) -> bool {
        Renderer::should_render_partial(args.0, args.1, args.2, args.3)
    }

    // ── scroll-coherence gate (fix-scroll-residual-tearing) ──────────────

    /// 滚动一致性门控决策表：偏移非零或相对上一呈现帧变化 → 强制全量。
    #[test]
    fn scroll_offset_active_decision_table() {
        let active = Renderer::is_scroll_offset_active;
        // 偏移为零且与上一帧一致：可走部分路径。
        assert!(!active(0.0, 0.0), "rest state must allow partial bands");
        // 偏移非零（拖动中），无论是否变化都必须全量。
        assert!(active(15.375, 15.375), "held non-zero offset stays active");
        assert!(active(15.375, 0.0), "starting a drag must force full");
        // 偏移变化（含归零），即使目标为零也要全量重绘。
        assert!(active(0.0, 15.375), "settling back to zero must force full");
        assert!(active(1.0, 2.0), "offset change must force full");
        // 任何非零偏移都强制全量（保守：EPSILON 量级抖动也走全量，
        // 只是浪费一次重绘，绝不允许残留）。
        assert!(active(f32::EPSILON, 0.0), "non-zero offset must force full");
        // 亚 EPSILON 的变化视为浮点噪声，不强制全量。
        assert!(
            !active(0.0, f32::EPSILON),
            "sub-epsilon change must not force full"
        );
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
        // Flat fill: no glyph, background carries the clear color.
        assert_eq!(clears[0].atlas_size, [0.0; 2]);
        assert_eq!(clears[0].background[3], 1.0);
        // Degenerate geometry produces no clears.
        assert!(
            renderer
                .band_clear_instances(&bands, 0.0, 1080, 2400)
                .is_empty()
        );
    }

    #[test]
    fn partial_happy_path() {
        assert!(partial((true, false, 1, false)));
    }

    #[test]
    fn partial_requires_accumulator() {
        assert!(!partial((false, false, 1, false)));
    }

    #[test]
    fn partial_requires_invalidated_false() {
        assert!(!partial((true, true, 1, false)));
    }

    #[test]
    fn partial_requires_bands() {
        assert!(!partial((true, false, 0, false)));
    }

    #[test]
    fn partial_rejected_by_overlays() {
        // Any full-screen overlay forces a full redraw.
        assert!(!partial((true, false, 1, true))); // kgp
    }
    // ── scroll-gate composition decision table：partial 候选与 scroll 门控的合成 ──
    // 覆盖生产合成（render_frame_with_plan 经 should_render_partial_frame 求值）：
    // 归零稳定基线接受部分渲染；滚动激活（含惯性滚动中、归零复位瞬间、
    // 持屏拖动非零保持）一律强制全量，不依赖 GPU。
    /// `(accumulator_ready, frame_invalidated, band_count, kitty_graphics_present, scroll_active) -> partial`
    fn planned_frame(decision: (bool, bool, usize, bool, bool)) -> bool {
        let (
            accumulator_ready,
            frame_invalidated,
            band_count,
            kitty_graphics_present,
            scroll_active,
        ) = decision;
        Renderer::should_render_partial_frame(
            accumulator_ready,
            frame_invalidated,
            band_count,
            kitty_graphics_present,
            scroll_active,
        )
    }

    #[test]
    fn scroll_gate_composition_accepts_clean_partial() {
        // 归零且稳定：累加器在、未失效、有脏带、无 overlay、无滚动 → 部分。
        assert!(planned_frame((true, false, 1, false, false)));
    }

    #[test]
    fn scroll_gate_composition_forces_full_on_hold_nonzero() {
        // 滚动激活（偏移非零保持），即使累加器/脏带齐全也必须全量。
        assert!(!planned_frame((true, false, 1, false, true)));
    }

    #[test]
    fn scroll_gate_composition_survives_predicate_flips() {
        // 其余四个谓词位各自独立翻转时，scroll 门控始终压过部分判定；
        // 复位瞬间（scroll_active 仍为真）绝不允许脏带残留路径。
        for accumulator_ready in [true, false] {
            for frame_invalidated in [true, false] {
                for band_count in [0usize, 1usize] {
                    for kitty_graphics_present in [true, false] {
                        assert!(
                            !planned_frame((
                                accumulator_ready,
                                frame_invalidated,
                                band_count,
                                kitty_graphics_present,
                                true
                            )),
                            "scroll_active must always veto partial",
                        );
                    }
                }
            }
        }
    }
}
