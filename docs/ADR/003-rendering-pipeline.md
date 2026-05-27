# ADR 003: 渲染管线 — wgpu v29 GPU 渲染 + cosmic-text/swash/skrifa 字体管线

**状态**: 已接受
**日期**: 2026-05-26
**决策者**: 项目负责人

---

## 上下文

渲染管线是终端模拟器中影响最大的架构决策。它决定：
- 帧率和延迟 (首要 UX 差异化因素)
- 字体质量 (连字, emoji, CJK, 可变字体)
- 电池寿命 (空闲 GPU 利用率)
- 内存占用 (字形图集管理)
- 平台可移植性

## 决策

**wgpu v29 原生 GPU 渲染，cosmic-text 0.19 → swash 0.2.7/skrifa 0.42 → guillotiere 字体管线，实例化四边形渲染。**

```
PTY 字节 → VT 解析器 → Grid 变更
↓
脏区域跟踪 (DirtyMask Vec<u64> 分区位标志)
↓
单元格 → 字形查找
↓
cosmic-text 0.19 (成形 + 布局) → swash 0.2.7 (缩放 via skrifa 0.42 + 光栅化 via zeno)
↓
guillotiere (图集打包) → wgpu 纹理上传
↓
实例缓冲区 (位置 + UV + 颜色 每单元格)
↓
wgpu v29 (单次绘制调用, 实例化四边形)
↓
帧缓冲 → Android Surface
```

## 理由

### 为什么选择 wgpu (WebGPU)？

| 后端 | 评估 |
|------|------|
| **OpenGL** | 遗留 API。不再接受驱动优化投资。复杂状态机。Mali 驱动有 SIGSEGV 问题。 |
| **Vulkan** | Linux/Android 最佳性能，但桌面平台锁定。 |
| **Metal** | macOS 最佳，但仅限 Apple。 |
| **DirectX** | 仅限 Windows。 |
| **wgpu v29** | ✅ 单一 API 原生目标 Metal/Vulkan/DX12。安全 Rust API。WebAssembly 目标。面向未来。 |

wgpu 是 2025-2026 几乎每个新终端模拟器的选择：WezTerm (迁移中)、ori-term、par-term、BeyondTTY、seance、Spectra、Ferrum、Basilisk、Rustty。生态系统已收敛于 wgpu 作为现代标准。

### wgpu 29 关键变更

wgpu 29 Surface 创建 API 变更：
1. `InstanceDescriptor.display` 字段为 `Option<Box<dyn WgpuHasDisplayHandle>>` — **Vulkan 后端不使用 DisplayHandle**
2. `SurfaceTarget` 两种变体: `DisplayAndWindow` (通用) 或 `Window` (仅 WindowHandle, 如果 display 已传给 Instance)
3. Android/Vulkan: `InstanceDescriptor::new_without_display_handle()` + `SurfaceTarget::Window`

这对 Android 集成影响：
```rust
// wgpu 29 — Android/Vulkan 模式
let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
    backends: wgpu::Backends::VULKAN,
    ..Default::default() // display = None
});

let surface = unsafe {
    instance.create_surface_unsafe(
        wgpu::SurfaceTargetUnsafe::RawHandle {
            raw_window_handle: android_raw_window_handle,
            raw_display_handle: Some(raw_display_handle), // Vulkan 后端不使用
        }
    )?
};
```

### Android 上 wgpu 的现状

| 关注点 | 状态 |
|--------|------|
| Vulkan 后端 | ✅ 成熟。Android 10+ (API 29) 原生支持 Vulkan 1.1。 |
| Surface 渲染 | ✅ 通过 `create_surface_unsafe(RawHandle{...})` 支持 Android `Surface`。 |
| 着色器编译 | ✅ wgpu 在 Android 上使用 Vulkan SPIR-V，naga 运行时编译。首次帧有编译延迟，可通过预热缓解。 |
| 多后端 | ❌ 不需要。Vulkan-only 是 Android 上的正确选择。 |

### Android 渲染路径

**SurfaceView 方案**: Rust 持有 wgpu Surface，直接渲染到 Android `SurfaceView`。Kotlin/Compose 管理输入事件和 UI 覆盖层 (修饰键栏、设置)，但终端内容本身完全由 Rust 渲染。

**为什么 SurfaceView 优于 AHardwareBuffer**:

| 因素 | AHardwareBuffer | SurfaceView |
|------|----------------|-------------|
| 延迟 | 需要缓冲区复制到 Compose | 零复制，GPU 直接呈现 |
| 实现复杂度 | 高: 需要自定义共享内存路径 | 低: wgpu 原生支持 Surface |
| 与 wgpu 兼容性 | 需要自定义集成 | ✅ 原生支持 |
| 输入处理 | Compose 正常处理 | SurfaceView 接收输入事件 |
| 组合层 | Compose 可以叠加 | 修饰键栏在 SurfaceView 之上 |
| 已验证实践 | 无已知终端使用 | 游戏引擎标准模式 |

这与移动游戏引擎使用的模式相同：Rust/wgpu 直接拥有渲染 Surface，Android UI 层叠加半透明控制。

### 为什么 cosmic-text + swash + skrifa？

这是驱动 COSMIC 桌面环境的纯 Rust 字体管线。

**角色分工**:
- **cosmic-text 0.19**: 文本成形 (rustybuzz/HarfBuzz), 布局, 字体回退, BIDI
- **swash 0.2.7**: 缩放 (via 内部 skrifa 依赖) + 字形光栅化 (via zeno), CBDT/COLR 彩色 emoji。0.2.x 版本的 `scale` feature 自动引入 skrifa，不再需要单独依赖 skrifa crate。
- **skrifa 0.42**: Google 字体缩放库。swash 0.2.x 已内部依赖，直接使用 swash 的 `scale` feature 即可。COLRv1 由 skrifa 原生支持。

**swash 0.2.x 架构变更**: swash 0.2.x 已将缩放功能内部迁移到 skrifa（`scale` feature 依赖 `skrifa` crate），光栅化使用 `zeno` crate。不再需要单独在 Cargo.toml 中添加 skrifa 依赖——swash 的 `scale` feature 会自动引入。这是从 0.1.x 的重大变更：0.1.x 的缩放由 swash 自身实现，0.2.x 完全委托给 skrifa。

备选方案评估:

| 选项 | 问题 |
|------|------|
| **Android Typeface + Canvas drawText** | CPU 逐单元格, 无 GPU 图集, 无连字控制。Termux 的瓶颈。 |
| **ab_glyph** | 无成形, 无连字, 无 emoji |
| **fontdue** | 无成形, 仅 CPU 光栅化 |
| **skia-safe** | C++ Skia 绑定, 复杂构建, 增加 200MB+ |
| **DirectFreeType** | C 依赖, Android 交叉编译痛苦 |
| **glifo 0.1.0** | Linebender/Vello 新项目, 早期开发, 未独立发布到 crates.io |
| **libvterm + Canvas** | Haven 模式: C→Kotlin 复制 + CPU Canvas。被我们拒绝。 |

### 为什么选择实例化四边形？

逐字符绘制纹理四边形需要每帧数千次 GPU 绘制调用。**实例化渲染** 将所有可见单元格打包到单个顶点缓冲区，带逐实例数据 (位置, UV 坐标, 前景色, 背景色, 下划线样式)。整个可见终端在**一次或两次绘制调用**中渲染。

这是 Alacritty、Warp、Kitty、WezTerm 和每个 GPU 加速终端模拟器使用的标准方法。Warp 明确记录这是实现 `find /` 下 60FPS 的关键优化。

### 脏区域优化

不每帧重新渲染所有单元格：
- 维护 `DirtyMask(Vec<u64>)` 分区位标志 (每 u64 覆盖 64 行)
- 仅处理标记为脏的行
- 将连续脏行批处理为脏矩形
- 仅上传变更的图集区域到 GPU

这是所有 GPU 终端的标准。Alacritty 使用它。Ghostty 使用它。Warp 的块模型是变体。

**与现有项目对比**:

| 项目 | 脏区域跟踪 | 重绘策略 |
|------|------------|----------|
| Termux | 无 | 每帧全屏重绘 |
| Haven | Compose 管理 | Compose 重组 |
| ConnectBot termlib | 无 | 每帧全屏重绘 |
| Alacritty | DirtyLine bitmask | 仅重绘脏行 |
| WezTerm | 损伤跟踪 | 仅重绘变更 |
| **Torvox** | **DirtyMask(Vec<u64>) + 实例缓冲区 diff** | **仅重绘脏行, 仅上传变更图集** |

## 后果

**正面**:
- 近零空闲功耗 (终端空闲时无 CPU/GPU 工作)
- 重输出下稳定 60-120 FPS
- 最高质量字体渲染 (连字, 可变字体, emoji, CJK)
- 单一 wgpu 代码库跨平台渲染
- 通向 GPU 计算着色器渲染的未来路径 (Ferrum 模式)

**负面**:
- 需要 Vulkan 1.1 支持 (Android 10+ = API 29+，覆盖 95%+ 设备)
- 首帧着色器编译卡顿 (通过会话启动时预热缓解)
- SurfaceView 输入处理需要额外代码 (通过 Kotlin 事件转发解决)
- 字体管线冷启动延迟 (通过初始化时预光栅化常见 ASCII 字形缓解)
- swash 0.2.x 已内部集成 skrifa (缩放迁移完成，不再需要单独管理两个 crate 的缩放边界)

## 开放问题

1. **MSDF vs 位图字形图集**: 初始使用位图图集 (简单可靠)。阶段 3+ 评估 MSDF — 条件: 着色器复杂度可接受 + 字形质量提升可测量。
2. **像素级平滑滚动**: MVP 使用逐行滚动。阶段 4+ 评估像素级 — 条件: 渲染器架构稳定 + 性能预算允许重架构。
3. **GPU 计算着色器网格**: 不在路线图中 (阶段 5 之前不考虑)。仅当单次绘制调用实例化成为瓶颈时评估。
4. **glifo 替代 swash**: 当 glifo 发布 1.0 稳定版时重新评估。swash 0.2.x 长期依赖稳定。
