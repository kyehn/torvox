# 深度研究：shashlik 补充 — 亲自逐文件阅读

> 研究日期：2026-08-06 | 项目链接：https://github.com/ShashlikMap/shashlik-map
> 前置：`research-shashlik.md`（android.rs 双后端，亲自读）；本文补充 **fps.rs 全 + app-surface/src/lib.rs 前 120 行 + renderer-cpu 结构**

## 1. fps.rs（通用 FPS 计数器，完整阅读）

```rust
pub struct Fifo<const N: usize, T> { items: [T; N], index: usize }  // 环形固定队列
impl Fifo { pub fn push(&mut self, item: T) -> T { ... } }          // 返回被替换的旧项

pub struct FpsCounter<const N: usize> { last: Instant, samples: Fifo<N, f64>, running_sum: f64 }
impl FpsCounter {
    pub fn update(&mut self) -> f64 {
        let now = Instant::now();
        let new_sample = (now - self.last).as_secs_f64();
        let prev_sample = self.samples.push(new_sample);
        self.running_sum -= prev_sample;   // 滑动窗口：减旧加新
        self.running_sum += new_sample;
        self.last = now;
        1.0 / (self.running_sum / N as f64)  // 帧间隔均值倒数
    }
}
```

**O(1) 滑动窗口 FPS**——`Fifo<const N>` + `running_sum`（减被替换项加新项），N=帧采样数（小=灵敏，大=稳定）。**torvox 可借鉴**：性能测试/渲染监控的轻量 FPS 计数（当前 torvox 用 Instant 手算？）。

## 2. app-surface（确认 fork 关系）

shashlik 的 app-surface 是 **wgpu-in-app 的直接 fork**（IASDQContext/SurfaceFrame/ViewSize 结构相同），差异：
- **acquire 简化**：`Success → frame`，`_ → panic!`（无 Timeout/Outdated/Lost/Occluded 分支）——比 wgpu-in-app 的完整四分支更粗糙
- `update_config_format` 的 view_formats 直接 `[srgb, non-srgb]` 双格式（无 Android 特例）——比 wgpu-in-app 的 `vec![format]` 特例粗糙
- android.rs 的 is_emulator 双后端（前研究已确认）

**结论**：shashlik 的 app-surface 是退化的 wgpu-in-app 副本，**无新增参考价值**（wgpu-in-app 的完整版已在亲自研究中覆盖）。

## 3. renderer-cpu（621 行，结构扫描）

skia_safe + lyon_algorithms + rstar 的 CPU 几何渲染器（Slint 场景专用）：
- `CpuRenderer`（:28-52）：skia Canvas + WorkerHandler 文本线程 + shapes 前后景分层
- `FontData`（:54-75）：skia FontMgr 加载 + antialias
- `CanvasApi` 实现（:132-168）
- 文本处理（:259-291）/ 形状处理（:292-420）

**与终端无关**（Slint UI 场景渲染），无 torvox 移植价值。

## 4. 结论

shashlik 补充阅读确认：**fps.rs 的 O(1) 滑动窗口 FPS 计数器可借鉴**（torvox 性能测试用），其余（app-surface fork、CPU 渲染器）无新增价值。

## deep-v1 增量（2026-08-07：WGPUTextureView 精读）

### kmp/shared/src/androidMain/.../WGPUTextureView.kt（完整 76 行）
- **TextureView + SurfaceTexture → wgpu 方案**：`onSurfaceTextureAvailable` 创建 `Surface(st)` → JNI `createShashlikMapApi(surface, isEmulator, tilesDb, dpiScale)` → uniffi 包装 → `resize()` + `render()`
- **`onSurfaceTextureUpdated` 里调用 `render()`**——每次 SurfaceTexture 更新触发渲染（TextureView 特有回调）
- isEmulator 检测：`Build.FINGERPRINT.contains("generic") || contains("sdk_gphone")`——**模拟器后端降级开关**（torvox 模拟器 GL/Vulkan 切换可参考此检测，P3）
- **torvox 对照**：torvox TerminalSurface 曾 SurfaceView↔TextureView 往返（历史）。shashlik 证明 **TextureView + onSurfaceTextureUpdated 渲染驱动可行**（每次更新渲染 vs torvox 独立渲染线程事件驱动）——torvox 渲染线程设计仍更优（不依赖 UI 线程回调），但 isEmulator 检测值得记录

### 新增汇总
| # | 发现 | 级别 |
|---|------|------|
| 1 | Build.FINGERPRINT 模拟器检测（generic/sdk_gphone）——torvox 后端选择可参考 | P3 |
| 2 | TextureView onSurfaceTextureUpdated 渲染驱动 vs torvox 渲染线程 | 确认 torvox 更优 |
