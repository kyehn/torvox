# 深度研究：wgpu-in-app（jinleili）— 亲自逐文件阅读版

> 研究日期：2026-08-06 | 克隆位置：`repositories/refs/wgpu-in-app`（depth 1，commit 见 git log）
> 研究方式：**主代理亲自逐文件完整阅读**（非子代理生成）
> 项目链接：https://github.com/jinleili/wgpu-in-app
> 许可证：MIT | 语言：Rust（wgpu 30）+ Kotlin（Compose）+ Swift

## 0. 项目定位

"不用 winit 把 wgpu 嵌入既有 iOS/Android 应用"的权威样板。核心 crate `app-surface` 提供跨平台 surface 抽象（iOS CAMetalLayer / Android ANativeWindow / wasm / winit 桌面），`wgpu-in-app` 是演示 crate（6 个示例：boids 计算着色器、MSAA 线、立方体、水面、阴影、HDR ASTC 贴图）。README 论证了"非游戏 App 需要自由控制输入设备（手写笔、传感器），不应被 winit 接管事件循环"。

## 1. 完整文件清单与逐文件研究

### 1.1 `app-surface/src/lib.rs`（491 行，跨平台核心抽象）

**`ViewSize`（:64-69）**：`#[repr(C)]` 的 `{width: u32, height: u32}`——JNI/Swift FFI 边界结构。

**归一化函数族（:71-131）**：
- `normalize_view_size`（:71-73）：`(0,0)` → `(1,1)`，防止 0 尺寸导致 wgpu 报错
- `normalize_scale_factor`（:75-81）：非法（0/负/NaN/Inf）→ 1.0
- `physical_size_from_logical_size`（:84-102）：逻辑×缩放 → 物理，各轴单独校验
- `logical_size_from_physical_size`（:105-116）：逆运算
- `normalize_touch_point`（:118-131）：触摸点归一化到 [0,1]（除以物理尺寸×缩放）

**`resize_surface_config`（:133-145）**：幂等 resize 判定——尺寸没变返回 false（避免重复 `surface.configure`）。

**`IASDQContext`（:152-174）**：`Instance/Adapter/Device/Queue` + `Surface<'static>` + `SurfaceConfiguration` 打包，**全部可 Clone**（注释 :151：wgpu v24 起 Instance/Adapter/Device/Queue 可 Clone）。`update_config_format`（:163-173）在运行期切换格式，view_formats 特例处理（webgl 不支持 view_formats）。

**`SurfaceFrame` trait（:198-250）**：
- `view_size()` / `resize_surface()` / `resize_surface_by_size()` / `pintch()` / `touch()` / `normalize_touch_point()` / `enter_frame()` / `get_current_frame_view()` / `create_current_frame_view()`
- **`create_current_frame_view`（:215-249）**：acquire 四分支处理——
  - `Success | Suboptimal` → 直接用
  - `Timeout | Outdated | Lost` → `surface.configure` 重配后重试一次，仍失败 `panic!`
  - `Occluded` → 返回 `None`（跳过本帧，不 panic）
  - `Validation` → `panic!("Validation error acquiring texture")`
  - view 格式默认 `config.format.add_srgb_suffix()`（:242，sRGB 优先）

**`create_iasdq_context`（:306-363）**：adapter/device 请求 → `surface.get_capabilities` → format 选择：wasm32 去 sRGB（Chrome WebGPU 不支持 sRGB，:317-323）；view_formats 三档（:324-344）：webgl `vec![]`、**Android `vec![format]`**（:329-339，注释详述 Android 不支持 SURFACE_VIEW_FORMATS downlevel flag）、其余 `[srgb, non-srgb]` 双格式。`get_default_config` 创建 config。

**`request_device`（:365-423）**：
- `PowerPreference::from_env().unwrap_or(HighPerformance)`（:371-372）——**环境变量可覆盖电源偏好**
- `apply_limit_buckets: false`（:375）——不应用 wgpu 的 limit 分档（要完整 limits）
- NVIDIA 特例（:399-402）：unix 上 NVIDIA 去 `EXPERIMENTAL_RAY_QUERY`（wgpu 已知问题）
- `required_features: adp_features` + `required_limits: adapter.limits()`（:409-410）——全量
- `memory_hints: MemoryHints::Performance`（:412）
- `trace: Trace::Off`（:413）

**单测（:425-491）**：5 个测试覆盖 normalize 函数族 + resize_surface_config（0 尺寸 clamp、非法 scale 回退、幂等判定）。

### 1.2 `app-surface/src/android.rs`（123 行，Android 平台实现）

**`NativeWindow`（:50-122）**：
- `new`（:61-70）：`ANativeWindow_fromSurface(env, surface)`——注释明确"返回时引用计数 +1，防止安卓端意外释放"（:63-64）
- 内部 `Arc<Mutex<*mut ANativeWindow>>`（:51）——跨线程共享裸指针
- `get_raw_window`（:72-75）/ `get_width`（:77-79）/ `get_height`（:81-83）/ `view_size`（:85-87）
- **`Drop`（:90-96）**：`ANativeWindow_release`——RAII 释放
- **`HasWindowHandle`/`HasDisplayHandle`（:98-120）**：raw-window-handle 实现——`AndroidNdkWindowHandle` + `AndroidDisplayHandle`
- **`unsafe impl Send/Sync`（:122-123）**：ANativeWindow 是引用计数对象（NDK 合约）

**`AppSurface`（:10-48）**：
- `new`（:18-43）：`Arc<NativeWindow>` → `InstanceDescriptor::new_with_display_handle(Box::new(native_window.clone()))` → **`Backends::VULKAN` 硬编码**（:20）→ `create_surface(SurfaceTarget::Window(handle))` → `futures_lite::future::block_on(create_iasdq_context(...))`（同步阻塞创建）
- `scale_factor: normalize_scale_factor(1.0)`（:39）——Android 上 scale=1（物理像素即逻辑）

### 1.3 `app-surface/src/ios.rs`（89 行，iOS 平台实现）

**`IOSViewObj`（:12-25）**：Swift 传入的 `{view, metal_layer, maximum_frames, callback_to_swift}`——CAMetalLayer 指针直接给 Rust。

**`AppSurface`（:27-71）**：
- `new`：`msg_send![obj.view, frame]` 取 CGRect → `physical_size_from_logical_size` → **`Backends::METAL` 硬编码**（:47）→ `create_surface_unsafe(SurfaceTargetUnsafe::CoreAnimationLayer(obj.metal_layer))`（:59-62，**iOS 不用 raw-window-handle，直接传 CAMetalLayer**）
- `get_scale_factor`（:63-67）：`contentScaleFactor`

### 1.4 `app-surface/src/touch.rs`（54 行，触摸事件模型）

- `StylusAngle<T>`（:3-9）：Apple Pencil 方位角/仰角
- `TouchPhase`（:11-17）：Started/Moved/Ended/Cancelled
- **`Touch`（:19-42）**：`{phase, position: glam::Vec2, stylus_angle, pressure, major_radius, interval}`——`#[repr(C)]` 可直接跨 FFI；`touch_start/move/end` 构造器

### 1.5 `app-surface/src/app_surface_use_winit.rs`（99 行，桌面/Web 实现）

- `AppSurface::new(view: Arc<Window>)` async：`view.scale_factor()` + `view.inner_size()`
- 后端选择（:62-69）：webgl feature → `Backends::GL`，否则 `Backends::PRIMARY`；**`Backends::from_env()` 覆盖**（WGPU_BACKEND 环境变量）
- `request_redraw()` / `pre_present_notify()`（:91-97）

### 1.6 `app-surface/src/unsupported.rs`（20 行）

- `compile_error!` 宏：桌面无 winit feature / web 无 winit+web_rwh feature 时编译期报错——**feature 门控的编译期强制**

### 1.7 `wgpu-in-app/src/lib.rs`（39 行）

- `init_logger()`（:16-38）：**Android：`android_logger::init_once` + `log_panics::init`**（:18-25）；桌面：`env_logger` + **`wgpu_hal`/`naga` 降到 Error 级**（:33-34，"very noisy"）——日志分级降噪

### 1.8 `wgpu-in-app/src/wgpu_canvas.rs`（57 行）

- `WgpuCanvas { app_surface, example: Box<dyn Example> }`——策略模式
- `new`（:11-24）：Empty 起始 → `change_example(idx)` → 回调 `callback_to_app(0)`
- `enter_frame`（:26-32）/ `resize`（:34-37）/ `change_example`（:39-41）——转发到 example
- `create_a_example`（:43-57）：idx → Boids/MSAALine/Cube/Water/Shadow/HDRImageView

### 1.9 `wgpu-in-app/src/ffi/android.rs`（40 行，JNI 导出）

**`#[jni_fn("name.jinleili.wgpu.RustBridge")]` 宏**（:11）：自动生成 `Java_name_jinleili_wgpu_RustBridge_<fn名>` 符号——不用手写 `#[no_mangle] pub extern "C" fn Java_...`。4 个导出：
- `createWgpuCanvas`（:12-19）：`WgpuCanvas::new(AppSurface::new(...))` → `Box::into_raw` → jlong
- `enterFrame`（:23-26）：`&mut *(obj as *mut WgpuCanvas)` 可变借用
- `changeExample`（:30-34）
- `dropWgpuCanvas`（:38-40）：`Box::from_raw` 释放

### 1.10 `wgpu-in-app/src/ffi/ios.rs`（~45 行）

- `create_wgpu_canvas(ios_obj: IOSViewObj) -> *mut c_void`（C ABI 结构体直接传入）
- `enter_frame` / `change_example`（`&mut *` 借用）

### 1.11 `wgpu-in-app/src/examples/`（7 文件）

**`mod.rs`（22 行）**：`Example` trait（`resize` + `enter_frame`）+ `Empty` 默认实现。
**`boids.rs`（~200 行）**：wgpu 官方 compute 示例移植——compute pipeline（粒子模拟）+ render pipeline（draw.wgsl），`NUM_PARTICLES=1500`、`PARTICLES_PER_GROUP=16`、双缓冲粒子 buffer（`particle_buffers: Vec<Buffer>` 长度 2，帧交替读写）。
**`cube.rs` / `water.rs` / `shadow.rs` / `hdr_image_view.rs` / `msaa_line.rs` / `point_gen.rs`**：渲染演示（未逐一精读——均为标准 wgpu 渲染管线，与终端渲染无关）。

### 1.12 Kotlin 宿主（4 文件）

**`RustBridge.kt`（12 行）**：`System.loadLibrary("wgpu_in_app")` + 4 个 external fun（`createWgpuCanvas(surface: Surface, idx: Int): Long` 等）。

**`WGPUSurfaceView.kt`（85 行，Android 渲染循环关键）**：
- `SurfaceView + SurfaceHolder.Callback2`
- init（:27-37）：`holder.addCallback(this)`、**`setZOrderMediaOverlay(true)`**（:35，让系统 UI 覆盖 SurfaceView 之上）+ `holder.setFormat(PixelFormat.TRANSPARENT)`（:36）——SurfaceView 透明 + Compose 可叠加
- **`surfaceCreated`（:43-49）**：`rustBrige.createWgpuCanvas(h.surface, idx)` 同步创建整个 wgpu 对象树 + `setWillNotDraw(false)`
- **`surfaceDestroyed`（:52-57）**：`dropWgpuCanvas(wgpuObj)` **整树销毁**，`wgpuObj = Long.MAX_VALUE` 哨兵
- **`onDraw`（:67-76）**：`enterFrame(wgpuObj)` + `invalidate()`——**渲染循环跑在 UI 线程，靠 invalidate 自驱动**（注释 :74-75）
- `changeExample`（:78-83）：idx 变化才调用

**`MainActivity.kt`（~70 行）**：Compose `AndroidView { WGPUSurfaceView(ctx) }` + `ToggleButton` 切换示例。`var surfaceView: WGPUSurfaceView?` 是**顶层全局变量**（非 Compose 状态，演示用）。

**`ToggleButton.kt`（~65 行）**：Compose 分段切换按钮（Material3）。

### 1.13 Swift 宿主（2 文件）

**`MetalView.swift`（~30 行）**：`layerClass = CAMetalLayer.self`；`layer.framebufferOnly = true`；**`contentScaleFactor = UIScreen.main.nativeScale`**（:28——注释链接 tomisacat 博客讲 nativeScale vs contentsScale 差异）。

**`ViewController.swift`（~90 行）**：**`CADisplayLink` 驱动渲染**（:18-20，iOS 的 vsync 回调）；前后台暂停/恢复 displayLink（:46-59）；`setupWGPUCanvasIfNeeded` 把 view/metal_layer/maximumFrames 打包 `ios_view_obj_t` 传给 Rust（:75-86）；`callback_to_swift` 状态回调（:93-104）。

### 1.14 构建工具与脚本

**`cargo-so/src/main.rs`**：`cargo so b` 子命令——用 `ndk_build` crate 自动检测 NDK 环境，交叉编译并打 APK（目标：Arm64V8a 默认）。替代手写 cargo ndk + gradle 步骤。
**`android_lib_build.sh`**：`cargo so b --lib --target aarch64-linux-android`（debug 带 `RUST_LOG=wgpu_hal=debug` 和 `RUST_BACKTRACE=full`）→ cp .so 到 `Android/app/libs/arm64-v8a/`。
**`Android/app/build.gradle.kts`**：`jniLibs.srcDir("libs")`（:22-24）；minSdk 28 / targetSdk 34；Compose BOM。

## 2. 与 torvox 功能对比（逐项）

| wgpu-in-app 功能 | 位置 | torvox 对应 | 对比结论 |
|---|---|---|---|
| NativeWindow RAII（fromSurface+release） | android.rs:50-96 | `native/src/render/context.rs`（round-12 已实现同款） | **torvox 已有**（`attach_surface`/`NativeWindow`） |
| 渲染循环在 UI 线程（onDraw+invalidate） | WGPUSurfaceView.kt:67-76 | `TerminalRuntime` 独立渲染线程 + notifyRender | **torvox 更优**（不阻塞 UI 线程） |
| surfaceCreated/Destroyed 整树创建/销毁 wgpu | WGPUSurfaceView.kt:43-57 | ADR-0007 惰性 attach（跨 surface 存活） | **torvox 更优**（surface 重建不重建 pipeline） |
| acquire 四分支（Timeout/Outdated/Lost 重配重试，Occluded 跳过） | lib.rs:222-237 | `pass.rs:132-181`（含 SwiftShader panic 兼容 + Mali-G57 检测） | **torvox 更完善** |
| Android view_formats = vec![format] 特例 | lib.rs:329-339 | 研究记录已标注 | torvox 已记录（context.rs 注释） |
| Android 硬编码 VULKAN | android.rs:20 | torvox 硬编码 GL（模拟器） | **场景不同**：wgpu-in-app 面向真机，torvox 面向模拟器兼容 |
| jni_fn 宏自动生成 JNI 符号 | ffi/android.rs:11 | torvox 手写 `#[no_mangle] Java_terminal_emulator_bridge_...` | **torvox 可吸收**（见 §4） |
| IASDQContext（可 Clone 的 Instance/Adapter/Device/Queue 打包） | lib.rs:152-174 | torvox Renderer 持有各对象 | 设计等价 |
| normalize 函数族 + 单测 | lib.rs:71-131 | torvox 无集中归一化工具 | **可吸收**（防御 0 尺寸） |
| PowerPreference::from_env 覆盖 | lib.rs:371-372 | torvox 固定 | **可吸收**（调试用） |
| wgpu_hal/naga 日志降噪 | lib.rs:33-34 | torvox logging | **可吸收** |
| setZOrderMediaOverlay + TRANSPARENT | WGPUSurfaceView.kt:35-36 | torvox TextureView | torvox 用 TextureView 更简单 |
| CADisplayLink vsync 驱动 | ViewController.swift:18-20 | torvox 渲染线程 + notifyRender | 设计等价 |
| iOS CAMetalLayer 直接 create_surface_unsafe | ios.rs:59-62 | 不适用（Android） | 知识备忘 |
| Touch 模型（repr C，含 stylus 角度/压力/半径） | touch.rs:19-42 | torvox 无 stylus 支持 | **可吸收**（未来手写笔） |
| cargo-so 构建工具 | cargo-so/ | torvox 用 cargo ndk + 脚本 | 等价（torvox 脚本已稳定） |
| `apply_limit_buckets: false` | lib.rs:375 | torvox 未用 | 细节（torvox limits 由 wgpu 默认） |
| WGPU_BACKEND 环境变量覆盖 | app_surface_use_winit.rs:68 | torvox 无 | **可吸收**（调试） |

## 3. 依赖分析

| 依赖 | 用途 | 适用 torvox？ |
|---|---|---|
| wgpu 30 | 渲染 | **已用**（同版本同代） |
| jni 0.21 | JNI | **已用**（同版本） |
| raw-window-handle 0.6 | surface 抽象 | **已用**（同版本） |
| **jni_fn**（crate） | JNI 符号名宏 | **适用**（减少手写错误，见 §4） |
| glam | 数学（触摸点 Vec2） | 不必要（torvox 无触摸数学需求） |
| futures-lite | block_on 同步创建 | 可替代（torvox 用 pollster 或自建） |
| android_logger / log_panics | Android 日志 | torvox 自研 logging.rs（等价） |
| ndk_sys | ANativeWindow | torvox 用 ndk crate（等价） |
| cargo-so / ndk_build | 构建 | 不必要（torvox cargo ndk 脚本稳定） |
| objc2 / objc2-core-foundation | iOS | 不适用（Android） |

**先进激进判断**：wgpu-in-app 依赖**不激进**（全部稳定生态），与 torvox 依赖零冲突。唯一建议新增：`jni_fn`（极小 crate，纯宏）。

## 4. 可吸收到 torvox 的具体内容

1. **`jni_fn` 宏**（P0）：`#[jni_fn("terminal.emulator.bridge.NativeBridge")] pub fn initSession(...)` 自动生成 `Java_terminal_emulator_bridge_NativeBridge_initSession` 符号。torvox `ffi.rs` 有 40+ 个手写 `#[no_mangle] pub extern "C" fn Java_...`——宏消除类名/函数名拼写错误风险。代码注释建议：
   ```rust
   // JNI 导出符号名由 jni_fn 宏生成（参考 wgpu-in-app wgpu-in-app/src/ffi/android.rs:11）
   // #[jni_fn("terminal.emulator.bridge.NativeBridge")] 展开为
   // #[no_mangle] pub extern "C" fn Java_terminal_emulator_bridge_NativeBridge_<fn名>
   ```
2. **acquire 四分支注释固化**（P0）：torvox `pass.rs` 已有实现，把 wgpu-in-app 的 Occluded→None 语义对照注释（lib.rs:222-237）写入。
3. **normalize_view_size 防御**（P1）：torvox 在 `resize` 前 clamp 尺寸 ≥1（避免 configure 0×0 panic）。参考 lib.rs:71-73。
4. **PowerPreference::from_env + WGPU_BACKEND**（P1）：调试时切换 GPU/后端。参考 lib.rs:371-372 + app_surface_use_winit.rs:68。
5. **wgpu_hal/naga 日志降噪**（P2）：torvox env_logger 配置加 `.filter_module("wgpu_hal", Error)`。参考 lib.rs:33-34。
6. **log_panics::init**（P2）：Rust panic 写 logcat（torvox 有 catch_unwind + JNI 异常抛出，可补充 log_panics 双保险）。参考 lib.rs:24。
7. **Touch/stylus 模型**（远期）：repr(C) Touch 结构体，未来手写笔支持。参考 touch.rs:19-42。

## 5. 项目文档吸收价值

- **README"为什么不用 winit"**（README.MD:1-28）：论证非游戏 App 不应被窗口库接管事件循环——torvox 架构文档可引用（torvox 同样不用 winit）
- README 的 Android 环境设置/构建流程：torvox 的 flake.nix 已覆盖（更优）
- 中文文档：https://jinleili.github.io/learn-wgpu-zh/integration-and-debugging/ —— wgpu 集成与调试的权威中文资料

## 6. 结论

wgpu-in-app 是"不用 winit 集成 wgpu"的样板，与 torvox 依赖栈完全同代（wgpu 30/jni 0.21/rwh 0.6）。torvox 在 surface 生命周期（ADR-0007 惰性 attach vs 整树销毁）、渲染线程（独立线程 vs UI 线程 invalidate）、acquire 处理（更完善）上**全面领先**。可吸收：`jni_fn` 宏（P0）、acquire 对照注释（P0）、normalize 防御（P1）、环境变量 GPU 覆盖（P1）、日志降噪（P2）、log_panics（P2）。

## deep-v1 增量（2026-08-07 全文件精读轮）

### 已精读文件清单（本次逐行读毕）
- `wgpu-in-app/src/lib.rs`（39 行）：WgpuCanvas 门面——`enter_frame`/`resize`/`change_example` 转发 + `Box<dyn Example>` 多态
- `wgpu-in-app/src/wgpu_canvas.rs`（58 行）：见上
- `wgpu-in-app/src/ffi/android.rs`（40 行）：**jni_fn 宏模式**——`#[jni_fn("name.jinleili.wgpu.RustBridge")]` 自动生成导出符号；**Box::into_raw/from_raw 生命周期管理**（createWgpuCanvas 返回 jlong 句柄，dropWgpuCanvas 回收）
- `wgpu-in-app/src/ffi/ios.rs`（40 行）：同构 iOS 版
- `app-surface/src/lib.rs`（491 行）：**IASDQContext**（Instance/Adapter/Surface/Device/Queue 全 Clone，wgpu v24+ 语义）；`create_current_frame_view` 的 acquire 五分支：Success/Suboptimal 直接取、Timeout/Outdated/Lost → **reconfigure 后重试一次**、Occluded → **返回 None**（不 panic）、Validation → panic；**view_formats 平台矩阵**（wasm 去 sRGB、webgl 空、Android 单格式、桌面双格式）——Android 不支持 SURFACE_VIEW_FORMATS downlevel 标志
- `app-surface/src/android.rs`（123 行）：`ANativeWindow_fromSurface`（引用计数 +1 注释）+ `Drop` 中 `ANativeWindow_release`；`unsafe impl Send/Sync for NativeWindow`；raw_window_handle 0.6 的 `AndroidNdkWindowHandle::new`/`AndroidDisplayHandle::new`
- `app-surface/src/touch.rs`（54 行）：repr(C) `Touch`/`TouchPhase`/`StylusAngle` 模型（pressure/major_radius/interval/stylus）——iOS 手写笔数据通道
- `app-surface/src/desktop.rs`（135 行）：winit 桌面集成（torvox 不需要）
- `app-surface/src/unsupported.rs`：非支持平台编译错误提示
- `cargo-so/src/main.rs`（77 行）：cargo-ndk 的 clap 包装（构建工具，torvox 用脚本等价）
- `Android/app/src/main/java/name/jinleili/wgpu/`（5 个 kt）：**WGPUSurfaceView**（SurfaceView + Callback2，surfaceCreated 建 wgpu 对象 / surfaceDestroyed 销毁 / **onDraw 中 enterFrame + invalidate() 自驱动连续渲染**——UI 线程渲染、无独立渲染线程、无 Choreographer）；RustBridge（4 个 external）；MainActivity/ToggleButton（示例切换）
- `Android/app/src/main/AndroidManifest.xml`：INTERNET 权限注释（GPU API tracing/metrics 需要）
- `wgpu-in-app/src/examples/*.rs`（8 文件 305-837 行）：boids/cube/hdr/msaa_line/point_gen/shadow/water 渲染技术示例——**与终端渲染无关**（阴影贴图/水波/粒子），仅证明 IASDQContext 可承载任意 wgpu 负载；`Example` trait = `new/resize/enter_frame` 三方法

### 新发现（相对 deep-v0 文档）

| # | 发现 | 级别 | torvox 对照 |
|---|------|------|------------|
| 1 | **Box::into_raw 句柄模式**：create 返回 `Box::into_raw(Box::new(canvas)) as jlong`，drop 用 `Box::from_raw`——**无全局注册表**，句柄即指针。torvox 用 SESSION_REGISTRY 全局 HashMap——**架构差异**：句柄模式免全局锁但要求调用方保证不 double-drop；torvox 全局注册表更强（防 double-free）但多一次锁。**结论：torvox 保留注册表**（多会话、JNI 多线程安全），wgpu-in-app 单对象模式不适用 | P3 | wgpu-in-app ffi/android.rs:12-19 |
| 2 | **onDraw+invalidate 自驱动渲染**：UI 线程连续渲染无 vsync。torvox 独立渲染线程 + 事件驱动（更优）——**但** wgpu-in-app 证明 SurfaceView 在 UI 线程渲染可行（torvox 渲染线程卡死时的 fallback 参考？不采纳——渲染线程是正确设计） | P3 | WGPUSurfaceView.kt:67-76 |
| 3 | **Android view_formats 限制**：Android 不支持 SURFACE_VIEW_FORMATS downlevel 标志 → view_formats 只能 `vec![format]`。**torvox context.rs 需检查**：若 torvox 设置了非空 view_formats 在 Android GL 后端可能 configure 失败（模拟器 GL 后端已验证通过，但 Vulkan 真机需核对） | **P1 需核对** | app-surface lib.rs:329-339 |
| 4 | **acquire 重试语义**：Timeout/Outdated/Lost → reconfigure+重试一次；Occluded → None。torvox pass.rs acquire 已有 worker 线程 + 重试——**对照确认 torvox 语义完整**，Occluded→None 需确认 torvox 处理（返回 Err 而非跳过帧？） | P2 | lib.rs:222-237 |
| 5 | **PowerPreference::from_env()**：环境变量切换 GPU 偏好——torvox 可加 `WGPU_POWER_PREF` 支持（调试） | P3 | lib.rs:371-372 |
| 6 | **normalize_view_size/scale_factor 防御**：所有尺寸入口 clamp ≥1、scale_factor 非法回退 1.0——torvox resize 前是否 clamp？ | P2 | lib.rs:71-81 |

### 依赖评估（deep-v1 确认）
- `jni_fn`（=0.1.x 宏 crate）：**适用，先进激进**——torvox ffi.rs 40+ 手写符号名，宏消除拼写错误（P0 候选，代码注释已建议）
- `futures-lite`（block_on）：torvox 用 tokio——不换
- `ndk-sys` vs torvox `ndk` crate：等价
- `glam`：wgpu-in-app 用于 3D 示例——torvox 渲染 2D 用自有 Vec4——不需要
- `log_panics`：P2（torvox 有 catch_unwind + JNI 异常，可补充）

### 文档吸收
- README.MD 的"为什么不用 winit"论证（非游戏 App 不应被窗口库接管事件循环）——torvox 架构文档可引用（torvox 同样不用 winit）

## deep-v2 增量（R31 全文件核对补漏）

### web_rwh/（wasm 平台，345 行——R1-R6 未覆盖，R31 补）
- `web_rwh/mod.rs`（142 行）：wasm 的 RawWindowHandle 实现（canvas → rwh 桥）
- `web_rwh/canvas.rs`（113 行）：Canvas 包装（scale_factor + physical_resolution）
- `web_rwh/offscreen_canvas.rs`（90 行）：OffscreenCanvasWrapper（SendSyncWrapper + OffscreenCanvas 离屏渲染）
- **torvox 对照：wasm 平台与 torvox Android 无关——仅补全研究完整性**（P3 记录：SendSyncWrapper 模式（wasm 对象包 Send/Sync）是通用模式，torvox 无需）
