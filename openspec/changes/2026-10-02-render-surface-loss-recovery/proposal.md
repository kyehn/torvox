# 渲染 Surface 丢失后自愈

## Why

`:app:connectedDebugAndroidTest` 全量套件实测（release profile native 库，Android 15
x86_64 模拟器 emulator-5554，161 例 / 27 失败 / 0 跳过 / 22m12s）里，A 类 8 例
（`diag/CursorPixelAcceptanceTest`、`diag/SgrColorPixelAcceptanceTest`、
`VisualInlineVerificationTest`、`SelectionEspressoTest`、
`ImePopupPixelInstrumentedTest#contentManyImePopupMovesUpBottomIdentical`）全部退化为
「零像素 / 零手柄 / 位移=0」。同一次运行 logcat 给出唯一根因链（app 进程 pid 5303）：

```text
attach_surface: configured                                          ← 全程仅 1 次
attach_surface: RECONFIGURE_SWAPCHAIN (fast path, existing surface)  ← 132 次
E BufferQueueProducer: SurfaceView[…]#1(BLAST Consumer)1 query: BufferQueue has been abandoned
E vulkan: NATIVE_WINDOW_MIN_UNDEQUEUED_BUFFERS query failed: No such device (-19)
E wgpu_hal::vulkan::swapchain::native: get_physical_device_surface_capabilities: ERROR_SURFACE_LOST_KHR
E native::render::context: GPU_UNCAPTURED_ERROR: Validation { … "In Surface::configure"
      Caused by: Surface does not support the adapter's queue family }
E native::android::ffi: render: frame failed: surface creation failed: begin_frame failed
W Runtime: SLOW_FRAME session=1 render=39.659880 count=-1 newOutput=false scrollOffset=0
```

`abandoned=6148`、`Surface::configure` 错误 `=6148`、`ERROR_SURFACE_LOST_KHR=3074`、
`begin_frame failed=3074`：**BufferQueue 在套件开始后不久被遗弃，此后 22 分钟里每一帧都
失败**（`count=-1`），终端恒为空白。真机上 SurfaceFlinger 重启、屏幕热插拔同样会造成该状态
（永久黑屏，直到杀进程）。

## What Changes

- **原生：区分「surface 级失败」与「本帧跳过」**。`acquire_texture` 改返回
  `AcquireOutcome::{Acquired, Skipped, SurfaceLost}`：工作线程忙/超时属 `Skipped`（慢机器的
  连续超时 MUST NOT 判死 surface），`Lost`/`Outdated` 属 `SurfaceLost`。
- **原生：连续失败即失效**。`Renderer` 记 `surface_loss_streak`，连续
  `SURFACE_LOSS_STREAK_LIMIT = 2` 次 surface 级失败即置 `surface_invalidated`（幂等，
  只记一条 error）。判定是纯函数 `surface_loss_transition`，可在无 GPU 的单测里覆盖。
- **原生：失效后重建而非 reconfigure**。`attach_surface` 快路径条件收紧为
  `surface.is_some() && surface_config.is_some() && !surface_invalidated`；重建成功后失效位
  与计数清零，`release_surface` 同样清零。
- **原生：零额外 JNI 的上报**。`renderWithNewOutput` 的打包返回重排为 位 0..31 渲染计数、
  32 `new_output`、33..42 光标行、43..52 内容下沿、**53 surface 失效位**（两个行字段由
  16/15 位收窄为各 10 位——Android 网格行数 ≤227，超出按哨兵处理，两种退化方向都保守）。
  失效位不挂在渲染计数门下：空闲帧 `render_inner` 返回 0（无新单元数据、无需呈现），
  门控会把它永远压在 0。
- **宿主：换掉整个 SurfaceView**。`TerminalRuntime` 读失效位、按间隔（500ms）与次数上限
  （5）裁决后递增 `surfaceRecreateRequests`（纯函数 `decideSurfaceRecreate`）；`TerminalScreen`
  以 `key(计数)` 包裹终端 Surface 容器，变更即拆旧视图（`surfaceDestroyed` →
  `releaseAllGpuSurfaces`）、建新视图，新 `surfaceCreated` 交付**新的** `ANativeWindow`，
  `attach_surface` 随之走重建慢路径。
- **Kotlin 侧复用既有 `postDelayedSurfaceRecreate`** 的判定口径（holder 有效性 + 尺寸），
  但**不**复用它的「对同一窗口反复 detach/attach」做法：唤不活被遗弃的 BufferQueue。

## Non-goals

- 不改用应用自建 `SurfaceTexture`（彻底方案，但涉及渲染架构与 FFI 形态变更，单独立项）。
- 不在帧内重建 surface（与渲染线程竞争会触发 `ERROR_NATIVE_WINDOW_IN_USE_KHR`，
  见 `context.rs` 注释）。
- 不改暂停语义：IME 收起过渡的 `render paused` 不置失效位，否则会触发重建风暴。

## Impact

- 受影响规格：`render-stability`（新增「surface 失效自愈」需求）。
- 27 个失败中 A 类 8 例为直接修复对象；B/C/E 类是否同源需渲染恢复后复测确认
  （design.md 第 4 节），结论回写该表。
- 像素族断言此前可能在「一片空白」上取样而恒真；恢复后才成为真实判据。