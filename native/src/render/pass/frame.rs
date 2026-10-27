//! 帧提交：实例上传、脏带合成与呈现。
use super::acquire::ACQUIRE_TIMEOUT;
use super::bands::{ensure_instance_capacity, instance_upload_layout, partial_frame_bands};
use crate::render::GpuError;
use crate::render::Renderer;
use crate::render::context::MIN_VERTEX_BUFFER_SIZE;
use crate::render::pipeline::QUAD_VERTEX_COUNT;
use crate::slow_log::SlowLogThrottle;

/// 渲染分段慢帧诊断的限频闸门。
///
/// 静态而非渲染态字段：渲染态是**全局单例**（`RENDER_STATE`），挂在它上面与静态
/// 等价，却要多改构造路径。此处每类诊断一个闸门，互不吞掉。
static SLOW_RENDER_PHASE_THROTTLE: SlowLogThrottle = SlowLogThrottle::new();
/// 单元构建慢帧诊断的限频闸门（与渲染分段各自限频）。
static SLOW_CELL_BUILD_THROTTLE: SlowLogThrottle = SlowLogThrottle::new();
/// 单颜色附件：整帧写入 `load` 指定的内容后保留。
pub(super) fn store_attachment(
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
            .acquire_texture(surface, config.width, config.height, ACQUIRE_TIMEOUT)
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
    /// 按需扩容并上传 KGP 实例数据，同 [`Self::upload_instances`]。
    pub(super) fn upload_kgp_instances(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        kgp_instance_buffer: &mut Option<wgpu::Buffer>,
        kgp_instances: &[crate::render::KittyGraphicsInstance],
        label: &str,
    ) {
        Self::upload_instances(
            device,
            queue,
            kgp_instance_buffer,
            bytemuck::cast_slice(kgp_instances),
            0,
            label,
        );
    }

    /// 两类实例共用的上传：容量不足时按需重建，否则原地覆写到 `offset_bytes`。
    pub(super) fn upload_instances(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        instance_buffer: &mut Option<wgpu::Buffer>,
        instance_data: &[u8],
        offset_bytes: u64,
        label: &str,
    ) {
        if instance_data.is_empty() {
            return;
        }
        let needed_size = offset_bytes + instance_data.len() as u64;
        let resize_buffer = instance_buffer
            .as_ref()
            .is_none_or(|buf| buf.size() < needed_size);
        if resize_buffer {
            *instance_buffer = Some(device.create_buffer(&wgpu::BufferDescriptor {
                label: Some(label),
                size: needed_size.max(MIN_VERTEX_BUFFER_SIZE),
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }));
        }
        if let Some(buf) = instance_buffer.as_ref() {
            queue.write_buffer(buf, offset_bytes, instance_data);
        }
    }
    /// 渲染一帧（合并通道/脏带架构说明见 [`Self::render_frame_with_plan`]）。
    pub fn render_frame_with_plan(
        &mut self,
        instances: &[crate::render::CellInstance],
        kgp_instances: &[crate::render::KittyGraphicsInstance],
        kgp_overlay_dirty: bool,
        plan: &crate::render::cell_builder::FramePatch,
    ) -> Result<(), GpuError> {
        let dirty_bands = &plan.bands[..];
        // 暂停期不呈现且必须报“未呈现”：调用方以 Ok 即推进 last_frame，
        // 会把从未上屏的新帧记为已呈现，后续 Idle 误判“已最新”不再补刷
        //（IME 弹出时输入不可见、隐藏后才出现的主因）。
        if self.render_paused {
            return Err(GpuError::SurfaceUnavailable("render paused".to_string()));
        }
        // 未暂停时 surface 与 config 必须可用。
        if self.surface.is_none() || self.surface_config.is_none() {
            return Err(GpuError::SurfaceUnavailable(
                "no surface configured".to_string(),
            ));
        }

        let frame_start = std::time::Instant::now();
        let mut frame_ctx = self
            .begin_frame()
            .ok_or_else(|| GpuError::SurfaceUnavailable("begin_frame failed".to_string()))?;
        let begin_done = std::time::Instant::now();

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
            kgp_overlay_dirty,
            scroll_active,
        );
        // 脏带清除实例（仅部分帧）：空单元只在有光标时收缩为光标标记，格内其余
        // 区域不被本帧覆盖，故 LoadOp::Load 上的脏带重绘会留下陈旧像素——最明显的
        // 是永不消失的旧光标块（模拟器实证：色块累积在此前每个光标列上）。单元着色
        // 器把 has_glyph=0 四边形直接以背景色填充，故每带一个清除实例即可在重绘
        // 前抹掉陈旧像素，无需额外管线。
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
        // 清除段与真实例作为同一缓冲的两段写入（免掉拼接中间 Vec：每个部分帧都要
        // 为几枚清除四边形整份复制约 190KB 实例数组）。容量按两段之和一次算足——
        // 先写清除段再为真实例扩容重建缓冲，会把刚落盘的清除段丢掉。
        let (clear_end, draw_end) = instance_upload_layout(clear_count as usize, instances.len());
        ensure_instance_capacity(
            &self.device,
            &mut self.instance_buffer,
            draw_end,
            "Instance Buffer",
        );
        Self::upload_instances(
            &self.device,
            &self.queue,
            &mut self.instance_buffer,
            bytemuck::cast_slice(&clear_instances[..]),
            0,
            "Instance Buffer",
        );
        Self::upload_instances(
            &self.device,
            &self.queue,
            &mut self.instance_buffer,
            bytemuck::cast_slice(instances),
            clear_end,
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

        // ── 合并主通道：背景 → 单元 → KGP ──────
        // Load 规则：
        // - 部分帧：一律 Load（各脏带叠在上一帧输出之上）
        // - 纯背景：Clear(background)
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

        // 单元：只画脏带或全画。
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

        // Kitty 图像：只在**它的像素被本帧擦掉**的那些行上重画。
        //
        // 叠加层是 alpha 混合：同一幅图反复混合到同一片像素上，透明度会逐帧
        // 收敛到不透明（十帧内），半透明图像于是「越用越实」。全量帧整屏重画，
        // 图像像素必被清掉，于是整层重画；部分帧只有与图像相交的脏带被重画，
        // 于是只在那几条带（按像素裁剪）上重画图像，其余像素原样留在累加器里。
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
            if partial {
                for band in dirty_bands {
                    if !Self::kgp_touched_by_band(kgp_instances, band, plan.cell_height_px) {
                        continue;
                    }
                    let (top, bottom) = Self::band_clear_pixel_range(
                        band.start_row,
                        band.end_row_exclusive,
                        plan.cell_height_px,
                        config_height,
                    );
                    if bottom <= top {
                        continue;
                    }
                    render_pass.set_scissor_rect(0, top, config_width, bottom - top);
                    render_pass.draw(0..QUAD_VERTEX_COUNT, 0..kgp_instances.len() as u32);
                }
                render_pass.set_scissor_rect(0, 0, config_width, config_height);
            } else {
                render_pass.draw(0..QUAD_VERTEX_COUNT, 0..kgp_instances.len() as u32);
            }
        }

        drop(render_pass);

        // ── 呈现：一次拷贝（累加器 → 交换链） ─────────────
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
        // 帧已完成，累加器内容保持自洽。
        if accumulator_view.is_some() {
            self.frame_invalidated = false;
        }
        // 本帧已按当前偏移呈现：作为下一帧滚动变化判定基准。
        self.last_drawn_viewport_scroll_px = self.viewport_scroll_px;

        // 提交 + 呈现
        let encode_done = std::time::Instant::now();
        let encoder = frame_ctx.encoder;
        let texture = frame_ctx.texture;
        self.queue.submit(std::iter::once(encoder.finish()));
        let submit_done = std::time::Instant::now();
        self.queue.present(texture);
        let present_done = std::time::Instant::now();

        // 慢帧只记一条，区分 acquire/编码/提交/呈现哪段阻塞；30ms 与 Kotlin 侧
        // SLOW_FRAME 阈值一致，正常帧不产生日志。
        //
        // 限频：持续慢时每帧都超阈值，逐帧记会把 logcat 刷成慢帧洪水
        // （实测软件 Vulkan 上 500ms 窗口内可出数百条），规范要求两次输出间隔
        // 不小于 500ms。本类诊断独占一个闸门，不与单元构建诊断共用计数器。
        let total = present_done.duration_since(frame_start);
        if total >= std::time::Duration::from_millis(30) && SLOW_RENDER_PHASE_THROTTLE.should_log()
        {
            let ms = |d: std::time::Duration| d.as_secs_f64() * 1000.0;
            log::warn!(
                "SLOW_RENDER_PHASE acquire={:.3}ms encode={:.3}ms submit={:.3}ms present={:.3}ms total={:.3}ms partial={partial} instances={}",
                ms(begin_done.duration_since(frame_start)),
                ms(encode_done.duration_since(begin_done)),
                ms(submit_done.duration_since(encode_done)),
                ms(present_done.duration_since(submit_done)),
                ms(total),
                instances.len(),
            );
        }

        // 呈现诊断按 500ms 窗口汇总：逐帧输出会把 logcat 刷成帧洪水（实测 84s 内
        // 691 行、间隔中位 15ms），规范要求两次输出间隔不小于 500ms。窗口内的
        // 呈现帧数随该行一起交出——限频吞掉的是条数，不是次数。
        if let Some(presented_frames) = self.present_log.record() {
            log::debug!(
                "render_frame: presented {presented_frames} frames (last: {} instances, partial={partial}, dirty_bands={})",
                instances.len(),
                dirty_bands.len(),
            );
        }

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
        search_highlights: &[crate::render::cell_builder::SearchHighlight],
        dirty_rows: Option<&[bool]>,
        kgp_instances: &[crate::render::KittyGraphicsInstance],
        kgp_overlay_dirty: bool,
    ) -> Result<(), GpuError> {
        // 图集尺寸由字形管线自身持有（UV 归一化的唯一权威），不再由调用方并传一份。
        let (atlas_px_width, atlas_px_height) = font_pipeline.atlas_dimensions();
        // 四边形几何必须用字体单元格尺寸（逻辑单元格度量 × raster_scale，即 Kotlin
        // 侧算出的 cellWidth/cellHeight），不能用 surface/rows：Kotlin 的网格行数来自
        // 内容区（surface 减去输入法与修饰键栏），故 IME 打开时 surface/rows 会把每个
        // 四边形拉高到整个 surface（2209/14 = 157.8px/字形，实测表现为“行距过大”与
        // “内容溢出且无法滚动”）。用字体单元格时无论屏幕能放下多少行，字形都填满四边形。
        let (font_w, font_h) = font_pipeline.cell_metrics();
        let scale = font_pipeline.get_raster_scale();
        let grid_cell_width = if font_w > 0.0 { font_w * scale } else { 0.0 };
        let grid_cell_height = if font_h > 0.0 { font_h * scale } else { 0.0 };
        // 两条分支的配置完全相同且 `CellInstanceConfig` 是 Copy，构造一次即可，
        // 避免全量/增量两条路径的参数各自演化。
        let config = crate::render::cell_builder::CellInstanceConfig {
            rows,
            cols,
            grid_cell_width,
            grid_cell_height,
            cursor,
            atlas_width: atlas_px_width as f32,
            atlas_height: atlas_px_height as f32,
            search_highlights,
            default_background: [
                self.background.r as f32,
                self.background.g as f32,
                self.background.b as f32,
                self.background.a as f32,
            ],
        };
        // 行级脏缓存：给出脏掩码时只重建被标记的行，干净行复制自跨帧缓存；
        // `None`（调用方无基线，如首帧）强制全量重建。
        let build_start = std::time::Instant::now();
        let (converted, rebuilt_all_rows) = match dirty_rows {
            Some(mask) => {
                let cache = self.cell_cache.get_or_insert_with(|| {
                    crate::render::cell_builder::CachedInstances::new(rows, cols)
                });
                let (effective_mask, rebuilt_all_rows): (&[bool], bool) =
                    if cache.is_compatible(rows, cols) {
                        (mask, false)
                    } else {
                        // 缓存已与网格不匹配（缩放）：新缓存是空的，若仍按“干净行”
                        // 取值会复制 0 个实例并丢行。本帧强制全量重建。
                        *cache = crate::render::cell_builder::CachedInstances::new(rows, cols);
                        self.cell_full_mask_cache.resize(rows as usize, true);
                        (&self.cell_full_mask_cache, true)
                    };
                let converted = crate::render::cell_builder::build_instances_cached(
                    cell_data,
                    config,
                    font_pipeline,
                    effective_mask,
                    cache,
                    &mut self.cpu_instances,
                );
                (converted, rebuilt_all_rows)
            }
            None => {
                // 无基线：丢弃可能已失效的陈旧缓存并重建所有行。
                self.cell_cache = None;
                (
                    crate::render::build_instances_from_cell_data(
                        cell_data,
                        config,
                        font_pipeline,
                        &mut self.cpu_instances,
                    ),
                    true,
                )
            }
        };
        let build_done = std::time::Instant::now();

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
        let bands = partial_frame_bands(
            rebuilt_all_rows,
            self.cell_cache.as_ref(),
            rows,
            cols,
            font_pipeline.atlas_generation(),
            dirty_rows,
        );
        let partial = bands.is_some();
        let plan = crate::render::cell_builder::FramePatch {
            bands: bands.unwrap_or_default(),
            cell_height_px: grid_cell_height,
        };
        let gpu_start = std::time::Instant::now();
        let result =
            self.render_frame_with_plan(&cpu_instances, kgp_instances, kgp_overlay_dirty, &plan);
        let gpu_done = std::time::Instant::now();
        self.cpu_instances = cpu_instances;
        let total = gpu_done.duration_since(build_start);
        if total >= std::time::Duration::from_millis(30) && SLOW_CELL_BUILD_THROTTLE.should_log() {
            let ms = |duration: std::time::Duration| duration.as_secs_f64() * 1000.0;
            log::warn!(
                "SLOW_CELL_BUILD build={:.3}ms gpu={:.3}ms total={:.3}ms instances={} partial={partial} rows={rows}",
                ms(build_done.duration_since(build_start)),
                ms(gpu_done.duration_since(gpu_start)),
                ms(total),
                cell_data.len(),
            );
        }
        result
    }
}
