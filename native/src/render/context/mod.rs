//! GPU 上下文：把 wgpu 的 instance、adapter、device 与管线集中到单个 `Renderer` 结构。

mod atlas;
mod gpu;
mod renderer;
mod surface;

pub use gpu::FrameContext;
#[cfg(test)]
pub(crate) use gpu::{
    INITIALIZATION_COUNT, global_gpu_for_tests, gpu_error_is_surface_lost, surface_loss_transition,
};
pub(crate) use gpu::{log_gpu_error, try_global_gpu};
#[cfg(test)]
pub(crate) use renderer::{rebuild_blocked_by_in_flight_surface, reuse_attached_surface};
#[cfg(test)]
pub(crate) use surface::surface_extent;
pub use surface::{MIN_VERTEX_BUFFER_SIZE, apply_scroll_px_offset, orthographic_projection};

/// GPU 渲染器：持有 wgpu 资源与管线。字段按空行分组：核心资源、单元管线资源、图集资源、
/// Kitty 图形协议（kgp_*）、逐帧瞬时状态。
///
/// 线程安全：`Renderer` 为 `Send + Sync`，位于全局 `RENDER_STATE` 互斥锁后，
/// `begin_frame()` 与 `render_frame()` 由单一渲染线程以 `&mut self` 调用。
pub struct Renderer {
    pub(crate) device: wgpu::Device,
    pub(crate) queue: wgpu::Queue,
    pub(crate) surface: Option<std::sync::Arc<wgpu::Surface<'static>>>,
    pub(crate) surface_config: Option<wgpu::SurfaceConfiguration>,
    /// 当前挂载的 `ANativeWindow` 句柄（地址，等价于指针比较）：`attach_surface`
    /// 能否原地 reconfigure 的唯一附加依据，见 [Self::can_reuse_attached_surface]。
    ///
    /// 真机日志里回前台与切会话送来的都是**新**窗口，而旧 surface
    /// 包裹的 BufferQueue 已被 SurfaceFlinger 回收：少了这道比对时快速路径照走，
    /// reconfigure 之后每帧 `dequeueBuffer: BufferQueue has been abandoned` →
    /// `ERROR_SURFACE_LOST_KHR` → 连续两次取帧失败判死 → 宿主重建 `SurfaceView`，
    /// 期间屏幕停在旧会话（或纯主题背景）那一帧。
    attached_window: Option<usize>,
    /// 最近一次 [`Renderer::release_surface`] 释放的窗口句柄（地址）。
    ///
    /// 为什么释放后还要记它：释放时若在途取帧排不干（工作线程仍持有旧 surface 的
    /// `Arc` 克隆，其下的 Vulkan surface 尚未销毁），宿主紧接着就会把**同一个**
    /// `ANativeWindow` 送回来重建（`TerminalSurface.postDelayedSurfaceRecreate` 实测
    /// 同址 attach）。那时 `attached_window` 已是 `None`，只凭它无从判断「这个窗口上
    /// 还有旧 surface 活着」，重建就会撞上 `ERROR_NATIVE_WINDOW_IN_USE_KHR` 触发
    /// wgpu-hal panic（2026-10-18 日志：整进程退出、整轮仪器化作废）。
    /// 判定见 [`rebuild_blocked_by_in_flight_surface`]；重建成功后清零。
    retired_window: Option<usize>,
    /// 连续 surface 级取纹理失败的计数（见 [Self::note_surface_acquire]）。
    surface_loss_streak: u8,
    /// 连续「本帧跳过」计数：工作线程死亡或取纹理持续超时时，帧循环每帧
    /// 只记 warn 不升级——累计到上限必须以 error 报出「屏幕冻结」这一事实。
    consecutive_skip_frames: u32,
    /// surface 已判死：缓存的 `surface` 指向已废弃的 BufferQueue，reconfigure 永不
    /// 复活（实测 abandoned BufferQueue 下每帧 `Lost` + `ERROR_SURFACE_LOST_KHR`，
    /// 22 分钟零帧上屏）。置位后 `attach_surface` 改走重建慢路径，且由
    /// `renderWithNewOutput` 的状态位上报宿主换新的 `ANativeWindow`。
    surface_invalidated: bool,
    /// 测试钩子：置位后每帧取纹理都按 surface 级失败处理。
    ///
    /// 真机上被遗弃的 BufferQueue 无法从进程内制造（那是 SurfaceFlinger 的回收决策），
    /// 而自愈路径若只能等系统回收来验证，就永远拿不到回归护栏。持续注入复刻的正是
    /// 实测到的真实形态（`abandoned=6148`、`begin_frame failed=3074`：每帧都失败），
    /// 供仪器化用例断言「宿主换新 `SurfaceView` 后画面重新有墨迹」。
    ///
    /// 只经进程内 JNI 暴露，无生产调用方；由调用方显式关闭。
    surface_loss_injected: bool,
    pub(crate) cell_pipeline: Option<wgpu::RenderPipeline>,
    pub(crate) quad_vertex_buffer: wgpu::Buffer,
    pub(crate) cell_bind_group: Option<wgpu::BindGroup>,
    pub(crate) cell_uniform_buffer: Option<wgpu::Buffer>,
    pub(crate) instance_buffer: Option<wgpu::Buffer>,
    /// 跨帧复用的 CPU 侧实例缓冲（避免每帧约 100KB 分配）。
    pub(crate) cpu_instances: Vec<crate::render::CellInstance>,
    /// 行级脏区实例缓存：保留各行的实例切片，使干净行只复制而不重建。
    pub(crate) cell_cache: Option<crate::render::cell_builder::CachedInstances>,
    /// 可复用的全 true 脏标记（长度为当前网格行数），用于 `cell_cache` 刚从零重建后
    /// 的那一帧（resize）：此时把“干净”行当作无缓存会导致丢行。
    pub(crate) cell_full_mask_cache: Vec<bool>,
    pub(crate) viewport_scroll_px: f32,
    /// 上一呈现帧实际使用的视口像素偏移。当前偏移非零或与此值不同
    /// 时，pass.rs 禁用脏带部分路径：纹理拷贝无法亚像素平移累加器，
    /// 部分路径会把旧像素与平移后的新几何混叠（滚动残留/撕裂）。
    pub(crate) last_drawn_viewport_scroll_px: f32,
    pub(crate) atlas_texture: Option<wgpu::Texture>,
    pub(crate) atlas_view: Option<wgpu::TextureView>,
    pub(crate) atlas_sampler: Option<wgpu::Sampler>,
    pub(crate) pipeline_format: wgpu::TextureFormat,
    pub(crate) projection_width: u32,
    pub(crate) projection_height: u32,
    pub(crate) readback_texture: Option<wgpu::Texture>,
    pub(crate) readback_buffer: Option<wgpu::Buffer>,
    pub(crate) background: wgpu::Color,
    pub(crate) kgp_pipeline: Option<wgpu::RenderPipeline>,
    /// 创建 kgp_pipeline 时所用的表面格式；表面格式变更后必须重建管线。
    pub(crate) kgp_pipeline_format: Option<wgpu::TextureFormat>,
    pub(crate) kgp_bind_group: Option<wgpu::BindGroup>,
    pub(crate) kgp_uniform_buffer: Option<wgpu::Buffer>,
    pub(crate) kgp_sampler: Option<wgpu::Sampler>,
    pub(crate) kgp_instance_buffer: Option<wgpu::Buffer>,
    pub(crate) kgp_texture: Option<wgpu::Texture>,
    pub(crate) kgp_atlas_width: u32,
    pub(crate) kgp_atlas_height: u32,
    pub(crate) render_paused: bool,
    /// 持久离屏帧累加器：权威帧内容跨帧存活，使部分（脏带）帧可合成到上次输出上，
    /// 每帧一次 `copy_texture_to_texture` 呈现到交换链。
    pub(crate) frame_texture: Option<wgpu::Texture>,
    /// `frame_texture` 的 view：只在纹理重建时换。view 的默认描述符不依赖任何
    /// 可变状态，纹理不变则内容同一，每帧重建等于每帧一次 image view 对象分配
    /// 与驱动侧对象表增长（同 `kgp_bind_group` 惰性化的理由）。
    pub(crate) frame_view: Option<wgpu::TextureView>,
    /// 累加器内容陈旧/未知，下一帧必须全量重绘（首帧、resize、格式变更、surface
    /// 重挂载）；成功全量绘制后清除。
    pub(crate) frame_invalidated: bool,
    /// 交换链是否支持 `COPY_DST`（挂载时依据 `usages` 检查）；为 false 时走旧的
    /// 直接渲染路径并忽略脏带。
    pub(crate) swapchain_copy_supported: bool,
    /// 呈现诊断的汇总闸门：逐帧呈现按 500ms 窗口汇总成一行，窗口内的呈现帧数
    /// 随该行一起交出（规范要求逐帧诊断限频且不得吞掉计数）。
    pub(crate) present_log: crate::render::slow_log::CountedLogThrottle,
}

#[cfg(test)]
mod tests;
