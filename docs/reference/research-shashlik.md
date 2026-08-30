# 深度研究：shashlik-map

> 研究日期：2026-08-06 | 克隆位置：`repositories/refs/shashlik`（depth 1）
> 语言：Rust + Kotlin；MIT
> 定位：Android 上的 Slint UI 渲染框架（wgpu 后端）。核心价值在 **wgpu Android 双后端选择（GL/Vulkan）** 与 **NativeWindow 封装**

## 1. wgpu Android 初始化（`app-surface/src/android.rs`）

```rust
pub async fn new(env: *mut JNIEnv<'_>, surface: jobject, is_emulator: bool) -> Self {
    let native_window = Arc::new(NativeWindow::new(env, surface));
    // 模拟器用 GL，真机用 Vulkan！
    let init_backend = if is_emulator { wgpu::Backends::GL } else { wgpu::Backends::VULKAN };
    let mut descriptor = wgpu::InstanceDescriptor::new_with_display_handle(Box::new(native_window.clone()));
    let mut instance = wgpu::Instance::new(descriptor);
    // https://github.com/gfx-rs/wgpu/issues/2384
    // GL 后端兜底重试
    if instance.request_adapter(...).is_err() {
        descriptor.backends = wgpu::Backends::GL;
        instance = wgpu::Instance::new(descriptor);
    }
}
```

**关键验证**：本项目（torvox）在模拟器上用 `wgpu::Backends::GL`（SwiftShader Vulkan swapchain 失败）的做法与 shashlik 完全一致——这是 wgpu Android 生态的公认模式。`wgpu#2384` 链接记录了 GL 后端的已知问题。

**对比 torvox**：
- torvox `wgpu_backend.rs:22-25` Android 强制 VULKAN（之前研究记录），后来模拟器验证发现需 GL —— shashlik 的 `is_emulator` 参数化 + GL 兜底重试是更稳健的模式，**建议 torvox 采用**（JNI 传入 is_emulator 或先尝试 Vulkan 失败后回退 GL）
- `NativeWindow::new(env, surface)`：封装 ANativeWindow 生命周期（`Arc<NativeWindow>` 传给 wgpu display handle）——与 torvox 的 `attachWindow`/`NativeWindow` RAII 包装思路一致

## 2. 其他模块

- `renderer-cpu/`：CPU 渲染器（模拟器 fallback）
- `renderer-common/`：worker_handler、fps、collision（Slint 场景）
- `kmp/`、`ffi-run/`：Kotlin Multiplatform 绑定

## 3. 结论

shashlik 最有价值的单点是 **wgpu Android 的 is_emulator 双后端 + GL 兜底重试**（`app-surface/src/android.rs:25-37`）。本项目应将其吸收进 `wgpu_backend.rs` 的后端选择逻辑。
