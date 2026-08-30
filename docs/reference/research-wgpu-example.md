# wgpu-example（matthewjberger/wgpu-example）深度研究报告

> 基于对仓库 `/home/runner/work/kudzu/kudzu/repositories/refs/wgpu-example` 全部源文件的逐行阅读，非推测。
> 所有引用位置格式为 `文件:行号`，相对该仓库根目录；torvox 侧引用相对 torvox 仓库根目录。
> 分析日期：2026 年（仓库当前 HEAD）。

---

## 目录

1. [项目概述与定位](#1-项目概述与定位)
2. [架构总览](#2-架构总览)
3. [逐文件深度分析](#3-逐文件深度分析)
   - 3.1 [Cargo.toml](#31-cargotoml)
   - 3.2 [src/main.rs 与 src/bin/rt.rs](#32-srcmainrs-与-srcbinrtrs)
   - 3.3 [src/lib.rs](#33-srclibrs)
   - 3.4 [src/raytracing.rs](#34-srcraytracingrs)
   - 3.5 [src/xr.rs](#35-srcxrrs)
   - 3.6 [WGSL 着色器](#36-wgsl-着色器)
   - 3.7 [构建/打包/CI 配套文件](#37-构建打包ci-配套文件)
   - 3.8 [README.md](#38-readmemd)
4. [功能对比：torvox 对照表](#4-功能对比torvox-对照表)
5. [依赖分析](#5-依赖分析)
6. [可吸收到 torvox 的具体内容](#6-可吸收到-torvox-的具体内容)
7. [项目文档吸收价值](#7-项目文档吸收价值)
8. [结论](#8-结论)

---

## 1. 项目概述与定位

**一句话定位**：一个用 Rust 写的跨平台 wgpu 图形演示样板（boilerplate），渲染一个旋转三角形，同时充当作者（matthewjberger）Nightshade 游戏引擎的脚手架，并外挂三个"极限"演示——硬件光线追踪、OpenXR VR（含 Quest 手部追踪）、WebGPU/WebGL。

**平台矩阵**（README.md:3, README.md:31-42）：

| 平台 | 入口 | 构建方式 |
|---|---|---|
| Native Desktop | `just run`（`app` bin） | `cargo build -r` |
| 硬件光线追踪（桌面） | `just run-rt`（`rt` bin） | `cargo build -r --bin rt` |
| WebGPU / WebGL | `just run-webgpu` / `run-webgl` | trunk（wasm32） |
| Android（arm64/x86_64） | `just run-android DEVICE_ID` | xbuild（`x` 命令） |
| Steam Deck | `just build-steamdeck` + scp | cross 交叉编译 |
| OpenXR VR（桌面 PCVR） | `just run-openxr` | `cargo run -r --features openxr` |
| OpenXR VR（Meta Quest 2/3/Pro/3S） | `just build-android-openxr` | xbuild + openxr feature |

**技术栈**（Cargo.toml:16-54）：wgpu 29（默认关闭默认 features，按平台开 backend）、winit 0.30.13、egui 0.34 + egui-wgpu/egui-winit、nalgebra-glm 0.20、web-time、bytemuck、futures；Android 侧用 android-activity 0.6（纯 Rust `android_main` 入口，**没有 Kotlin/Compose**）；VR 侧用 openxr 0.21 + ash 0.38 + wgpu-hal（`from_hal` 零拷贝接管 OpenXR 的 Vulkan 设备）。

**规模**：约 3800 行 Rust + 5 个 WGSL 着色器 + 一整套 justfile/Cross.toml/manifest.yaml/CI。

**架构核心思想**：单 crate `app_core`，`cdylib + rlib` 双 crate-type（Cargo.toml:8-10）——桌面/Web 当 rlib 用，Android 当 cdylib 从 `android_main` 导出。所有平台共享同一个 `App`/`Renderer`/`Scene` 结构，用 `#[cfg]` 切平台差异，用 feature 切渲染后端（`webgl`/`webgpu`/`android`/`openxr`，Cargo.toml:56-62）。

---

## 2. 架构总览

```
winit EventLoop / android_main / trunk
        │
        ▼
┌───────────────── App (lib.rs:77-87) ─────────────────┐
│ ApplicationHandler::resumed / window_event / suspended│
│ 持有: window, renderer, gui_state(egui), last_size    │
└──────────────────────┬───────────────────────────────┘
                       ▼
┌────────────── Renderer (lib.rs:450-455) ──────────────┐
│ Gpu(instance/device/queue/surface) + 深度纹理 +       │
│ egui_wgpu::Renderer + Scene                           │
│ render_frame(): scene.update → egui update_buffers →  │
│   get_current_texture(4 分支) → scene.render +        │
│   egui_renderer.render → submit + present             │
└──────────────────────┬───────────────────────────────┘
                       ▼
┌────── Gpu (lib.rs:608-614) ──┐   ┌── Scene (lib.rs:792-798) ──┐
│ new_async / resize /         │   │ 三角形 + uniform + pipeline │
│ create_depth_texture /       │   │ update(): 旋转 30°/s        │
│ new_async_headless           │   └────────────────────────────┘
└──────────────────────────────┘

两个独立二进制：
├─ rt bin (src/bin/rt.rs) → run_raytracing() (raytracing.rs:1136)
│    桌面专用: compute shader + BLAS/TLAS + ray_query 硬件光追
└─ openxr feature → run_xr() (xr.rs:1427)
    桌面/Quest: openxr 接管 Vulkan 设备 → wgpu from_hal → multiview 渲染
```

三条渲染路径互不相干，各自维护完整状态机：
- **主路径**（lib.rs）：winit/egui 驱动的三角形演示；
- **rt 路径**（raytracing.rs）：无 egui、无深度，compute→blit 两趟；
- **xr 路径**（xr.rs）：无 winit 事件循环（手动 `loop` + `poll_events`），直接提交 CompositionLayer。

---

## 3. 逐文件深度分析

### 3.1 Cargo.toml

| 位置 | 内容 | 说明 |
|---|---|---|
| Cargo.toml:1-6 | `name = "app_core"`, edition 2024, rust-version 1.97 | 与 torvox 相同的 edition/MSRV |
| Cargo.toml:8-10 | `[lib] crate-type = ["cdylib", "rlib"]` | **Android 纯 Rust 的关键**：cdylib 供 `android_main` 导出，rlib 供桌面 bin 链接 |
| Cargo.toml:12-14 | `[[bin]] name = "app"` | 默认 bin，桌面入口 |
| Cargo.toml:16-28 | 通用依赖 | bytemuck 1.25(derive)、egui 0.34、egui-wgpu 0.34(winit)、futures、log、nalgebra-glm 0.20(convert-bytemuck + serde-serialize)、web-time 1.1、wgpu 29(默认 features 关)、winit 0.30.13 |
| Cargo.toml:30-36 | `cfg(not(wasm32))`：ash 0.38(optional)、env_logger、egui-winit、gpu-allocator 0.28(optional)、pollster、wgpu-hal 29(vulkan, optional) | 桌面/Android 共享；pollster 把 async wgpu 初始化同步化 |
| Cargo.toml:38-39 | `cfg(all(not(wasm32), not(android)))`：openxr 0.21(static+loaded, optional) | **OpenXR 桌面静态链接 loader** |
| Cargo.toml:41-46 | wasm32：console_error_panic_hook、console_log、egui-winit(no default)、wasm-bindgen、wasm-bindgen-futures | Web 日志与 panic hook |
| Cargo.toml:48-54 | android：android-activity 0.6(native-activity)、android_logger 0.15、egui-winit(no default)、ndk-context 0.1、ndk-sys 0.6、openxr 0.21(loaded, optional) | **Quest 的 OpenXR loader 是运行时 dlopen（`loaded`），桌面是静态链接（`static`）**——同一个 feature 两套加载策略 |
| Cargo.toml:56-62 | features：`default = ["wgpu/default"]`、`openxr`（拉 ash+wgpu-hal+gpu-allocator）、`webgl`、`webgpu`、`android = ["wgpu/vulkan"]`、`android-openxr` | feature 矩阵就是平台矩阵 |

**设计要点**：`wgpu` 默认 features 关闭 + 按目标平台显式开 backend（android→vulkan，web→webgl/webgpu），避免编译时把所有 backend 都编进去。`openxr` feature 只允许在非 wasm 出现（依赖表的 cfg 保证）。

### 3.2 src/main.rs 与 src/bin/rt.rs

**src/main.rs**（共 15 行）：
- main.rs:2-5：`feature = "openxr"` 时直接 `app_core::run_xr()`——桌面 VR 模式根本不创建 winit 窗口；
- main.rs:7-14：默认路径构建 `winit::event_loop::EventLoop`（`ControlFlow::Poll`，main.rs:10）、`App::default()`、`event_loop.run_app()`。

**src/bin/rt.rs**（共 9 行）：
- rt.rs:1-4：非 wasm 非 android 时 `app_core::run_raytracing()`；
- rt.rs:6-9：wasm/android 直接 `panic!("The hardware ray tracing demo is desktop-only.")`——用编译期 + 运行期双保险挡住不支持平台。

**对 torvox 的启发**：torvox 的桌面调试路径（exec-bin）可以借鉴这种"bin 内 `#[cfg]` + 一句话 panic"的平台门禁写法。

### 3.3 src/lib.rs（1132 行）

#### 3.3.1 模块导出与 Android 入口

- lib.rs:9-13：`xr` 模块和 `run_xr` 仅 `all(not(wasm32), feature = "openxr")` 导出；
- lib.rs:15-19：`raytracing` 仅 `all(not(wasm32), not(android))` 导出——**模块级条件编译代替运行时检测**；
- **lib.rs:33-50 `android_main(app: AndroidApp)`（非 openxr）**：`#[unsafe(no_mangle)]` 导出符号；`android_logger::init_once`（lib.rs:36-38）；`EventLoop::builder().with_android_app(app)`（lib.rs:40-43）——winit 0.30 的 Android 集成方式是把 `AndroidApp` 塞进 EventLoopBuilder，而不是 JNI 函数；随后与桌面完全相同的 `App::default()` + `run_app`（lib.rs:46-49）。
- **lib.rs:52-75 `android_main`（openxr 版）**：后台线程跑 `run_xr()`（lib.rs:59-61），主线程 `app.poll_events` 循环等 `MainEvent::Destroy` 时 `std::process::exit(0)`（lib.rs:63-72）——Quest 上避免阻塞 Android 主线程的惯用法。

#### 3.3.2 `App`（lib.rs:77-87）

```rust
pub struct App {
    window: Option<Arc<Window>>,
    renderer: Option<Renderer>,
    gui_state: Option<egui_winit::State>,
    last_render_time: Option<Instant>,
    #[cfg(target_arch = "wasm32")]
    renderer_receiver: Option<futures::channel::oneshot::Receiver<Renderer>>,
    last_size: (u32, u32),
    initialized: bool,
}
```
全部 `Option`，因为 `ApplicationHandler::resumed/suspended` 允许任意次进入退出；`Renderer` 的创建是 async（wgpu adapter 查询），所以 wasm 上用 oneshot channel 异步回传（lib.rs:83-84, 191-204）。

#### 3.3.3 `ApplicationHandler` 实现

- **`suspended`（lib.rs:90-93）**：直接把 `renderer` 和 `window` 置 `None`——Android 后台时释放 GPU 资源，`resumed` 时重建。**这是 Android surface 生命周期的最简处理**（比 torvox 的 attach/detach 方案粗，见 §4）。
- **`resumed`（lib.rs:95-210）**：
  - 幂等保护：`window.is_some()` 直接 return（lib.rs:96-98）；
  - 桌面窗口标题（lib.rs:102-105）；wasm 从 DOM 拿 `<canvas id="canvas">` 并 `with_canvas`（lib.rs:111-126，`WindowAttributesExtWebSys`）；
  - **egui state 初始化（lib.rs:135-162）**：`egui_winit::State::new(context, viewport_id, &window, Some(scale_factor), Some(Theme::Dark), None)`；wasm 上 `pixels_per_point` 固定 1.0（lib.rs:145），Android 上设为窗口 scale_factor（lib.rs:148-152）；
  - 渲染器创建三路径：桌面 `env_logger::init()` 只做一次 + `pollster::block_on(Renderer::new(...))`（lib.rs:170-179）；Android 同样 block_on（lib.rs:181-187）；wasm 用 `wasm_bindgen_futures::spawn_local` + oneshot（lib.rs:189-205），并安装 `console_error_panic_hook`（lib.rs:194）；
  - 收尾：`initialized = true`（lib.rs:209）。
- **`window_event`（lib.rs:212-447）**：
  - wasm 渲染器就绪轮询 `receiver.try_recv()`（lib.rs:218-233）；
  - **egui 事件消费短路**：`gui_state.on_window_event(window, &event).consumed` 为真就 return（lib.rs:244-246）——egui 吞掉它处理的输入，游戏逻辑不再重复处理；
  - Esc 退出（lib.rs:249-260，`PhysicalKey::Code` 匹配）；ScaleFactorChanged 更新 egui ppi（lib.rs:261-271）；Resized 时 0 尺寸直接跳过（lib.rs:272-275，Android 常见）并 `renderer.resize`（lib.rs:278）；
  - **RedrawRequested（lib.rs:295-442）**：wasm 先比对 canvas 实际尺寸与 `last_size`（lib.rs:296-314，因为 JS 侧 resizeCanvas 改的是 canvas.width/height 而非窗口事件）；`delta_time` 计算（lib.rs:316-318）；egui input 组装（lib.rs:320-342，wasm 手动 `screen_rect`）；按 feature 选窗口标题（lib.rs:344-360）；`egui_ctx().run_ui` 画四个面板（顶部菜单栏 File/Edit、左 Scene Tree、右 Inspector、底 Console，lib.rs:362-424）——纯 UI 骨架，无实际功能；`tessellate`（lib.rs:428）→ `ScreenDescriptor`（lib.rs:430-439）→ `renderer.render_frame(...)`（lib.rs:441）；
  - 事件末尾 `window.request_redraw()`（lib.rs:446）——配合 `ControlFlow::Poll` 实现持续渲染。

#### 3.3.4 `Renderer`（lib.rs:450-606）

- 字段（lib.rs:450-455）：`gpu: Gpu`、`depth_texture_view: wgpu::TextureView`、`egui_renderer: egui_wgpu::Renderer`、`scene: Scene`。
- **`DEPTH_FORMAT = Depth32Float`（lib.rs:458）**。
- **`new`（lib.rs:460-486）**：`Gpu::new_async` → `create_depth_texture` → egui renderer（带 `depth_stencil_format: Some(Depth32Float)`，lib.rs:471-474，保证 egui 与 3D 场景同 pass 深度兼容）→ `Scene::new`。
- **`resize`（lib.rs:488-491）**：surface 与深度纹理一起重建。
- **`render_frame`（lib.rs:493-605）**——本仓库最有移植价值的一段：
  1. `scene.update(queue, aspect_ratio, delta_time)`（lib.rs:502-503）；
  2. egui 纹理增量 set/free（lib.rs:505-512）；
  3. 创建 encoder（lib.rs:514-519）；`egui_renderer.update_buffers`（lib.rs:521-527）；
  4. **`get_current_texture` 四分支（lib.rs:529-546）**：
     - `Success | Suboptimal` → 直接使用；
     - `Outdated | Lost` → `surface.configure` 重配后再取一次，失败 return（lib.rs:532-541）；
     - `Occluded | Timeout` → return（不 present）；
     - 其他 → `panic!`。
  5. surface view 显式描述（lib.rs:548-561，含 `format: Some(self.gpu.surface_format)`）；
  6. 单 render pass：清屏 `(0.19, 0.24, 0.42)`（lib.rs:572-577）+ 深度清 1.0（lib.rs:584-587）→ `scene.render` + `egui_renderer.render`（lib.rs:594-600，`render_pass.forget_lifetime()`）；
  7. `submit` + `surface_texture.present()`（lib.rs:603-604，wgpu 29 API；torvox 的 wgpu 30 是 `queue.present(texture)`）。

#### 3.3.5 `Gpu`（lib.rs:608-790）

- 字段（lib.rs:608-614）：surface/device/queue/surface_config/surface_format——**与 torvox `Renderer` 的 GPU 部分字段一一对应**。
- `aspect_ratio`（lib.rs:617-619）。
- `resize`（lib.rs:621-625）。
- **`create_depth_texture`（lib.rs:627-656）**：Depth32Float、`RENDER_ATTACHMENT | TEXTURE_BINDING`（可被采样）、view 显式指定 D2 全层。
- **`new_async`（lib.rs:658-725）**——标准 wgpu 引导：
  - `InstanceDescriptor::new_without_display_handle_from_env()`（lib.rs:663-664）：**无 display handle 创建实例**，之后 `create_surface(window)`（lib.rs:665）；
  - `request_adapter` 用 `PowerPreference::default()` + `compatible_surface`（lib.rs:667-674）；
  - `request_device`：`memory_hints: MemoryHints::default()`、`required_features: Features::default()`、**limits 用 `Limits::default().using_resolution(adapter.limits())`**（lib.rs:682-683）、wasm webgl 用 `downlevel_webgl2_defaults().using_resolution(...)`（lib.rs:686-688）、`experimental_features: ExperimentalFeatures::disabled()`、`trace: Trace::Off`（lib.rs:689-690）；
  - **表面格式：优先非 sRGB（lib.rs:698-703）**：`.find(|f| !f.is_srgb()).unwrap_or(formats[0])`——与 torvox context.rs:441-449 完全相同的取舍（见 §4）；
  - `surface_config`（lib.rs:705-714）：`usage: RENDER_ATTACHMENT`、`present_mode: caps.present_modes[0]`（**直接取第一个，不做优先级选择**，不如 torvox 的 `select_present_mode`）、`desired_maximum_frame_latency: 2`。
- **`new_async_headless`（lib.rs:727-789）**：无窗口创建——`compatible_surface: None`（lib.rs:735），**伪造一个 Win32 dummy surface**（lib.rs:768-780，`create_surface_unsafe` + `NonZeroIsize::new(1)`）以便测试/离屏。torvox 的测试用 `global_gpu()`（context.rs:51-75）+ lavapipe，是另一条路，但此 dummy-surface 技巧在 torvox 无窗口 CI 测试里也可参考。

#### 3.3.6 `Scene`（lib.rs:792-921）

- 字段（lib.rs:792-798）：`model: nalgebra_glm::Mat4`、vertex/index buffer、`UniformBinding`、pipeline。
- `new`（lib.rs:801-827）：`create_buffer_init` + `UniformBinding::new` + `create_pipeline`。
- `render`（lib.rs:829-837）：`set_pipeline` → `set_bind_group(0, ...)` → vertex/index buffer → `draw_indexed(0..3, 0, 0..1)`。
- `update`（lib.rs:839-859）：`perspective_lh_zo(aspect, 80°, 0.1, 1000)`（lib.rs:840-841）+ `look_at_lh`（lib.rs:842-846）+ 绕 Y 轴 30°/s 旋转（lib.rs:847-851）→ `uniform.update_buffer`。
- `create_pipeline`（lib.rs:861-920）：WGSL 内嵌字符串（`SHADER_SOURCE`，lib.rs:1103-1132）；`TriangleStrip` + `strip_index_format: Uint32` + `FrontFace::Cw`（lib.rs:886-894）；深度 `Less` + 写开启（lib.rs:895-901）；`BlendState::ALPHA_BLENDING`（lib.rs:912）。

#### 3.3.7 数据与工具结构

- `Vertex`（lib.rs:923-942）：`position: [f32;4]` + `color: [f32;4]`；`vertex_attributes()` 用 `wgpu::vertex_attr_array!`（lib.rs:931-933）。
- `UniformBuffer`（lib.rs:944-948）+ `UniformBinding`（lib.rs:950-1009）：buffer/bind_group/bind_group_layout 三件套 + `update_buffer` 用 `queue.write_buffer`（lib.rs:997-1008）。
- 常量数据：`VERTICES`/`INDICES`（lib.rs:1011-1026）、**`CUBE_VERTICES`/`CUBE_INDICES`（lib.rs:1028-1066，8 顶点 36 索引单位立方体）**、`GREEN_CUBE_VERTICES`（lib.rs:1068-1101）——这三个常量被 xr.rs 复用（xr.rs:443, 452, 461）。

### 3.4 src/raytracing.rs（1143 行）

**功能**：`just run-rt` 的硬件光线追踪演示。整条路径只在桌面编译（lib.rs:15-19 门禁）。

#### 3.4.1 常量与工具

- 常量（raytracing.rs:14-21）：`WIDTH/HEIGHT = 800`、`LIGHT_POSITION = [-5.5, 6.5, 3.5]`、`LIGHT_RADIUS = 1.0`、`LIGHT_INTENSITY = 1.5`、`MAX_BOUNCES = 8.0`、`SAMPLES_PER_FRAME = 6.0`。
- `Vertex`（raytracing.rs:23-29）：`pos_refl: [f32;4]`（xyz + 反射率塞进 w）、`normal: [f32;4]`（xyz + 材质 id 塞进 w）、`color: [f32;4]`——**属性通道复用技巧**，着色器侧 `vertex0.pos_refl.w` / `vertex0.normal.w` 读出（raytrace.wgsl:139-140）。
- `Uniforms`（raytracing.rs:31-39）：view_inverse、proj_inverse、light_pos(+radius)、params(time/bounces/intensity/spp)、frame(accum/total)。
- `Camera`（raytracing.rs:41-68）：yaw/pitch/distance 球坐标 + `eye()`。
- `look_at_rh`（raytracing.rs:70-92）：手写右手系 lookAt（列主序矩阵手工构造）。
- `perspective_vk`（raytracing.rs:94-114）：手写 Vulkan 约定（`z_far/(z_near-z_far)`、-1 透视项）的投影矩阵——**与 wgpu 的 NDC 约定（0..1 深度）匹配**。

#### 3.4.2 场景构建（CPU 程序化建模）

- `SceneData`（raytracing.rs:116-119）+ `build_scene`（raytracing.rs:121-212）：地板 + 玻璃盒 + 高镜柱 + 反光圆环 + 白球 + 红盒 + 黄盒 + 紫球 + 粉球 + 发光球（`add_sphere(LIGHT_POSITION, ...)`，raytracing.rs:203-210）。
- `add_floor`（raytracing.rs:214-234）：单 normal 四顶点，reflectivity 存 w。
- `add_sphere`（raytracing.rs:236-278）：24 stacks × 48 slices 经纬球。
- `rotate_y`（raytracing.rs:280-288）。
- `add_box`（raytracing.rs:290-381）：6 面 × 4 角硬编码 + yaw 旋转。
- `add_torus`（raytracing.rs:383-429）：48×24 分段参数化圆环。

**可移植性评价**：这套程序化几何生成（`fn add_*(&mut SceneData, ...)` 模式）零依赖、可单测，torvox 若做单元测试几何或离屏渲染验证可直接搬。

#### 3.4.3 `RayTracer` 结构（raytracing.rs:431-462）

字段分组清晰：surface/device/queue/config；uniform/accum buffer + output texture view；compute pipeline 三件套；blit pipeline 三件套；`tlas` + `#[allow(dead_code)] blas/vertex/index`；相机与帧累积状态（`camera_dirty`、`accum_frame`、`total_frame`、`start_time`）。

#### 3.4.4 `RayTracer::new`（raytracing.rs:465-764）

1. **能力检查（raytracing.rs:483-492）**：`adapter.features().contains(wgpu::Features::EXPERIMENTAL_RAY_QUERY)` 否则 panic 带清晰提示——"GPU 不支持就明确失败"的范例；
2. **设备请求（raytracing.rs:496-508）**：`required_features: EXPERIMENTAL_RAY_QUERY`、`Limits::default().using_resolution(adapter.limits()).using_minimum_supported_acceleration_structure_values()`、`experimental_features: unsafe { wgpu::ExperimentalFeatures::enabled() }`——**注意 wgpu 29 的实验特性需要显式 unsafe 开启**；
3. present mode 优先 `Fifo`（raytracing.rs:517-524）；
4. 顶点/索引 buffer usage 含 `BLAS_INPUT`（raytracing.rs:539-554）；
5. **compute bind group layout（raytracing.rs:574-637）**：binding 0 = `AccelerationStructure { vertex_return: false }`（TLAS）、1 = storage texture `Rgba8Unorm` WriteOnly、2 = uniform（相机）、3/4 = read-only storage（顶点/索引）、5 = read-write storage（累积 buffer）；
6. compute pipeline（raytracing.rs:639-660）：`include_str!("raytrace.wgsl")`、entry `main`；
7. blit pipeline（raytracing.rs:673-735）：无顶点 buffer 全屏三角形 + `blit.wgsl`。

#### 3.4.5 加速结构构建（raytracing.rs:766-830）

`build_acceleration_structures`：
- `BlasTriangleGeometrySizeDescriptor`（raytracing.rs:773-779）：Float32x3 顶点、Uint32 索引、`OPAQUE` flags；
- `create_blas`（raytracing.rs:781-790）：`PREFER_FAST_TRACE` + `update_mode: Build`；
- `create_tlas`（raytracing.rs:792-797）：`max_instances: 1`；
- 单实例 `TlasInstance::new(&blas, identity_matrix, 0, 0xFF)`（raytracing.rs:799-804）；
- `encoder.build_acceleration_structures` 一次提交（raytracing.rs:806-827）。

#### 3.4.6 帧循环（raytracing.rs:832-1042）

- `create_accum_buffer`（raytracing.rs:832-839）：`width*height*16` 字节（f32x4 每像素）。
- `create_output_texture`（raytracing.rs:841-857）：`Rgba8Unorm`，`STORAGE_BINDING | TEXTURE_BINDING`。
- `resize`（raytracing.rs:917-943）：重建 accum/output + 两个 bind group + `camera_dirty = true`。
- **`update_uniforms`（raytracing.rs:945-978）——时域累积的核心**：`camera_dirty` 时 `accum_frame = 0` 重置累积（raytracing.rs:946-948），否则 `accum_frame += 1`；每帧写 `frame: [accum_frame, total_frame, 0, 0]`。着色器用 `1.0/(frame+1)` 做指数滑动平均（raytrace.wgsl:309-315），相机一动就从头累积——"静止锐化、移动重启"的经典渐进式渲染。
- **`render`（raytracing.rs:980-1042）**：get_current_texture 同样四分支（raytracing.rs:981-992）→ compute pass `dispatch_workgroups(width.div_ceil(8), height.div_ceil(8), 1)`（raytracing.rs:1013-1015）→ blit pass `draw(0..3, 0..1)`（raytracing.rs:1037）→ submit + present。

#### 3.4.7 `RayTracingApp`（raytracing.rs:1045-1143）

- `resumed`（raytracing.rs:1054-1067）：`with_resizable(false)` 固定 800×800 窗口。
- 交互（raytracing.rs:1094-1126）：左键拖拽改 yaw/pitch（`delta * 0.005`，pitch 限 ±1.5，raytracing.rs:1109-1112）、滚轮指数 zoom（`0.9_f32.powf(scroll)`，距离限 [3,30]，raytracing.rs:1123-1124）——每次改动置 `camera_dirty = true`。
- `run_raytracing`（raytracing.rs:1136-1143）。

### 3.5 src/xr.rs（1498 行）

**功能**：OpenXR VR 模式。桌面走 SteamVR/VD 串流，Android 走 Quest 原生（`android-openxr` feature）。复用 lib.rs 的 `Scene`、`CUBE_VERTICES` 等常量，新增网格地面（grid.wgsl）、程序化天空（sky.wgsl）、双手柄方块。

#### 3.5.1 常量与 Uniform 布局

- `MULTIVIEW_MASK = NonZeroU32::new(0b11)`（xr.rs:7）——`0b11` = 只渲染 layer 0/1，即两个眼睛。
- `VK_TARGET_VERSION = 1.1`（xr.rs:9-15）——OpenXR 要求的 Vulkan 最低版本，同时换算成 ash 的 `make_api_version`。
- `MeshUniform`（xr.rs:17-21）：`mvp: [[[f32;4];4];2]`——**两眼的 MVP 数组**（mesh.wgsl 用 `@builtin(view_index)` 索引）。
- `GridUniform`（xr.rs:23-34）：view_proj×2 + camera_world_pos×2 + 网格参数 + `_padding: [f32;3]`。
- `SkyUniform`（xr.rs:36-41）：proj_inv×2 + view×2。

#### 3.5.2 `XrContext` 结构（xr.rs:43-78）

持有：swapchain 纹理数组、深度视图、立方体/网格/天空三套 pipeline 资源、`xr::Swapchain<xr::Vulkan>`、stage/左右手 space、动作集（move/left_trigger/right_trigger）、frame_stream/frame_waiter、session、instance、`_vk_instance/_vk_entry`（ash 持有，防析构顺序问题）、分辨率、玩家位置、`session_running`。

#### 3.5.3 `XrContext::new`（xr.rs:81-779）——**OpenXR × wgpu 双栈初始化，本仓库技术密度最高的函数**

阶段一：**XR 实例**（xr.rs:82-133）
- 桌面 `xr::Entry::linked()`（xr.rs:83）/ Android `unsafe { xr::Entry::load() }` + `initialize_android_loader()`（xr.rs:86-89）；
- 扩展：`khr_vulkan_enable2` 必需（xr.rs:92），Android 加 `khr_android_create_instance`（xr.rs:96）；
- `create_instance`（xr.rs:99-109）→ `system(HEAD_MOUNTED_DISPLAY)`（xr.rs:111）→ `enumerate_view_configuration_views(PRIMARY_STEREO)`（xr.rs:113-116）取推荐分辨率（xr.rs:118-121）；
- `graphics_requirements::<xr::Vulkan>` 版本检查（xr.rs:123-133）。

阶段二：**Vulkan 实例接管**（xr.rs:135-209）
- `ash::Entry::load()`（xr.rs:135）；
- **`wgpu_hal::vulkan::Api::Instance::desired_extensions(&vk_entry, ...)`（xr.rs:137-141）——让 wgpu-hal 决定要哪些 Vulkan 扩展**，再把这些扩展传给 `xr_instance.create_vulkan_instance`（xr.rs:154-174，含 `std::mem::transmute` 的函数指针类型转换）；
- `xr_instance.vulkan_graphics_device(...)` 选物理设备（xr.rs:182-184）；
- **`wgpu_hal::vulkan::Api::Instance::from_raw(...)`（xr.rs:196-209）把 ash 的 vkInstance 包装成 wgpu-hal 实例**；
- `expose_adapter(vk_physical_device)`（xr.rs:211-213）拿到 `wgpu::ExposedAdapter`。

阶段三：**Vulkan 设备创建（xr.rs:220-287）**——多视图（multiview）是关键：
- `physical_device_multiview_features { multiview: vk::TRUE }` 通过 `.push_next()` 挂进 `DeviceCreateInfo`（xr.rs:232-242）——**不启用 multiview，OpenXR 双视图渲染无法工作**；
- `xr_instance.create_vulkan_device(...)`（xr.rs:244-266）创建设备（OpenXR 要求设备必须由 runtime 创建，这样 xrCreateSession 才能用）；
- **`wgpu_exposed_adapter.adapter.device_from_raw(vk_device, ...)`（xr.rs:269-280）把 ash 设备包成 wgpu-hal 设备**（指定 queue family 0）。

阶段四：**wgpu 侧对象（xr.rs:289-305）**
- `wgpu::Instance::from_hal::<wgpu_hal::api::Vulkan>(wgpu_vk_instance)`（xr.rs:290）；
- `create_adapter_from_hal`（xr.rs:291）+ `create_device_from_hal(wgpu_open_device, ...)`（xr.rs:293-305，`memory_hints: Performance`）。

阶段五：**Session 与动作（xr.rs:307-370）**
- `create_session::<xr::Vulkan>` 传 instance/physical_device/device 指针 + queue family（xr.rs:307-318）；
- 动作集：move(thumbstick)、left/right_hand_pose、left/right_trigger（xr.rs:320-333）；
- `suggest_interaction_profile_bindings` 绑定 Oculus Touch 路径（xr.rs:335-359）；
- 左右手 space 从动作创建 + STAGE 参考空间（xr.rs:363-370）。

阶段六：**Swapchain 与 wgpu 纹理桥接（xr.rs:372-437）**
- `session.create_swapchain`：`COLOR_ATTACHMENT | SAMPLED`、`R8G8B8A8_SRGB`、`array_size: 2`（双眼一图）（xr.rs:372-383）；
- **`enumerate_images()` 拿到的 `vk::Image` 逐个转 wgpu::Texture（xr.rs:385-437）**：`device.as_hal().texture_from_raw(color_image, ...)`（xr.rs:390-414，`TextureMemory::External`）→ `wgpu_device.create_texture_from_hal(...)`（xr.rs:415-434）。**这就是"OpenXR 独占纹理 → wgpu 渲染"的零拷贝路径**，torvox 若未来做 VR 只能走这条路。

阶段七：**三条管线（xr.rs:439-708）**
- mesh pipeline（xr.rs:466-546）：复用 `crate::Vertex` 布局 + `mesh.wgsl`，`multiview_mask: MULTIVIEW_MASK`（xr.rs:544），目标格式 `Rgba8UnormSrgb`（xr.rs:521）；
- grid pipeline（xr.rs:548-637）：无顶点 buffer、`TriangleList`、混合 `SrcAlpha/OneMinusSrcAlpha`（xr.rs:602-609）、深度 `LessEqual` + **`DepthBiasState { constant: 2, slope_scale: 2.0 }` 防 z-fighting**（xr.rs:628-632）、`depth_write_enabled: false`（xr.rs:625）；
- sky pipeline（xr.rs:639-708）：`vs_sky/fs_sky`、`BlendState::REPLACE`（xr.rs:693）、无深度。
- 深度纹理（xr.rs:710-735）：**`depth_or_array_layers: 2` 的 D2Array**，view 也是 `D2Array` 2 层（xr.rs:725-735）——与 swapchain 双眼一图的结构对应。

#### 3.5.4 事件与状态机

- **`poll_events`（xr.rs:781-815）**：`SessionStateChanged`：`READY → session.begin(PRIMARY_STEREO)`（xr.rs:788-793）、`STOPPING → session.end()`（xr.rs:794-798）、`EXITING/LOSS_PENDING → 返回 false`（xr.rs:799-803）；`InstanceLossPending → false`（xr.rs:807-810）。
- `is_session_running`（xr.rs:817-819）。
- **`shutdown`（xr.rs:821-857）**：`request_exit()` 后**最多轮询 200 次**（每次 10ms 空转，xr.rs:848）等 `STOPPING → end()`、`EXITING → break`——OpenXR 的退出握手必须等 runtime 确认，不能直接 drop。
- `wait_frame`（xr.rs:859-861）。

#### 3.5.5 移动与渲染

- **`update_movement`（xr.rs:863-904）**：`sync_actions` → 读 thumbstick 状态（死区 0.1，xr.rs:872）→ 取 `views[0]` 头部位姿四元数 → 计算头部 yaw（xr.rs:881-889）→ 把 thumbstick 输入旋转到头朝向坐标系（xr.rs:895-896）→ 以 2.0 m/s 移动 `player_position`（xr.rs:891-899）。**OpenXR 左手系坐标 → 屏幕空间的转换范例**。
- **`render_frame`（xr.rs:906-1424）**：
  1. `frame_stream.begin()`（xr.rs:914）；`!should_render` 提前 `end`（xr.rs:916-923）；
  2. `locate_views` + `POSITION_VALID|ORIENTATION_VALID` 检查（xr.rs:925-940）；
  3. `acquire_image` + `wait_image(INFINITE)`（xr.rs:942-943）；`color_view` D2Array（xr.rs:948-958）；
  4. **逐眼矩阵（xr.rs:960-1031）**：OpenXR 四元数要先绕 X 翻转 180°（`flip_x * openxr_quat`，xr.rs:971-976，OpenXR 右手系 ↔ 渲染左手系的经典转换），视图 `look_at_rh`（xr.rs:988），投影从 FOV 切平面手算（xr.rs:991-1020，`tan_left/right/up/down` → 标准非对称投影矩阵）；
  5. 四个独立 encoder 依次提交：sky（xr.rs:1046-1073，Clear BLACK）→ 三角形（xr.rs:1089-1125，深度 Clear）→ 网格（xr.rs:1159-1193，深度 Load）→ 左右手方块（xr.rs:1249-1288 / 1345-1384，扳机 >0.5 变绿色，xr.rs:1240-1247）；
  6. `release_image`（xr.rs:1387）→ `CompositionLayerProjection` + `SwapchainSubImage { image_array_index: view_index }`（xr.rs:1389-1415）→ `frame_stream.end(..., &[&projection_layer])`（xr.rs:1417-1421）。
- **`run_xr`（xr.rs:1427-1498）**：手动主循环（无 winit）：`poll_events → wait_frame → update_movement → render_frame`，任一失败即退出（xr.rs:1438-1487）；退出前 `device.poll(PollType::Wait { .. })` 排空 GPU 工作（xr.rs:1490-1493），再 `shutdown()` + drop。

### 3.6 WGSL 着色器

#### 3.6.1 blit.wgsl（16 行）
全屏三角形（`positions[vertex_index]`，blit.wgsl:4-11）+ `textureLoad(source, vec2<i32>(position.xy), 0)`（blit.wgsl:14-16）——compute 输出 → 屏幕的最简 blit，无采样器、无 mip。

#### 3.6.2 grid.wgsl（153 行）——**无限网格，对 torvox 有直接参考价值（背景/调试网格）**

- Uniform（grid.wgsl:1-9）：`view_proj/camera_world_pos` 各 2 份（双眼）+ 网格参数 + `is_orthographic`。
- 顶点（grid.wgsl:20-55）：6 顶点 switch 构造 2 三角形（grid.wgsl:24-32）；**相机跟随**：`world_pos = pos + camera_world_pos.xz`（grid.wgsl:38-42）；正交投影时钳制 `clip_pos.z`（grid.wgsl:47-49）。
- 片元（grid.wgsl:62-136）：
  - **三档 LOD**：`lod = log10(scale * min_pixels / cell_size) + 1`（grid.wgsl:72），`cell_size_lod0/1/2` 每档 ×10（grid.wgsl:73-75）；
  - 用 `dpdx/dpdy` 算屏幕空间线宽，`mod_pos` 求距离，alpha 按到网格线距离渐变（grid.wgsl:77-95）；
  - LOD 间 `smoothstep(0.2, 0.8, lod_fade)` 淡入淡出（grid.wgsl:97, 107）；
  - 远处透明度衰减 `1 - smoothstep(0.8*size, 3*size, dist)`（grid.wgsl:115-119）；
  - X/Z 轴分别染红/绿（grid.wgsl:121-129）；`color.a < 0.02 → discard`（grid.wgsl:131-133）；
  - 手写 `log10/saturate/max2` 辅助函数（grid.wgsl:139-153）。
- **评价**：完整的抗锯齿无限网格实现（含 LOD 与正交适配），无纹理依赖，任何 3D 场景可直接复用。

#### 3.6.3 mesh.wgsl（29 行）
`mvp: array<mat4x4<f32>, 2>` + `@builtin(view_index)` 选眼（mesh.wgsl:19-24）——多视图渲染的最简形态。

#### 3.6.4 sky.wgsl（66 行）
- 顶点（sky.wgsl:14-31）：`view_index` + 4 顶点全屏四边形（`tmp1*4-1, tmp2*4-1`），`transpose(mat3x3(view))` 反转视图旋转（sky.wgsl:25），用 `proj_inv` 反投影出世界方向（sky.wgsl:26-28）。
- 片元（sky.wgsl:33-66）：上下四色渐变（`pow(1-height, 1/curve)` 曲线，sky.wgsl:49-55）→ 亮度 ×1.3（sky.wgsl:57）→ 太阳盘 `smoothstep(0, 0.02, sun_angle)`（sky.wgsl:59-63）。

#### 3.6.5 raytrace.wgsl（319 行）——**wgpu 内联光线查询的完整教材**

- `enable wgpu_ray_query;`（raytrace.wgsl:1）——WGSL 实验扩展声明。
- 绑定（raytrace.wgsl:17-22）：`acceleration_structure`、storage texture、uniform、顶点/索引 storage、累积 storage。
- 常量：**RGB 三通道不同折射率 `IOR_R=1.50, IOR_G=1.52, IOR_B=1.54`**（raytrace.wgsl:24-26）——玻璃色散（彩虹边缘）的来源。
- 随机数：`pcg` 哈希（raytrace.wgsl:44-48）+ `rnd`（raytrace.wgsl:50-52）。
- `sky_color`（raytrace.wgsl:54-57）、`glsl_mod`（raytrace.wgsl:59-61）、`checker` 棋盘（raytrace.wgsl:63-67）。
- `make_basis`（raytrace.wgsl:69-79）：从法线构造正交基（`abs(normal.x) > 0.9` 选参考轴）。
- `sample_cone`（raytrace.wgsl:81-93）：圆锥内均匀采样——**面光源软阴影**。
- **`trace_closest`（raytrace.wgsl:95-148）**：`rayQueryInitialize` → `while (rayQueryProceed(&query)) {}` → `rayQueryGetCommittedIntersection`（raytrace.wgsl:97-101）；`RAY_QUERY_INTERSECTION_NONE` 时 `t = -1`（raytrace.wgsl:103-106）；重心坐标插值顶点属性（raytrace.wgsl:116-129）；背面翻转法线（raytrace.wgsl:131-134）。
- **`trace_shadow`（raytrace.wgsl:150-167）**：`RAY_FLAG_TERMINATE_ON_FIRST_HIT` 阴影光线。
- **`render_sample`（raytrace.wgsl:169-287）**——路径追踪核心：
  - 抖动采样 `jitter = rnd-0.5`（raytrace.wgsl:170-171）；
  - **hero 波长采样（raytrace.wgsl:186-187, 206-213）**：`sample_index % 3` 决定 R/G/B 哪个波长参与折射，色散只作用一次（`dispersed` 标志，raytrace.wgsl:235-246），折射后 throughput ×3 补偿；
  - 菲涅尔 `r0 = ((1-1.52)/(1+1.52))²` + Schlick 近似（raytrace.wgsl:221-224）；
  - 全反射检测 `dot(refracted, refracted) < 1e-6`（raytrace.wgsl:227）；
  - mat_id==3 自发光（raytrace.wgsl:200-203）、mat_id==2 玻璃（raytrace.wgsl:205-250）、mat_id==0 棋盘（raytrace.wgsl:253-254）；
  - 阴影 4 次圆锥采样（raytrace.wgsl:266-273）；`albedo * (0.12 + diffuse)` 环境项（raytrace.wgsl:274）；
  - 反射率累积 `throughput *= reflectivity`（raytrace.wgsl:282），`<=0.001` 提前终止（raytrace.wgsl:279-281）。
- **`main`（raytrace.wgsl:289-319）**：8×8 workgroup；`textureDimensions` 边界检查（raytrace.wgsl:291-294）；每像素 `spp` 次采样 + 确定性种子（raytrace.wgsl:300-302）；**时域累积**：`frame.x == 0` 直接覆盖，否则 `mix(previous, color, 1/(frame.x+1))`（raytrace.wgsl:309-315）；`pow(1/2.2)` gamma 映射（raytrace.wgsl:317）。

### 3.7 构建/打包/CI 配套文件

#### 3.7.1 justfile（179 行）
任务清单：`build/check/docs/fix/format/lint/test/udeps/watch/versions`（justfile:10-38, 163-179）；wasm：`init-wasm`（rustup target + trunk，justfile:32-34）、`build-webgl/build-webgpu/run-webgl/run-webgpu`（justfile:56-70）；Android：`init-android`（rustup 4 个 target + **从 git master 装 xbuild**，justfile:73-78）、`pair-android/connect-android/list-android`（justfile:80-90）、`build-android/build-android-all/build-android-openxr`（justfile:92-103）、`install-android/run-android/run-android-x64`（justfile:105-116）、模拟器 `start-android-emulator/list-android-emulators`（unix/windows 双版本，justfile:118-134）、私有 `_ensure-libs-x64`（justfile:136-144，xbuild 要求每个 ABI 目录存在）；Steam Deck：`init-steamdeck/build-steamdeck/deploy-steamdeck/steamdeck-ssh`（justfile:146-161）；环境变量 `RUST_LOG=info,wgpu_core=off`、`RUST_BACKTRACE=1`（justfile:3-4）。

#### 3.7.2 Cross.toml（17 行）
Steam Deck 交叉编译：`PKG_CONFIG_ALLOW_CROSS=1` passthrough（Cross.toml:1-4）；pre-build 装 `libvulkan-dev:amd64 libwayland-dev libxkbcommon-dev libx11-dev libxcb* libudev-dev`（Cross.toml:6-11）；`PKG_CONFIG_PATH` 指向 x86_64 pkgconfig（Cross.toml:13-17）。

#### 3.7.3 X.toml（4 行）
xbuild 配置：包名 `app_core`（X.toml:1-2）、`min_sdk_version = 24` / `target_sdk_version = 34`（X.toml:4-5）——**与 torvox 的 minSdk 一致（torvox 的 AGENTS/构建文档）**。

#### 3.7.4 manifest.yaml（26 行）
xbuild Android 清单：`runtime_libs: [libs]`（manifest.yaml:2-3，把 `libs/arm64-v8a/libopenxr_loader.so` 打包进 APK）；`uses_feature: android.hardware.vulkan.level version 1`（manifest.yaml:9-12）；**Meta Quest 专属**：`com.oculus.supportedDevices: quest2|questpro|quest3|quest3s`（manifest.yaml:14-16）、`com.oculus.intent.category.VR`（manifest.yaml:30）；`NativeActivity` + `android.app.lib_name: app_core`（manifest.yaml:17-24）；`config_changes` 完整列表（manifest.yaml:21）。

#### 3.7.5 rust-toolchain（4 行）
`stable` + rustfmt/clippy + `wasm32-unknown-unknown` target（rust-toolchain:1-4）。

#### 3.7.6 index.html（59 行）
trunk 入口：`<link data-trunk rel="rust" href="Cargo.toml" data-target-name="app">`（index.html:39）；`resizeCanvas()` JS 按 `devicePixelRatio` 放大 canvas（index.html:43-57）——与 lib.rs:296-314 的 wasm 尺寸轮询配合。

#### 3.7.7 .github/workflows/rust.yml（119 行）
6 个 job：check（`cargo check --features webgpu`）、check_wasm（`--target wasm32-unknown-unknown --lib`）、test（装 X11 依赖后 `cargo test --lib`）、fmt、clippy（`-D warnings`）、trunk（wget 二进制 trunk 0.18.8 构建 webgpu）。**CI 特色**：`mozilla-actions/sccache-action` + `SCCACHE_GHA_ENABLED` + `RUSTC_WRAPPER=sccache`（rust.yml:10-12, 21），`Swatinem/rust-cache` 只在 main 分支 save（rust.yml:20）。

#### 3.7.8 .github/workflows/pages.yml（33 行）
main 分支 push 触发，trunk `--release --public-url "${GITHUB_REPOSITORY#*/}"`（pages.yml:27）+ `JamesIves/github-pages-deploy-action` 部署 dist 到 Pages（pages.yml:29-33）。

### 3.8 README.md（325 行）
质量极高的平台文档：快速开始矩阵表（README.md:31-42）；硬件光追原理说明（README.md:62-83，明确"wgpu 无 ray tracing pipeline/SBT，只能 compute + ray_query"）；Android 无线调试分步（README.md:130-143）；**模拟器排障经验（README.md:162-171）：MuMu/BlueStacks 的 `libEGL_emulation.so` 会让 wgpu 的 `Surface::configure` 报 `EGL_BAD_ALLOC`，只有 Google 官方模拟器 `-gpu host` 可用**；Apple Silicon 上 x86_64 镜像无 HVF 加速（README.md:171）；**xbuild 0.2.0 的 x64 链接 bug 需要 git master（README.md:226）**；Quest 安装流程（README.md:288-320）。

---

## 4. 功能对比：torvox 对照表

> torvox = Android 终端（Termux 风格），Rust native 侧用 wgpu 30 渲染（workspace Cargo.toml:25），Compose UI + JNI 桥（native/src/android/ffi.rs）。

| # | 功能 | wgpu-example 位置 | torvox 有没有 | torvox 位置 / 差异说明 |
|---|---|---|---|---|
| 1 | **Android 纯 Rust 入口**（`android_main` + android-activity） | lib.rs:33-50 | **有，但路径不同** | torvox 是 JNI 导出（`native/src/android/ffi.rs`）+ Compose UI；wgpu-example 无 Kotlin，直接 `android_main`。torvox 的 Kotlin UI 是产品需求（Termux 式），不应回归纯 Rust |
| 2 | **surface 生命周期 suspended/resumed 重建** | lib.rs:90-93（置 None） | **有，更精细** | torvox `attach_surface`/`detach_surface`（context.rs:386-491, ADR-0007），处理了格式变化重建 pipeline（context.rs:484-489）和"surface 未配置永久失败"陷阱（context.rs:425-426）。wgpu-example 的做法（全部置 None 重建）在终端场景会丢渲染状态，torvox 不应模仿 |
| 3 | **表面格式选择（非 sRGB 优先）** | lib.rs:698-703 | **有，相同策略** | context.rs:441-449（注释甚至解释了 SwiftShader 失败原因）。两仓库独立得出同一结论 |
| 4 | **present mode 选择** | 直接 `caps.present_modes[0]`（lib.rs:710）；rt 用 Fifo 优先（raytracing.rs:517-524） | **有，更优** | context.rs:873-881 `select_present_mode`：Mailbox → Fifo → AutoVsync → Immediate。torvox 已领先，无需吸收 |
| 5 | **get_current_texture 异常分支处理** | lib.rs:529-546（Success/Suboptimal/Outdated/Lost/Occluded/Timeout） | **有，更强** | pass.rs:132-181：除 reconfigure+retry 外，还有 SwiftShader panic 兼容（pass.rs:65, 158）和 Mali-G57 驱动 hang 检测（pass.rs:163）。torvox 已领先 |
| 6 | **深度纹理（Depth32Float）** | lib.rs:458, 627-656 | **无** | torvox 全部 `depth_stencil: None`（pipeline.rs:148, 250, 344, 417, 467；pass.rs:94 等）。终端是 2D 层叠，不需要深度测试；若未来做 3D 背景/动画才需要 |
| 7 | **egui 集成（egui-wgpu 同 pass 渲染）** | lib.rs:135-162, 362-441 | **无** | torvox UI 是 Compose，native 无 UI 框架。egui 对 torvox 是死重，**不适用** |
| 8 | **winit 事件循环 + ApplicationHandler** | main.rs:7-14, lib.rs:89-448 | **无** | torvox 事件经 JNI 从 Kotlin 侧进入（event.rs）。winit 在 Android 上会与 Compose 冲突，**不适用** |
| 9 | **WASM/WebGPU/WebGL 支持** | Cargo.toml:41-46, 59-60; index.html | **无** | torvox 是 Android 终端，无 Web 目标。**不适用** |
| 10 | **OpenXR VR（含 Quest）** | xr.rs 全文件 | **无** | torvox 无 VR 需求。若未来有，xr.rs:220-287（multiview 设备创建）和 xr.rs:385-437（纹理桥接）是唯一可行的 wgpu 方案 |
| 11 | **硬件光线追踪（BLAS/TLAS + ray_query）** | raytracing.rs + raytrace.wgsl | **无** | 需要 `EXPERIMENTAL_RAY_QUERY`（raytracing.rs:483-492），Android 移动 GPU 基本不支持，且演示仅桌面编译。**不适用** |
| 12 | **时域累积渲染（渐进式抗锯齿）** | raytracing.rs:945-978; raytrace.wgsl:309-315 | **无** | 终端是每帧全量重绘，无累积需求；但"dirty 标志重置累积"模式可类比 torvox 的脏矩形思路 |
| 13 | **程序化几何生成（add_box/sphere/torus/floor）** | raytracing.rs:214-429 | **无** | torvox 测试需要几何时可直接搬（零依赖、纯函数式） |
| 14 | **无限 LOD 网格着色器** | grid.wgsl | **无** | torvox 若做 3D 背景/调试可视化可直接复用（见 §6.2） |
| 15 | **xvk 多视图（multiview）渲染** | xr.rs:7, 232-242, 544 | **无** | 同上，仅 VR 需要 |
| 16 | **headless GPU 初始化（dummy surface）** | lib.rs:727-789 | **有，不同方案** | torvox 用 `global_gpu()`（context.rs:51-75）+ 测试直接建 Renderer；wgpu-example 的 Win32 dummy surface 技巧（lib.rs:768-780）可作 CI 无窗口渲染的备选 |
| 17 | **wasm panic hook / console_log** | lib.rs:194-195 | **无** | torvox 自研 logcat 日志（android/logging.rs:8 注明弃用 android_logger）；**torvox 缺全局 panic hook**（docs/dependency-research-rust-aggressive.md:22 已记录此缺口） |
| 18 | **Android 日志（android_logger）** | lib.rs:36-38, 55-57 | **有，自研替代** | torvox logging.rs 自研 logcat+文件双写（docs/project-health.md:58）。android_logger 不引入 |
| 19 | **CI：sccache + rust-cache 分层** | rust.yml:10-21, 36-41 | **部分** | torvox 有 android-tests/release/rust-checks 三个 workflow，未见 sccache（见 §6.4） |
| 20 | **GitHub Pages 部署 wasm demo** | pages.yml | **无** | torvox 无 Web demo；但 torvox 有 screenshot 测试产物展示需求，可参考 pages 部署思路 |
| 21 | **xbuild 打包 + manifest.yaml runtime_libs** | X.toml, manifest.yaml | **无** | torvox 用 Gradle（android/build.gradle.kts），体系不同；但 `runtime_libs` 机制（打包 .so 进 APK）在 torvox 需要打包额外 .so（如字体库）时可借鉴 |
| 22 | **Cross.toml 交叉编译（Steam Deck）** | Cross.toml | **无** | torvox 桌面调试用 exec-bin 本地构建；若未来要 Linux ARM 交叉编译可参考 |
| 23 | **justfile 任务矩阵** | justfile | **部分** | torvox 用 scripts/ + tasks/ + Makefile？风格不同；justfile 的 `RUST_LOG=info,wgpu_core=off`（justfile:3）值得抄（见 §6.5） |
| 24 | **`using_resolution(adapter.limits())` limits 策略** | lib.rs:683 | **有** | wgpu_backend.rs:117 用 `required_limits: adapter.limits()`（等价，甚至更直接） |
| 25 | **`InstanceDescriptor::new_without_display_handle_from_env`** | lib.rs:664 | **有，不同** | wgpu_backend.rs:82 用 `InstanceDescriptor { display: ... }` 显式 display handle（Android 需要）。torvox 的注释（wgpu_backend.rs:71-74）说明这正是 Android 必需 |
| 26 | **命令式 UI 骨架（egui 面板）** | lib.rs:362-424 | **无** | Compose 承担。**不适用** |
| 27 | **`web-time` 统一时间源** | Cargo.toml:26 | **无** | torvox 桌面用 std::time，Android 侧无渲染循环计时需求；若未来跨平台统一可考虑（轻量） |
| 28 | **RUST_LOG 环境变量默认值** | justfile:3 | **无** | torvox 日志级别在 logging.rs 配置；可加 `RUST_LOG` 支持便于调试（§6.5） |

**总结论**：wgpu-example 与 torvox 在"wgpu 引导 + surface 管理 + 帧循环"层面是同一套打法，但 torvox 在 Android 专用路径上（attach/detach、present mode、acquire 异常、SwiftShader/Mali 兼容）**已经全面领先**；wgpu-example 的独特价值在三个 torvox 没有的领域——3D 渲染配套（深度纹理/网格/天空/光追）、VR（OpenXR 接管 wgpu）、Web（trunk/wasm）。真正可吸收的是：**grid.wgsl 无限网格、程序化几何、OpenXR 桥接知识（备忘）、CI sccache 配置、README 的排障经验**。

---

## 5. 依赖分析

| 依赖 | 版本 | 用途 | 是否适用于 torvox | 是否激进 |
|---|---|---|---|---|
| `wgpu` | 29（torvox 用 **30**） | 渲染 | ✅ 已是 torvox 核心（workspace Cargo.toml:25） | torvox 版本更新，**不降级** |
| `winit` | 0.30.13 | 窗口/事件/Android 入口 | ❌ torvox 用 JNI + Compose | 激进（Android 上双事件源冲突） |
| `egui` / `egui-wgpu` / `egui-winit` | 0.34 | UI + wgpu 集成渲染 | ❌ torvox UI 是 Compose | 激进（引入整套 retained-GUI） |
| `bytemuck` | 1.25 | 顶点/uniform 字节化 | ✅ torvox 已有（workspace Cargo.toml:28） | 成熟 |
| `nalgebra-glm` | 0.20 | 数学（矩阵/四元数） | ⚠️ 可选 | torvox 目前无 3D 数学需求；若做 3D 特效（网格背景/动画）再引入，否则激进 |
| `web-time` | 1.1 | wasm 兼容计时 | ❌ torvox 无 wasm | 激进（无收益） |
| `futures` | 0.3 | wasm oneshot/异步 | ✅ torvox 已有（workspace Cargo.toml:42），用于 block_on | 成熟 |
| `pollster` | 0.4 | async 阻塞执行 | ✅ 可替代 torvox 的 `futures::executor::block_on`（context.rs:54） | 非必需（等价） |
| `env_logger` | 0.11 | 桌面日志 | ⚠️ torvox 桌面调试可用，但 torvox 有自研 logger | 低 |
| `log` | 0.4 | 日志门面 | ✅ torvox 已有 | 成熟 |
| `android-activity` | 0.6 | Android 应用入口 | ❌ torvox 走 JNI + Kotlin Activity | 激进（与 Compose 体系冲突） |
| `android_logger` | 0.15 | logcat | ❌ torvox 自研 logging.rs | torvox 已评估过（docs/project-health.md:58） |
| `ndk-context` / `ndk-sys` | 0.1 / 0.6 | NDK 绑定 | ⚠️ torvox 用 `jni`（workspace Cargo.toml:33）直接拿 ANativeWindow，无需 ndk-context | 低 |
| `openxr` | 0.21 | VR | ❌ torvox 无 VR | 激进（大量 unsafe FFI + 平台限制） |
| `ash` / `wgpu-hal` / `gpu-allocator` | 0.38 / 29 / 0.28 | OpenXR 底层桥接 | ❌ 仅 VR 需要 | 激进 |
| `console_error_panic_hook` / `console_log` | 0.1 / 1.0 | wasm 日志 | ❌ | 激进 |
| `wasm-bindgen` / `wasm-bindgen-futures` | 0.2 / 0.4 | wasm 互操作 | ❌ | 激进 |
| `trunk`（工具） | 0.18.8 | wasm 构建 | ❌ | 激进 |

**结论**：wgpu-example 的依赖里 torvox 真正缺且值得考虑的只有 `nalgebra-glm`（做 3D 时）和 `pollster`（可选便利）。其余要么 torvox 已有（bytemuck/log/futures/wgpu 且版本更新），要么与 torvox 架构冲突（winit/egui/android-activity/android_logger/wasm 全家桶/OpenXR 全家桶）。版本激进程度：wgpu 29 + winit 0.30 + egui 0.34 都是各自生态的当前主线，不算激进；真正的"激进"是 `wgpu_ray_query`（实验特性）和 OpenXR 的 `std::mem::transmute` 函数指针 hack（xr.rs:157-168）——这两处 torvox 明确不要碰。

---

## 6. 可吸收到 torvox 的具体内容

### 6.1 【高价值】无限 LOD 网格着色器 → 未来 3D 调试/背景

**来源**：`src/grid.wgsl`（153 行，无外部依赖，无纹理）。
**可复用场景**：torvox 若做 3D 渲染验证（如渲染器调试场景、性能测试基准场景、或者终端未来叠加 3D 层），一个无限网格地面是最便宜的"有深度感"的背景。

**吸收方式**：复制 `grid.wgsl` 全文；uniform 结构改为单视图（torvox 无 VR）：

```wgsl
// 改造自 wgpu-example/src/grid.wgsl（原文件为双眼数组，torvox 单眼）
struct Uniform {
    view_proj: mat4x4<f32>,          // 原 grid.wgsl:2 为 array<mat4x4<f32>, 2>
    camera_world_pos: vec4<f32>,     // 原 grid.wgsl:3
    grid_size: f32,                  // 原 grid.wgsl:4
    grid_min_pixels: f32,            // 原 grid.wgsl:5
    grid_cell_size: f32,             // 原 grid.wgsl:6
    orthographic_scale: f32,         // 原 grid.wgsl:7
    is_orthographic: f32,            // 原 grid.wgsl:8
}
```
删除 `@builtin(view_index)` 参数（原 grid.wgsl:21），`u.view_proj[view]` → `u.view_proj`（原 grid.wgsl:45），`u.camera_world_pos[view]` → `u.camera_world_pos`（原 grid.wgsl:34）。其余（LOD 逻辑 grid.wgsl:62-136、轴染色 grid.wgsl:121-129、正交钳制 grid.wgsl:47-49）原样保留。

**管线配置参考**（改自 xr.rs:588-637）：`TriangleList`、无顶点 buffer、混合 `SrcAlpha/OneMinusSrcAlpha`（xr.rs:602-609）、深度 `LessEqual` + `depth_write_enabled: false` + `DepthBiasState { constant: 2, slope_scale: 2.0 }`（xr.rs:623-633）——注意 torvox 管线目前全部无深度（pipeline.rs:148 等），**要启用网格需先给 torvox 加深度纹理**（lib.rs:627-656 的 `create_depth_texture` 可原样移植，仅需把 `wgpu::TextureFormat` 路径换成 wgpu 30 的写法）。

### 6.2 【高价值】程序化几何生成器 → torvox 渲染测试

**来源**：`src/raytracing.rs:214-429` 的 `add_floor/add_sphere/add_box/add_torus` + `SceneData`（raytracing.rs:116-119）。
**可复用场景**：torvox 的 `render/tests.rs`（8.5 万行测试）目前只有终端网格；若要给渲染器加"3D 几何正确性"冒烟测试（如投影/裁剪验证），这套生成器是零依赖、可单测的。

**吸收方式**：原样复制 `SceneData`、`rotate_y`（raytracing.rs:280-288）、四个 `add_*` 函数；`Vertex`（raytracing.rs:23-29）的"w 分量塞反射率/材质 id"技巧（raytrace.wgsl:139-140 消费）在 torvox 若不用可以简化为 `[f32;3]` 法线 + 单独属性。

```rust
// 建议注释（置于 add_box 上方，raytracing.rs:290 原版无文档）
// 6 面 × 4 角硬编码 + 绕 Y 轴 yaw 旋转的立方体生成。
// 法线不随 yaw 旋转（world_normal 用 rotate_y 处理），
// 顶点局部坐标先缩放 half 再旋转——顺序不能反（raytracing.rs:356-370）。
```

### 6.3 【中价值】CI 加 sccache 缓存

**来源**：`.github/workflows/rust.yml:10-12, 21`（`SCCACHE_GHA_ENABLED` + `RUSTC_WRAPPER=sccache` + `mozilla-actions/sccache-action`）。
**torvox 现状**：三个 workflow（android-tests.yml / release.yml / rust-checks.yml）未见 sccache 配置。
**建议**：在 rust-checks.yml 的 check/clippy/test job 里加：
```yaml
env:
  SCCACHE_GHA_ENABLED: "true"
  RUSTC_WRAPPER: "sccache"
steps:
  - uses: mozilla-actions/sccache-action@v0.0.9
```
参考 rust.yml:14-21 的完整组合（dtolnay/rust-toolchain + Swatinem/rust-cache shared-key + sccache-action 顺序）。注意 rust.yml:20 的 `save-if: ${{ github.ref == 'refs/heads/main' }}`——只在 main 分支写缓存，PR 只读，避免缓存竞争。

### 6.4 【中价值】get_current_texture 分支写法核对（wgpu 29→30 备忘）

wgpu-example 的四分支（lib.rs:529-546）与 torvox pass.rs:132-181 逻辑一致，但注意 **API 演进**：wgpu-example 是 wgpu 29 的 `surface_texture.present()`（lib.rs:604），torvox 是 wgpu 30 的 `queue.present(self.texture)`（context.rs:33）。torvox 侧的写法（pass.rs:29-181：`request.surface.get_current_texture()` 走 `SurfaceFrameRequest`、失败计数、重试上限）比参考仓库更完善，**维持 torvox 现状，不吸收**。

### 6.5 【低价值但顺手】调试默认环境变量

**来源**：justfile:3-4 `export RUST_LOG := "info,wgpu_core=off"`、`export RUST_BACKTRACE := "1"`。
**建议**：torvox 桌面调试入口（exec-bin）或 scripts 里加 `RUST_LOG=info,wgpu_core=off`——`wgpu_core=off` 能屏蔽 wgpu 内部 validation 噪音（torvox 的 log_gpu_error 已捕获 uncaptured error，context.rs:14-16，但 wgpu_core 的 info 日志仍会刷屏）。

### 6.6 【知识备忘】OpenXR × wgpu 桥接（torvox 暂无需求，记录备查）

如果 torvox 未来要 VR 终端（远期的"终端宇宙"愿景），唯一可行的 wgpu 路径已由 xr.rs 完整示范：
1. **设备必须由 OpenXR 创建**：`xr_instance.create_vulkan_device`（xr.rs:244-266），wgpu 侧用 `device_from_raw` 接管（xr.rs:269-280）；
2. **multiview 必须显式开启**：`PhysicalDeviceMultiviewFeatures { multiview: vk::TRUE }` push_next（xr.rs:232-242）；
3. **swapchain 纹理零拷贝**：`texture_from_raw` + `create_texture_from_hal`（xr.rs:390-434），`TextureMemory::External`；
4. **坐标转换**：OpenXR 四元数绕 X 翻转 180°（xr.rs:971-976）；
5. **退出握手**：`request_exit` 后最多 200 次轮询等 STOPPING/EXITING（xr.rs:832-854）。
这五条是 wgpu 生态里没有第二份的完整答案，值得在 ADR 里留一条指针。

### 6.7 【不吸收清单】（避免踩坑）

- ❌ winit Android 入口（lib.rs:33-50）：torvox 是 Compose + JNI，winit 的 `with_android_app` 要求全权接管 Activity，冲突；
- ❌ `std::mem::transmute` 函数指针 hack（xr.rs:157-168）：unsafe 且脆弱，仅 OpenXR 需要；
- ❌ `panic!("desktop-only")` 平台门禁（rt.rs:6-9）：torvox 是单一 Android 目标，无此需求；
- ❌ `new_async_headless` 的 Win32 dummy surface（lib.rs:768-780）：torvox 测试已有 lavapipe 路径（context.rs:64 提示 `VK_ICD_FILENAMES`），dummy surface 是 Windows 专用；
- ❌ egui 的"事件 consumed 短路"模式（lib.rs:244-246）：torvox 的输入在 Kotlin 侧已分派，无等价物。

---

## 7. 项目文档吸收价值

README.md 是全仓库最有吸收价值的部分（比代码更值得抄）：

1. **平台排障知识库**（README.md:161-171）：
   - MuMu/BlueStacks 模拟器与 wgpu Vulkan 的 `EGL_BAD_ALLOC` 冲突（README.md:166-169）——torvox 若支持模拟器测试，这条可直接进 docs/lessons/03-android-pitfalls.md；
   - `xbuild 0.2.0` x64 链接 bug 需 git master（README.md:226）——torvox 用 Gradle 不受影响，但若未来评估 xbuild 路线有用；
   - Apple Silicon 无 HVF 加速 x86_64 guest（README.md:171）。
2. **`just --list` 即文档**（justfile:6-8）：任务名即帮助文本（`#` 注释自动成为说明）。torvox 的 scripts/ 任务可以用同样约定。
3. **硬件光追原理一节**（README.md:62-67）："wgpu 无 ray-tracing pipeline/SBT，只能 compute + ray_query"——这是 wgpu 生态少见的权威总结，值得抄进 torvox 的 docs/reference/ 作为 wgpu 能力边界备忘。
4. **README 的平台矩阵表格**（README.md:31-42）结构清晰，torvox 的 docs/standards/BUILD.md 可参照重排。
5. **manifest.yaml 的 Quest 元数据**（manifest.yaml:9-30）：`uses_feature` + `meta_data` 的写法是 Android 打包文档的稀有样例；torvox 若要声明 `android.hardware.vulkan.level`（目前只在代码层面假设 Vulkan，context.rs:63-67），可抄这份 YAML。

另外 torvox 的 `docs/reference-projects.md:496-497` 已有一行 wgpu-example 简评（"winit + egui + wgpu 跨平台样板"），本文档是它的深化，可在该处补一行链接。

---

## 8. 结论

1. **架构层面**：wgpu-example 是"一套代码多平台"的样板，torvox 是"单平台深度优化"的终端；两者在 wgpu 引导、表面格式选择、acquire 异常处理上殊途同归，且 torvox 在 Android 专用路径上已经领先（attach/detach、present mode 优先级、Mali/SwiftShader 兼容）。
2. **真正可吸收的三样**：① `grid.wgsl` 无限 LOD 网格（torvox 做 3D 背景/调试时，需配套先引入深度纹理，lib.rs:627-656）；② 程序化几何生成器（raytracing.rs:214-429，供渲染测试）；③ CI sccache 配置（rust.yml:10-21）。
3. **知识备忘一项**：OpenXR × wgpu 的五步桥接法（§6.6），torvox 短期无需求但 wgpu 生态无第二份完整答案。
4. **明确不吸收**：winit/egui/android-activity/wasm/OpenXR 依赖全家桶、`transmute` hack、dummy-surface 技巧——与 torvox 架构冲突或版本落后（torvox 的 wgpu 30 比其 wgpu 29 新）。
5. **文档层面**：README 的模拟器排障（README.md:161-171）与 xbuild 坑（README.md:226）、`just` 任务即文档约定（justfile:6-8）、wgpu 光追能力边界总结（README.md:62-67），值得抄入 torvox 的 lessons/reference 文档体系。
