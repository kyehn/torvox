# 项目状态

本文件此前首行写着「任何情况下禁止对本文件进行任何修改」。该约束已被 2026-11-13 的用户
指令取代：指令明确要求修正本文件与实际状态不符之处。该行删除；本文件恢复为可按需更新的
状态清单。与代码不符的内容以 `docs/specification/` 与 `openspec/specs/` 为准。

基线：本清单核对截至 2026-11-14 的 main（`89332b26`）。

## 未被解决的问题

1. **输入法弹出时切换后台返回终端大概率闪烁**

   修复已落地（`226ef094`）：后台期间冻结 IME insets 位移；仪器化用例
   `imeShiftFrozenWhileBackgrounded` 无修复时 225→0 必失败、修复后保持 225（模拟器通过）。
   Android 13/15 真机复验待做（模拟器切后台不拆 Surface，端到端遮罩/原生重建路径只在真机
   出现），复验前仍按未解决计。

2. **后台切换返回概率黑屏，添加新会话依旧黑屏**

   根因是 Mali Vulkan 设备被判死后应用仍逐帧调用死设备：实测 2026-11-14 日志 206 次
   `get_current_texture` panic、1 次 `Device::poll` panic、207 条 panic 钩子告警、7 次
   遮罩回读 `map_async` 失败，而宿主换的新窗口仍挂在同一死设备上，终端永久黑屏；新建
   会话共享 `GlobalGpu`，同样起不来。

   修复已落地（本轮）：
   - `aec01f76`：wgpu 设备丢失回调 + panic 载荷文字判据（`Parent device is lost`）→ 进程级
     `DEVICE_LOST` 原子位；丢失后 `render_inner` 在 GPU 调用前返回、`read_cover_frame`
     返回 `None`（遮罩退化平底色）、失效位把 `device_lost` 合并上报；首次记一条 error。
   - `3d68f784`：宿主换新 `SurfaceView` 时 `attachWindow` 重建 instance/adapter/device 与
     `Renderer`（搬运暂停位/背景色/滚动偏移），成功才清丢失位，失败继续上报由重建预算换窗口。
   - `ab9e1ac0`：`Surface::configure` 失败（`GpuWaitTimeout`）导致的「surface 未配置」
     按 Lost/Outdated 同源自动重配，根除永久 panic 死锁。
   - `98acc0f3`：「设备丢失恢复后新会话必须照常出帧」回归用例。
   - 仪器化回归：`SurfaceLossRecoveryInstrumentedTest` 全量 7/7 通过（模拟器注入 + 新会话用例）。

   模拟器注入模拟的是应用侧自愈链路；**Mali 真机复验待做**（设备丢失的真实时机、恢复节奏、
   首帧发布时点），复验前仍按未解决计。

## 已归因、非应用缺陷的日志噪声（如实保留，不掩盖）

| 条目 | 归因 | 处置 |
|---|---|---|
| `E/mali_gralloc: Requested R8 format is not supported ...` | AOSP libvulkan 枚举表面格式时对 R8 无条件探测 `AHardwareBuffer_isSupported`（`GetPhysicalDeviceSurfaceFormatsKHR`），与应用请求的 `Rgba8Unorm` 无关 | 平台侧，应用无开关；减少表面重建次数是应用侧唯一可做的 |
| `E/mali_gralloc: Unrecognized and/or unsupported format ...` | 同上 | 同上 |
| `W/Parcel: Expecting binder but got null!` | AOSP `Parcel.cpp` `readStrongBinder` 返回空 | 平台侧，应用无可修复动作 |
| `W/FeatureOpt: ensureGetRight ... key=zte_foldable` | ZTE/Unisoc 厂商 FeatureOpt 服务（折叠屏能力探测） | 平台侧 |
| `W/libc: Access denied finding property "persist.unipnp.debug" / "vendor.gpu.fbc" / "vendor.MB.running"` | Unisoc/ZTE 平台 sepolicy 拒绝读取厂商私有属性 | 平台侧 |
| `W/ziparchive: Unable to open '/system_ext/framework/unipnp-framework.dm' (等 unisoc-framework.dm)` | 厂商 OTA dexopt metadata 文件不存在 | 平台侧 |
| kernel `audit(...)` avc 行（`fish`/`git`/`login`/`DefaultDispatch`/`chroot-ng`，`granted { execute }` 与 `/proc` 拒绝） | 内核 SELinux 审计记录，guest 环境进程；`granted` 是审计而非失败 | 平台/SELinux 侧 |
| `E/com.termux: Not starting debugger since process cannot load the jdwp agent` | debug 构建下 ART 对 jdwp 附着的提示（`am start` 未带 `-D`），仅 debug 发生 | debug-build 提示，非故障 |
| `W/ActivityThread: Package uses different ABI(s)` | 仪器化测试包（无 ABI）与应用包（x86_64）ABI 标示差异提示 | 平台信息性提示 |
| `W/com.termux: Unexpected CPU variant for x86_64: x86_64_v3` | ART/cpu variant 探测不认识该型号，回退默认 | 平台信息性告警 |
| `W/com.termux: ClassLoaderContext shared library size mismatch` | ART 对 interposer 共享库（libnative.so 等）的上下文提示 | 平台信息性提示 |
| `W/wgpu_core::instance: Missing downlevel flags: DownlevelFlags(SURFACE_VIEW_FORMATS)` | wgpu-core 能力汇报；本项目对 surface `view_formats` 有意置空（见 `context/renderer.rs` 注释） | 平台库信息性提示 |
| `W/com.termux: Cleared Reference was only reachable from finalizer` | AOSP ART `reference_queue.cc`；应用不创建软/弱/幻引用 | 平台 GC 信息性告警 |
| `W/wgpu_core::instance: DownlevelCapabilities` | wgpu-core 启动期能力汇报 | 平台库信息性告警 |
| `E/Typeface: updateDefaultTypeface(...)` | 平台 Typeface 解析 RRO 字体路径（`/data/resource-cache/font/...`），非应用字体 | 平台侧 |
| `W/com.termux: Accessing hidden method ... WindowManagerGlobal ...` | `debugImplementation com.ms-square:debugoverlay` → radiography → curtains 反射隐藏 API，仅 debug 构建存在 | debug-only 依赖的预期行为，release 不含 |
| `W/SecondStageRunner: coreutils postinst ... Segmentation fault` | Termux bootstrap x86_64 二进制在本 AVD 镜像下崩溃（全新 AVD 首启 6 次），非应用代码 | bootstrap 环境噪声 |
| `E/FrameTracker: force finish cuj, time out J<IME_INSETS_...>` | 平台 FrameTracker CUJ 看门狗 | 平台侧 |

## 使用习惯

- 渲染/设备丢失这类深路径：模拟器用例证明应用侧链路，真机（Mali / Android 13）节奏由用户
  复验后才能收口；复验后把结果回填到本文件并注明复验的设备与日期。
- 每一项缺陷修复独立 commit 且 push；日志等级/过滤/tag 不得用于掩盖。
