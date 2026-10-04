//! 渲染循环：帧提交、同步与错误恢复。
use crate::render::GpuError;
use crate::render::Renderer;
use crate::render::context::MIN_ATLAS_BUFFER_SIZE;
use crate::render::pipeline::QUAD_VERTEX_COUNT;
use std::sync::OnceLock;
use std::sync::mpsc::SyncSender;

/// 设备轮询超时：仅测试回读脚手架使用（`render_to_buffer` 内），生产零引用。
#[cfg(test)]
const GPU_POLL_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(2);
pub(crate) const ACQUIRE_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(2);
/// warmup 的取纹理期限：它跑在 `attach_surface` 内，即持有全局 `RENDER_STATE`，
/// 而该调用由 UI 线程（`surfaceChanged`）驱动——按渲染帧的 2s 上限会让一次
/// GPU 卡死冻结主线程 2s。warmup 只是「立刻铺一层背景色」的尽力而为，
/// 失败也由随后的渲染帧补上，故用约 6 个 vsync 的短期限。
const WARMUP_ACQUIRE_TIMEOUT: std::time::Duration = std::time::Duration::from_millis(100);
/// 回读 map 轮询步长：每次 poll 等待的分片，避免忙等。
/// 仅测试使用（`render_to_buffer` 是无 surface 的回读测试脚手架，生产零调用）。
#[cfg(test)]
const MAP_POLL_STEP: std::time::Duration = std::time::Duration::from_millis(10);
/// 回读 map 总超时：超时即报 Readback 错误，不无限等待。
/// 仅测试使用（同上）。
#[cfg(test)]
const MAP_READBACK_TIMEOUT: std::time::Duration = std::time::Duration::from_millis(100);

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
        .unwrap_or_else(|spawn_error| {
            panic!(
                "FATAL: cannot spawn gpu-acquire worker thread: {spawn_error}. \
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

/// 单帧取纹理的结局。
///
/// `Lost`/`Outdated` 与「本帧跳过」必须分开：前者是 surface 级信号（连续出现即判
/// 死窗口，见 [`Renderer::note_surface_acquire`]`），后者只是工作线程忙或超时，
/// surface 仍然可用。合并成 `Option` 会让慢机器每帧误伤 surface。
pub(crate) enum AcquireOutcome {
    /// 拿到可呈现纹理。
    Acquired(wgpu::SurfaceTexture),
    /// 本帧跳过：工作线程忙/已死、取纹理超时或呈现缺失，下一帧重试。
    Skipped,
    /// surface 级失败：已原地 `reconfigure`，本帧丢弃。
    SurfaceLost,
}

impl AcquireOutcome {
    pub(crate) fn into_texture(self) -> Option<wgpu::SurfaceTexture> {
        match self {
            Self::Acquired(texture) => Some(texture),
            Self::Skipped | Self::SurfaceLost => None,
        }
    }
}

/// 单颜色附件：整帧写入 `load` 指定的内容后保留。
fn store_attachment(
    view: &wgpu::TextureView,
    load: wgpu::LoadOp<wgpu::Color>,
) -> [Option<wgpu::RenderPassColorAttachment<'_>>; 1] {
    [Some(wgpu::RenderPassColorAttachment {
        view,
        resolve_target: None,
        ops: wgpu::Operations {
            load,
            store: wgpu::StoreOp::Store,
        },
        depth_slice: None,
    })]
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
        let Some(output) = self
            .acquire_texture(surface, config.width, config.height, WARMUP_ACQUIRE_TIMEOUT)
            .into_texture()
        else {
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
                color_attachments: &store_attachment(&view, wgpu::LoadOp::Clear(self.background)),
                depth_stencil_attachment: None,
                ..Default::default()
            });
        }
        self.queue.submit(std::iter::once(encoder.finish()));
        self.queue.present(output);
    }

    /// surface 失效时重配（Lost/Outdated 的统一恢复）。
    fn reconfigure_surface(
        surface: &wgpu::Surface<'static>,
        surface_config: &Option<wgpu::SurfaceConfiguration>,
        device: &wgpu::Device,
    ) {
        if let Some(config) = surface_config {
            surface.configure(device, config);
        }
    }

    pub(crate) fn acquire_texture(
        &self,
        surface: &std::sync::Arc<wgpu::Surface<'static>>,
        _config_width: u32,
        _config_height: u32,
        deadline: std::time::Duration,
    ) -> AcquireOutcome {
        // Mali-G57（联发科/展锐 SoC）在缺 SURFACE_VIEW_FORMATS 时会永久卡在
        // vkAcquireNextImageKHR，故用常驻工作线程 + 超时，绝不让渲染线程无限阻塞。
        // Lost/Outdated 在工作线程内就地处理；卡死视为永久故障（Mali-G57 专有），
        // Outdated 是瞬态（surface 被缩放或重建），reconfigure 后可恢复 —— 模拟器实测：
        // SwiftShader 的 dequeueBuffer 超时与 SurfaceFlinger 缩放竞态都会报 Outdated，
        // 不 reconfigure 的话切回应用后渲染线程会永久空转在 begin_frame 失败上。
        // 图集格式必须等于 surface 格式，且 Android 的 view_formats 只能是
        // vec![format]（不支持 downlevel SURFACE_VIEW_FORMATS）。
        // 工作线程经 OnceLock 只建一次并跨帧复用，避免每进程重复建线程；
        // 每帧仅一次同步通道分配。
        let (response_sender, response_receiver) =
            std::sync::mpsc::sync_channel::<AcquireResult>(1);
        let request = AcquireRequest {
            surface: std::sync::Arc::clone(surface),
            response: response_sender,
        };
        // 通道满 = 上一帧的请求仍卡在工作线程里（Mali-G57 专有卡死），此时**绝不能**
        // 退回渲染线程就地取纹理：那正是本文件存在的理由所要避免的无超时阻塞，
        // 会让渲染线程永久挂死。本帧跳过，下一帧重试。
        if let Err(error) = acquire_worker_tx().try_send(request) {
            log::warn!("acquire_texture: worker busy or dead ({error}); frame skipped");
            return AcquireOutcome::Skipped;
        }

        match response_receiver.recv_timeout(deadline) {
            Ok(Ok(result)) => match result {
                wgpu::CurrentSurfaceTexture::Success(tex)
                | wgpu::CurrentSurfaceTexture::Suboptimal(tex) => AcquireOutcome::Acquired(tex),
                wgpu::CurrentSurfaceTexture::Lost | wgpu::CurrentSurfaceTexture::Outdated => {
                    Self::reconfigure_surface(surface, &self.surface_config, &self.device);
                    AcquireOutcome::SurfaceLost
                }
                _ => AcquireOutcome::Skipped,
            },
            Ok(Err(_)) => {
                log::warn!("acquire_texture: get_current_texture panicked");
                AcquireOutcome::Skipped
            }
            Err(_) => {
                log::warn!(
                    "acquire_texture: get_current_texture timed out after {}ms (slow SurfaceFlinger/SwiftShader; frame skipped, retried next frame)",
                    deadline.as_millis()
                );
                AcquireOutcome::Skipped
            }
        }
    }

    /// 按需扩容并上传单元实例数据。写成对精确字段的关联函数（而非 `&mut self`），
    /// 使调用方在整帧持有其他字段借用（如单元管线）时仍能调用。
    /// 表面帧与截图回读两条路径共用，`label` 用于在调试工具中区分。
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

    /// 按需扩容并上传 KGP 实例数据，同 [`Self::upload_cell_instances`]。
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

    /// 脏带部分渲染的判定（纯函数，表驱动单测）。仅当本帧唯一变化是被标记
    /// 脏带的单元实例时成立；任何全屏叠加（kitty 图像）或累加器失效都强制全量重绘。
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

    /// 每个脏带构造一个纯色实例：has_glyph=0 的四边形，单元着色器直接以
    /// `background` 原样填充，覆盖该带的完整像素矩形（全部列 × 该带的行跨度）。
    fn band_clear_instances(
        &self,
        dirty_bands: &[crate::render::cell_builder::DirtyBand],
        cell_height_px: f32,
        config_width: u32,
        config_height: u32,
    ) -> Vec<crate::render::CellInstance> {
        if cell_height_px <= 0.0 || config_width == 0 || config_height == 0 {
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
            let y0 = (band.start_row as f32 * cell_height_px).floor().max(0.0) as u32;
            let y1 =
                ((band.end_row_exclusive as f32 * cell_height_px).ceil() as u32).min(config_height);
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

    /// 渲染一帧（合并通道/脏带架构说明见 [`Self::render_frame_with_plan`]）。
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
        // 未暂停时 surface 与 config 必须可用。
        if self.surface.is_none() || self.surface_config.is_none() {
            return Err(GpuError::Surface("no surface configured".to_string()));
        }

        let mut frame_ctx = self
            .begin_frame()
            .ok_or_else(|| GpuError::Surface("begin_frame failed".to_string()))?;

        let config_width = frame_ctx.config_width;
        let config_height = frame_ctx.config_height;
        // 克隆持有，使其活得比下方对 &mut self 的调用更久。
        let swapchain_view = frame_ctx.view.clone();
        let encoder = &mut frame_ctx.encoder;

        // ── 选定渲染目标（必须早于 `pipeline` 借用 self） ──
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

        // ── 叠加与部分路径状态（必须早于实例上传：脏带清除实例会拼进上传缓冲） ──
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
        // 脏带清除实例（仅部分帧）：空单元不产生覆盖四边形，故 LoadOp::Load 上的
        // 脏带重绘会留下陈旧像素——最明显的是永不消失的旧光标块（模拟器实证：色块
        // 累积在此前每个光标列上）。单元着色器把 has_glyph=0 四边形直接以背景色
        // 填充，故每带一个清除实例即可在重绘前抹掉陈旧像素，无需额外管线。
        let clear_instances = if partial {
            self.band_clear_instances(
                dirty_bands,
                plan.cell_height_px,
                config_width,
                config_height,
            )
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
        // 有累加器时各通道写入其中（呈现的正是它的内容），否则直接写交换链。
        let view = match accumulator_view.as_ref() {
            Some(acc) => acc,
            None => &swapchain_view,
        };

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
            color_attachments: &store_attachment(view, load),
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

    /// 由 CellData 渲染一帧：先查图集并定位转成 CellInstance，再提交 GPU。
    ///
    /// `dirty_rows` 为 `Some` 时只重建被标记的行，干净行取自 `self.cell_cache`；
    /// `None` 强制全量重建并丢弃陈旧缓存。
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
        kgp_instances: &[crate::render::KittyGraphicsInstance],
    ) -> Result<(), GpuError> {
        // 四边形几何必须用字体单元格尺寸（逻辑单元格度量 × raster_scale，即 Kotlin
        // 侧算出的 cellWidth/cellHeight），不能用 surface/rows：Kotlin 的网格行数来自
        // 内容区（surface 减去输入法与修饰键栏），故 IME 打开时 surface/rows 会把每个
        // 四边形拉高到整个 surface（2209/14 = 157.8px/字形，实测表现为“行距过大”与
        // “内容溢出且无法滚动”）。用字体单元格时无论屏幕能放下多少行，字形都填满四边形。
        let (font_w, font_h) = font_pipeline.cell_metrics();
        let scale = font_pipeline.get_raster_scale();
        let grid_cell_width = if font_w > 0.0 { font_w * scale } else { 0.0 };
        let grid_cell_height = if font_h > 0.0 { font_h * scale } else { 0.0 };
        // 行级脏缓存：给出脏掩码时只重建被标记的行，干净行复制自跨帧缓存；
        // `None`（调用方无基线，如首帧）强制全量重建。
        let converted = match dirty_rows {
            Some(mask) => {
                let cache = self.cell_cache.get_or_insert_with(|| {
                    crate::render::cell_builder::CachedInstances::new(rows, cols)
                });
                let effective_mask: &[bool] = if cache.is_compatible(rows, cols) {
                    mask
                } else {
                    // 缓存已与网格不匹配（缩放）：新缓存是空的，若仍按“干净行”
                    // 取值会复制 0 个实例并丢行。本帧强制全量重建。
                    *cache = crate::render::cell_builder::CachedInstances::new(rows, cols);
                    self.cell_full_mask_cache.resize(rows as usize, true);
                    &self.cell_full_mask_cache
                };
                crate::render::cell_builder::build_instances_cached(
                    cell_data,
                    crate::render::cell_builder::CellInstanceConfig {
                        rows,
                        cols,
                        grid_cell_width,
                        grid_cell_height,
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
                // 无基线：丢弃可能已失效的陈旧缓存并重建所有行。
                self.cell_cache = None;
                crate::render::build_instances_from_cell_data(
                    cell_data,
                    crate::render::cell_builder::CellInstanceConfig {
                        rows,
                        cols,
                        grid_cell_width,
                        grid_cell_height,
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
        // 把缓冲移出 self，使 render_frame 能借用它而不与对 self 的 &mut 调用
        // 别名冲突（NLL 无法拆分同一个接收者的借用）。
        let cpu_instances = std::mem::take(&mut self.cpu_instances);
        // 把行级脏掩码解析成连续的实例切片脏带，供 GPU 脏带路径使用。
        // 仅在缓存
        // 本身一致（确实发生了增量构建）时有效，否则空脏带列表会强制全量重绘。
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
        let plan = crate::render::cell_builder::FramePatch {
            bands: bands.unwrap_or_default(),
            cell_height_px: grid_cell_height,
        };
        let result = self.render_frame_with_plan(&cpu_instances, kgp_instances, &plan);
        self.cpu_instances = cpu_instances;
        result
    }

    /// 无 surface 的像素回读：仅测试脚手架（背景填充/滚动帧像素断言），生产零调用。
    /// `#[cfg(test)]` 使其不进生产二进制；删测试脚手架时随之删除。
    #[cfg(test)]
    pub fn render_to_buffer(
        &mut self,
        instances: &[crate::render::CellInstance],
        kgp_instances: &[crate::render::KittyGraphicsInstance],
    ) -> Result<Vec<u8>, GpuError> {
        let (frame_width, frame_height) = self
            .surface_config
            .as_ref()
            .map_or((0, 0), |surface_config| {
                (surface_config.width, surface_config.height)
            });
        if frame_width == 0 || frame_height == 0 {
            return Err(GpuError::Surface("No surface config".to_string()));
        }

        self.ensure_kgp_pipeline(frame_width, frame_height);

        let tex_size = wgpu::Extent3d {
            width: frame_width,
            height: frame_height,
            depth_or_array_layers: 1,
        };
        // the readback texture must match the pipeline
        // format (the cell/kgp pipelines are created against the surface
        // format, which is usually Bgra8Unorm on Android) — a hardcoded
        // Rgba8Unorm triggered a wgpu validation error when used as the
        // render attachment for those pipelines.
        let pipeline_format = self.pipeline_format;
        let needs_new = match &self.readback_texture {
            Some(texture) => {
                texture.width() != frame_width
                    || texture.height() != frame_height
                    || texture.format() != pipeline_format
            }
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

        let bytes_per_row_padded = ((frame_width * 4) + (wgpu::COPY_BYTES_PER_ROW_ALIGNMENT - 1))
            & !(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT - 1);
        let buf_size = (bytes_per_row_padded * frame_height) as u64;
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
                color_attachments: &store_attachment(&view, wgpu::LoadOp::Clear(self.background)),
                depth_stencil_attachment: None,
                ..Default::default()
            });
            let width_float = frame_width as f32;
            let height_float = frame_height as f32;
            rp.set_pipeline(pipeline);
            rp.set_viewport(0.0, 0.0, width_float, height_float, 0.0, 1.0);
            rp.set_scissor_rect(0, 0, frame_width, frame_height);
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
                    rows_per_image: Some(frame_height),
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
        // 用 oneshot 通道可靠地判定 map 完成。
        let (map_tx, map_rx) = std::sync::mpsc::channel();
        slice.map_async(wgpu::MapMode::Read, move |r| {
            let _ = map_tx.send(r);
        });
        // 反复轮询直到 map 完成或超时。
        let poll_start = std::time::Instant::now();
        let map_result;
        loop {
            if let Err(error) = self.device.poll(wgpu::PollType::Wait {
                submission_index: None,
                timeout: Some(MAP_POLL_STEP),
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
            if poll_start.elapsed() > MAP_READBACK_TIMEOUT {
                // 超时即 unmap：不断开映射直接返回会让该缓冲永久处于 mapped 态，
                // 下次对其 copy 会被 wgpu-core 拒收乃至设备丢失（R28-T2）。
                dst.unmap();
                return Err(GpuError::Readback("map_async timed out".into()));
            }
        }
        // 向上传递 map_async 错误（如缓冲过大、设备丢失）。
        map_result
            .map_err(|map_error| GpuError::Readback(format!("map_async failed: {map_error:?}")))?;
        let data = slice
            .get_mapped_range()
            .map_err(|map_error| GpuError::Readback(map_error.to_string()))?
            .to_vec();
        dst.unmap();

        let pixel_bytes = (frame_width * frame_height * 4) as usize;
        let stride = bytes_per_row_padded as usize;
        let trimmed = if data.len() > pixel_bytes && stride > (frame_width as usize * 4) {
            let mut flat = Vec::with_capacity(pixel_bytes);
            for row in 0..frame_height as usize {
                let row_start = row * stride;
                let row_end = row_start + (frame_width as usize * 4);
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

    // ── should_render_partial 判定表 ──

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

    /// 脏带清除实例：每带一个纯色四边形，覆盖该带完整像素矩形
    /// （空单元在 LoadOp::Load 上不产生覆盖四边形，会残留陈旧光标像素）。
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
        let cell_height = 44.0;
        let clears = renderer.band_clear_instances(&bands, cell_height, 1080, 2400);
        assert_eq!(clears.len(), 2, "one clear quad per band");
        assert_eq!(clears[0].quad_origin, [0.0, 0.0]);
        assert_eq!(clears[0].quad_size, [1080.0, cell_height]);
        assert_eq!(clears[1].quad_origin, [0.0, 3.0 * cell_height]);
        assert_eq!(clears[1].quad_size, [1080.0, 2.0 * cell_height]);
        // 纯色填充：无字形，清除色由背景承担。
        assert_eq!(clears[0].atlas_size, [0.0; 2]);
        assert_eq!(clears[0].background[3], 1.0);
        // 退化几何不产生清除。
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
        // 任何全屏叠加都强制全量重绘。
        assert!(!partial((true, false, 1, true))); // kgp
    }
    // ── 滚动门控合成判定表：partial 候选与 scroll 门控的合成 ──
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
