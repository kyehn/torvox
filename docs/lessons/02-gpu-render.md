# GPU/Render 经验教训

## 1. Render Thread 生命周期管理

### 问题
Android Surface 的生命周期与 Render Thread 的生命周期未正确同步，导致三个问题：

**问题 1: ANativeWindow_setBuffersGeometry 缺失**
`set_native_window()` 没有调用 `ANativeWindow_setBuffersGeometry()`，导致首次渲染时 wgpu 的 Surface 没有缓冲区格式配置。后续 `update_native_window()` 虽然调用了此函数但首次初始化时缺失。

**问题 2: Surface 销毁后 Render Thread 继续运行**
导航到设置页面时，`surfaceDestroyed` 设置 `currentSurface=null`，但 render thread 继续在已死亡的 surface 上渲染。累积 300 个连续错误后（约 30 秒），线程永久退出。

**问题 3: 返回时 Render Thread 不会重启**
从设置页面返回时，`updateNativeWindow` 设置了新的 native window 指针，但已经死掉的 render thread 永远不会被重新启动，导致终端永久黑屏。

### 修复
1. `set_native_window` 中添加 `ANativeWindow_setBuffersGeometry` 调用
2. 添加 `pauseRendering()` 方法用于停止渲染线程并释放 GPU surface
3. `updateNativeWindow` 中检测并重启已死亡的渲染线程
4. `surfaceDestroyed` 生命周期回调中调用 `pauseRendering()`
5. 引入 `generation counter` 防止 stale render thread 干扰

### 教训
- Android SurfaceView/TextureView 的 `surfaceDestroyed` 信号必须同步停止 GPU 操作
- ANativeWindow 操作的两步：设置窗口指针 + 配置缓冲区几何
- Render Thread 应该有明确的启动/停止生命周期管理，而不是"等它自然死亡"
- 跨页面导航返回后需要检查渲染线程是否存活
- 使用 generation counter 跟踪渲染线程的"代"可以防止 stale 线程干扰

### 相关提交
- `a4f47aa2`: fix(android): set buffer geometry on init, fix surface lifecycle render thread death
- `fd9c2469`: Track render generation to stop stale threads
- `e5177f8f`: fix: resilient render thread handles transient errors gracefully
- `83e6d1cf`: Fix render thread shutdown via generation tracking
- `8ea94441`: fix: prevent stale render threads with generation counter

## 2. GPU Surface 未释放导致 VK_ERROR_NATIVE_WINDOW_IN_USE_KHR

### 问题
切换 Session 时没有释放当前的 GPU Surface，旧的 native window 句柄仍然被 wgpu 持有。当新 Session 尝试绑定到同一个 Android native window 时，Vulkan 驱动返回 `VK_ERROR_NATIVE_WINDOW_IN_USE_KHR` 错误，渲染失败。

### 根因
wgpu 在创建 Surface 时会获取 native window 的独占访问权。在释放 Surface 之前，同一 window 不能被绑定到第二个 wgpu 实例或 Surface。

### 修复
切换 Session 之前显式 drop/release 当前 GPU Surface。

### 教训
- wgpu Surface 绑定到 ANativeWindow 是独占的
- 任何 session 切换或 surface 重建前，必须确保旧 surface 已释放
- 使用 drop 语义（Rust RAII）确保释放，不要依赖 GC

### 相关提交
- `deda3178dc`: fix(native): release GPU surface when switching sessions to avoid VK_ERROR_NATIVE_WINDOW_IN_USE_KHR
