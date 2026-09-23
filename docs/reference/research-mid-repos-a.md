# 中期仓库研究（A 组）：osmosis / redterm / terminator / termx

> 研究日期：2026-08-06　｜　研究对象：4 个参考仓库（逐一完整阅读源码）
> 对比基准：**torvox**（Android 终端，Kotlin Compose + Rust native + wgpu + libghostty-vt）
> 本文档与 `research-mid-repos-b.md`、`research-small-repos.md` 属同一系列，聚焦"中型仓库"。

---

## 0. torvox 功能基线（对比用）

研究前先固化 torvox 已具备的能力，避免在"该功能 torvox 有没有"上误判。依据：`docs/architecture.md`、`docs/tech-stack.md`、`docs/adr/` 及 `android/app/src/main/java/terminal/emulator/` 源码清单。

**渲染/终端核心**：wgpu 30（Vulkan-only）GPU 渲染；cosmic-text 0.19 整形 + swash 0.2 栅格化 + guillotiere 0.7 图集打包；libghostty-vt（vendored Zig）完整 VT 解析；CellData（80B bytemuck Pod）经 flume 管道到 RenderThread；TextureView + ANativeWindow 表面；`GridSnapshot` 仅走命令路径（选择/滚动/OSC）。单 crate `native/`（terminal/render/android/mcp），14 个 JNI 函数，内嵌 tower-mcp（7 个 MCP 工具），4+1 线程模型（PTY Reader 兼 VT 解析 / Input Writer / Process Waiter / Render Thread + MCP Listener）。

**Kotlin 侧已有功能**：`ui/TextSearchBar.kt` + `SearchResult.kt`（文本搜索）、`ui/ModifierBar.kt`（修饰键栏）、`ui/SessionDrawer.kt`（会话抽屉）、`service/TerminalForegroundService.kt`（前台服务）、`installer/Bootstrap*`（Termux bootstrap 安装）、`runtime/PasteChunker.kt`、`ui/UrlDetector.kt`、`ui/TerminalNotificationHelper.kt`、`settings/SettingsRepository.kt`（DataStore）、`monitor/`（AnrWatchDog/BootGuard/MemoryMonitor/RenderWatchDog/ThermalMonitor）、`bridge/NativeQueryPort.kt`、`input/KeyModifiers.kt` 等。

**torvox 没有**（本组 4 仓库的对照点）：proot/发行版容器、X11/VNC 服务器、SSH 服务器、Cron、宏系统、HTTP 服务器、网络/系统 API 命令集、3D 渲染、音频播放、WebRTC、跨端（仅 Android）、指纹锁、桌面小组件/快捷磁贴、VT 解析器之外的备份渲染路径（GPU-only 是刻意决策，ADR-0008）。

---

# 1. osmosis（yebei199/osmosis）— Slint 跨端应用骨架

## 1.1 项目定位

一个**以 Rust 单一代码库驱动 Android/desktop/iOS/Web 四端**的 Slint 应用骨架：音乐播放器（跨端音频）+ WebRTC 实时同播（syncplay）+ Bevy 3D 可视化（cloud/nav-glass/warp 三种渲染 pass）。它的价值不在于"音乐 App"，而在于**把跨端分层、wgpu 设备共享、Slint+Bevy 双渲染器共存、依赖边界自动化、ADR 决策文化**完整示范出来——这正是它被选入研究的原因。

- 仓库规模：workspace 含 `apps/*`（4 端入口）+ `crates/*`（contract/api/app-core/ui/audio/render3d/syncplay）+ `server/`（axum+tonic 后端）+ `xtask/`（构建/边界检查）+ `docs/adr/`（12 个 ADR）。
- 技术栈：Slint 1.17/1.18-dev（fork + Android MCP patch）、Bevy 0.16、wgpu-29（与 Slint/Bevy 共享 device）、rodio+symphonia（音频）、webrtc-rs（同播）、axum 0.8 + tonic（后端）、android-activity 0.6、wasm-bindgen（Web）。
- 文档：12 个 ADR + `docs/note/`（架构方向笔记）+ 自写 blog；CI 用 xtask 强制 crate 依赖边界（`cargo tree` 断言）。

## 1.2 完整架构

依赖方向（自底向上，ADR-0003 定案，`xtask/boundaries.rs` 用 cargo tree 在 CI 里强制）：

```
apps/{android,desktop,ios,web}   ← 每端一个薄入口 crate（只做平台初始化 + 调 ui::run）
        ↑
crates/ui                          ← 组装点：Slint 编译（build.rs）、全部绑定、渲染器注册
        ↑
crates/app-core                    ← 领域逻辑（无平台依赖、无 async）：HealthTracker/PlaybackState/Queue/Lyric/Counter
        ↑
crates/contract                    ← 只共享线上 DTO（health/track/device/lyric/playback 命令）
        ↑
crates/api                         ← 网络边界：Send 关在 api 内部（ADR-0002），wasm/native 双实现
        ↑
平台能力层（被 ui 按需依赖）：
  crates/audio      — rodio 播放 + symphonia 解码 + FFT 频谱（visualizer）
  crates/render3d   — Bevy 场景 + 独立 wgpu pass（nav-glass/warp），wgpu device 全端共享基座（ADR-0005）
  crates/syncplay   — WebRTC 同播：signalling → peer → pump(PCM) → session(roster)
        ↑
server/（独立部署）  — axum：BangDream 搜索/每日代理（tonic gRPC）+ SSE 信令 + roster
```

关键机制：

1. **wgpu 设备共享（ADR-0005，核心看点）**：`render3d::Scene::new` 创建唯一 wgpu `Device`/`Queue`，通过 `Arc` 同时喂给 Bevy（`WgpuSettings { device, queue, .. }`）与 Slint 渲染器；Slint 侧用外部纹理（`slint::platform::WindowAdapter` 的 texture 通道）把 Bevy 渲染结果贴进 UI 的 `image` 元素；`extract_texture`（读回 framebuffer 像素）后做"动态分辨率纹理重建"（虚拟分辨率→实际像素，512→768→1024 逐档自适应），nav-glass 的侧栏选中器与 warp 视觉各是一个独立 wgpu render pass，绕开 Bevy 的 ECS 开销。Bevy 是**硬依赖**而非 feature（ADR-0011），避免 feature 矩阵爆炸。
2. **双相机遮挡**：主相机渲染 viz 内容，第二台 occluder 相机（frustum 裁剪相机）只画不透明层到小纹理，供 Slint 判断"哪些元素被 3D 内容遮挡"。
3. **音频管线**：`Player` 内部 spawn decode 任务（symphonia 解码成 f32 PCM）→ rodio 输出 + `visualizer`（FFT Analyzer 频谱）双路；`codec.rs` 提供 Encoder/Decoder/Tee 抽象用于测试与转发；`stream_source.rs` 做 HTTP 分块流式读。
4. **syncplay**：`Signalling`（axum SSE 轮询长连）→ `Client`（Event/Command 循环）→ `Peer`（webrtc-rs，role=host/listener，listener 推流）→ `pump`（spawn_host/spawn_listener 把本地 PCM 泵给对端）→ `Roster`（在线成员）。
5. **UI 组装**：`ui/lib.rs` 把所有 Slint 回调 bind 到 app-core/api；`app.slint` 的 `MainWindow` 依宽度在 compact/wide 两套版式间切换（`if root.width < 720px`）；`ui/music.rs` 是全项目测试最厚的地方（bounded context 的 query 驱动测试）。
6. **每端入口**：Android 用 `android-activity` 的 `android_main`（`unsafe(no_mangle)`），Web 用 `wasm_bindgen(start)`（异步 init，因 wgpu 实例必须 async），desktop 退出用 `libc::_exit` 规避 wgpu TLS 析构崩溃，iOS 导出 `slint_study_main`。
7. **后端**：`server/main.rs` 的 `Upstream` 结构包装 BangDream 官方 gRPC（tonic 客户端），REST 端点带 `SearchQuery`/`PageQuery` 分页；`signaling.rs` SSE 广播加入/离开；proto 定义在 `server/proto/music/v1/music.proto`。

## 1.3 文件级功能说明（函数/结构体级别）

### crates/contract/src/lib.rs（8.7KB）
- `HealthDto` / `TrackDto` / `DeviceDto` / `LyricLineDto` / `PlaybackStateDto` — 线上 DTO 结构体（wire format，带 `serde` 与 from 转换）
- `SearchQuery` / `PageQuery` — 请求参数结构（关键词、页码、页大小）
- `PlaybackCommand` / `DeviceCommand` 等枚举 — 播放/设备控制命令的线上表示
- 职责：**只**放跨进程共享的类型（ADR-0001 定的边界），禁止 app-core 直接依赖 api/audio 等平台 crate。

### crates/app-core/src/
- `lib.rs`（1.1KB）：`mod` 声明 + 公开 re-export。
- `health.rs`（6.9KB）：`HealthTracker` — 后端健康状态机（`start()`/`update()`/`expire()`，`seen_at`、`failures`、`labels` 字段），`snapshot()` 输出当前状态。供 UI 显示"后端离线/在线"。
- `counter.rs`（1.6KB）：`Counter` — `inc()`/`get()` 计数器（同播消息序号等用途）。
- `playback.rs`（12KB）：`PlaybackState` 枚举（Stopped/Buffering/Playing/Paused）+ `Command` 枚举 + `CommandLog`（命令日志，`is_terminal()`/`apply()` 状态迁移函数，附带大量单元测试）。
- `queue.rs`（13KB）：`Queue` — 播放队列（`tracks`/`position`/`shuffle_rng`），`current()`/`advance()`/`set_shuffle()`/命令应用 + 测试。
- `lyric.rs`（4.4KB）：LRC 歌词解析 — `parse_metadata()`/`parse_timing()`/`LyricLineDto` 转换。
- 设计要点：**纯领域逻辑、无 async、无平台依赖**，全部可被桌面/Android/Web 共享且可单测。

### crates/api/src/lib.rs（14.9KB）
- `ApiError` — 错误枚举（Network/Http/Deserialize/…）
- `Api` 结构体 + `send()` — 注入式请求函数：`health()`/`search()`/`daily()`/`stream()`/`favorite()`
- wasm 分支用 `reqwest` 的 wasm 特性，native 分支用原生 reqwest；**Send 边界封在 crate 内部**（ADR-0002：调用方不需要关心 Send）。

### crates/audio/src/
- `lib.rs`：`AudioError`、`Source` trait、`runtime()`（rodio OutputStream 封装）、`decode()`（symphonia 解码任务）、`Player` — `new()`/`play()`/`visualizer()`/`pause()`/`resume()`/`stop()`/`seek()`/`volume()`。
- `codec.rs`：`Encoder`（`LoopEncoder`/`ForwardEncoder`/`NullEncoder` 三个实现，供测试与转播）、`Decoder`（含 `SeekDecoder`）、`Tee` — 一路输入分多路输出（播放+频谱双路消费）。
- `spectrum.rs`：`Analyzer` — FFT 频谱分析器，`payload()` 输出频段能量数组（喂给 render3d 可视化）。
- `stream_source.rs`：`StreamSource` — HTTP 分块流式读（Range 续传）。

### crates/render3d/src/
- `lib.rs`（39KB）：`Scene` — Bevy App + wgpu 共享 device 的管理者：`new()`/`new_async()`（wasm）、`require_wgpu_29(Manual)`、`spawn_camera()`、`spawn_occluder_camera()`（双相机遮挡）、`render_viz_frame()`、`extract_texture()`（framebuffer 像素读回）、`resize()`（动态分辨率重建）、`apply_pointer()`、`apply_cover()`（封面纹理）、`rebuild_viz_content()`（粒子/点云重建）。`VizFrame` 结构、`WARP_SIDE` 常量。
- `cloud.rs`（48KB）：`CloudPass` — 独立 wgpu render pass（云粒子渲染，含 WGSL shader 内嵌），`CloudParams`（粒子数/密度），`DensityMode`（PCD 点云模式），`render_frame()`。这是"不经过 Bevy ECS、直接用 wgpu 画"的范本。
- `navglass.rs`：`NavParams`、`NavGlassPass`（`new()`/`render_frame()`）— 侧栏 metaball 选中器纹理的独立 wgpu pass。
- `warp.rs`：`WarpPass` — 播放页 warp 扭曲视觉的独立 wgpu pass。
- `*.wgsl`：cloud/navglass/warp 三个 shader 源码。

### crates/syncplay/src/
- `signalling.rs`：`Signalling`（`connect()`/`next()`/`sender()`）、`SignalSender` — axum SSE 信令通道。
- `client.rs`：`Client` — `start()`/事件循环（Event/Command 枚举）。
- `peer.rs`：`PeerRole`（host/listener）、`Peer` — `negotiate()`/`on_track()`/`state()`/`configuration()`/`audio_track()`（webrtc-rs 封装）。
- `pump.rs`：`spawn_host()`/`spawn_listener()` — 把本地 PCM 音频泵给对端的任务。
- `session.rs`：`Roster` — `new()`/`update()`/`others()` 在线成员表。
- `envelope.rs`：`Envelope` — 信令消息类型（join/leave/offer/answer/ice…）。

### crates/ui/src/
- `lib.rs`（15.6KB）：`fps_enabled`（env 开关，`OSMOSIS_FPS`）、`MAX_TAB`、`build_ui()`、`run()`/`run_with_renderers()`（注册 NavGlassPass/WarpPass）、`Source`。
- `cover.rs`：`CoverFeed`（解码封面像素流）、`CoverPixels`。
- `music.rs`（41KB）：`Deck`（播放器页面状态）、`bind()`（绑定全部 Slint 回调）、`bind_search()`/`bind_list()`/`fetch_daily()`/`fetch_into()`/`show()`/`push_rows()`/`bind_play()`/`bind_controls()`/`advance()`/`play_current()`/`start_autoplay()` — 页面逻辑 + 大量测试。
- `nav_glass.rs`：`NavGlassControls`（strip 宽度等 Slint 属性结构）。
- `syncplay.rs`：`Sync` — `bind()`/`feed()`/`is_listening()`/`leave()`/`signalling_url`/`identity`/`describe_role()`/`bind_push()`。
- `viz.rs`：`VizControls`/`VizPointer`/`VizCover`/`VizImages` — 渲染器与 Slint 之间的载荷结构；`payload()`。
- `slint/app.slint`（55KB）：`MainWindow`（顶部导航 + 播放页 + 设置页，`fps`、`tracks`、`playback-text`、`compact` 版式切换、`play-page-open`、`viz-*` 属性）；`HoverButton`、`NavEntry`/`TrackRow`/`DeviceRow` 结构、`Nav` global、`RoundControl`、`SyncStrip`、`EntryCard` 组件。
- `slint/glass.slint`：玻璃质感样式组件。
- `build.rs`：编译期 Slint 生成（`slint-build`，含 Android MCP fork patch）。

### apps/
- `apps/android/src/lib.rs`：`android_main`（`unsafe(no_mangle)`）— android-activity 入口、日志重定向、MCP 端口注入、`Scene::new` 后 `run_with_renderers()`；工程侧有 gradle + `MainActivity.java`（NativeActivity 包装）+ 全面屏配置。
- `apps/desktop/src/main.rs`：`main()` — env_logger、`Scene::new`、注册 NavGlass/Warp pass、`run_with_renderers()`；退出用 `libc::_exit` 规避 wgpu TLS 析构崩溃（评论区有详细说明）。
- `apps/web/src/lib.rs`：`wasm_bindgen(start)` + `console_error_panic_hook` → `ui::run()`（wgpu 实例必须 async 创建）。
- `apps/ios/src/lib.rs`：`slint_study_main`（`unsafe(no_mangle)`）导出。
- 各端 `Cargo.toml`：Android 用 `android-activity`、desktop 用 winit/egui 窗口、web 用 wasm。

### server/
- `main.rs`：`Upstream` 结构（BangDream gRPC 上游封装，含 `fail()` 错误映射）、`SearchQuery`/`PageQuery`（分页）、REST 路由（搜索/每日/流地址）、信令 SSE。
- `lib.rs`/`bangdream.rs`/`error.rs`/`paging.rs`/`roster.rs`/`signaling.rs`：后端模块（上游代理、错误、分页、在线名单、SSE 广播）。
- `proto/music/v1/music.proto`：BangDream 代理 proto 定义。

### xtask/
- `main.rs`：子命令分发。
- `android.rs`：cargo-ndk 打包 + APK 组装。
- `boundaries.rs`（核心亮点）：8 个依赖边界检查函数，用 `cargo tree` 输出断言"哪个 crate 不得依赖哪个 crate"（如 `app-core` 不得依赖 `api`、`ui` 不得依赖 `audio` 内部符号等），CI 必跑。
- `shell.rs`：shell 命令封装。

### 根工程
- `Cargo.toml`（7.5KB）：workspace 定义 + 全部依赖（含 `[patch]` 的 Slint fork）。
- `justfile`（16.4KB）：开发命令清单（build/run/test/边界检查/发布）。
- `docs/adr/`（12 个 ADR）：0001 contract 边界、0002 api 内藏 Send、0003 apps 薄入口、0005 wgpu device 共享基座（**本组研究价值最高**）、0007 UI 版式按宽度切换、0011 Bevy 硬依赖等。
- `docs/note/slint-bevy-architecture-and-direction.md` / `vision.md`：架构方向与愿景笔记。
- `AGENTS.md`（10.5KB）：开发守则（含"不引第三方 crate 到 contract""边界检查必须过"等规则）。

## 1.4 与 torvox 功能对比

| 功能 | torvox 有没有 | 对比结论 |
|---|---|---|
| wgpu GPU 渲染 | ✅ 有（wgpu 30，Vulkan-only，直渲终端网格） | osmosis 用 wgpu-29 且**共享给 Slint+Bevy 双渲染器**；torvox 单渲染器更简单，但"单一 device + 外部纹理导入 + framebuffer 读回"模式在 torvox 做 MCP 截图/OCR/桌面预览时有直接参考价值 |
| VT 解析 | ✅ 有（libghostty-vt 完整） | osmosis 无终端，不适用 |
| 音频播放/频谱 | ❌ 无 | osmosis 的 `Tee`（一路 PCM 双消费：播放+FFT）与 `Analyzer` 是干净的可移植模式；torvox 若要加"终端响铃可视化"或 MCP 音频事件可借鉴 |
| 3D 可视化（Bevy） | ❌ 无 | torvox 是 2D 文本渲染，不需要 Bevy；但 osmosis 的**独立 wgpu pass（绕过 ECS）**证明了"wgpu 不止能画一种场景"，对 torvox 未来加特效层（如选中动画、背景模糊）是架构参考 |
| WebRTC 同播 | ❌ 无 | 与 torvox 定位无关，不吸收 |
| 跨端（Android/desktop/iOS/Web） | ❌ 仅 Android | torvox 刻意只做 Android（ADR 无跨端条目）；osmosis 的 apps 薄入口 + crate 分层思想可用于 torvox 未来拆 desktop 调试端（exec-bin 已有雏形） |
| 依赖边界自动化（xtask boundaries） | ⚠️ 部分（单 crate 无边界问题） | torvox 单 crate 化后边界问题消失，但**边界检查思路可复用到"libghostty-vt 不得泄漏"的验证** |
| MCP | ✅ 有（tower-mcp 内嵌） | osmosis 的 Slint fork 也加了 Android MCP（用于 agent 截图/控制），印证 torvox 方向；osmosis 的"MCP 端口注入"做法可对比 torvox 的 Unix socket 方案 |
| 后端服务器 | ❌ 无 | 与 torvox 无关（torvox 不需要在线服务） |
| 决策文档（ADR） | ✅ 有（8 个） | osmosis 12 个 ADR 质量高，可作为 torvox 未来 ADR 的写作参照（见 1.7） |

## 1.5 依赖分析

| 依赖 | 版本 | 激进程度 | 适用于 torvox？ |
|---|---|---|---|
| slint | 1.17/1.18-dev（fork patch） | 激进（需 fork + 自维护 patch，Android 上还要 NativeActivity 配合） | ❌ torvox 已定 Kotlin Compose UI；Slint 无法替换现有栈 |
| bevy | 0.16 | 激进（APK 体积 +30MB、编译时间长、feature 矩阵复杂） | ❌ 终端不需要 ECS 游戏引擎 |
| wgpu | 0.29（torvox 用 0.30） | 先进但稳定 | ⚠️ 版本略旧于 torvox；共享 device 的用法值得抄（见 1.6） |
| webrtc-rs | 最新 | 激进（编译重、wasm/Android 支持参差） | ❌ 无同播需求 |
| rodio/symphonia | 0.19/0.5 | 常规 | ⚠️ 若 torvox 未来做音频反馈可考虑 |
| axum/tonic | 0.8/0.12 | 常规 | ❌ 无服务端需求 |
| android-activity | 0.6 | 常规 | ⚠️ torvox 用普通 Activity + TextureView，不需要 |

结论：osmosis 的依赖整体**先进激进**（Slint fork + Bevy 是重投入），但"激进"集中在 UI/3D 层，底层（wgpu、tokio、serde）与 torvox 同代。**不值得**把 Bevy/Slint 引入 torvox；**值得**抄的是架构模式而非依赖。

## 1.6 可吸收到 torvox 的具体内容

1. **wgpu 共享 device + framebuffer 读回模式**（`crates/render3d/src/lib.rs` 的 `Scene`/`extract_texture`）。
   - 场景：torvox 的 MCP 工具未来要 `screenshot`（给 agent 看终端画面做 OCR/验证），或桌面 exec-bin 要导出 PNG。
   - 建议实现（示意）：
   ```rust
   // native/src/render/snapshot.rs（新文件）
   // 从 render pass 结束后读回 surface/纹理像素（借鉴 osmosis render3d::extract_texture）：
   // 1) 创建 1x1 大小 texture copy（Bgra8UnormSrgb）
   // 2) encoder.copy_texture_to_buffer → BufferSlice.map_async
   // 3) 轮询 device.poll(Maintain::Wait) 后取 buffer 数据
   // 注意：torvox 是每帧连续渲染，读回会阻塞渲染线程；
   // 建议只在 MCP screenshot 请求时临时暂停渲染循环并做一次 blocking read，
   // 或者用单独的 readback texture + 双缓冲，避免影响主渲染路径（参照 ADR-0008 的 fast/slow path 划分）。
   ```
2. **依赖边界检查脚本**（`xtask/boundaries.rs` 的 cargo tree 断言）。
   - 场景：torvox 单 crate 后，唯一的外部依赖是 vendored libghostty-vt。可在 CI 里加一个 `scripts/check-vt-boundary.nu`：`cargo tree -p native -i libghostty-vt` 断言只有 `native::terminal` 模块引用它（可通过 `cargo geiger`/`cargo tree --edges` 组合实现），防止未来重构把 ghostty 类型泄漏到 render 层。
3. **Tee 双路消费模式**（`crates/audio/src/codec.rs`）。
   - 场景：若 torvox 实现"响铃 → 可视化脉冲"或"PTY 输出 → 关键字高亮数据流"两个消费者时，用 `Tee` 思路在 flume 上游分叉，保持单一写入方。
4. **动态分辨率重建**（render3d `resize()`）：torvox 的 `resize()` 已是核心能力（TIOCSWINSZ + surface resize），osmosis 的"按纹理实际大小自适应重建"思路可作为终端窗口快速拖拽时的防抖参考（torvox 已有类似处理，仅作对照）。
5. **`unsafe(no_mangle)` 显式标注 + `#[cfg]` 分端入口**：torvox 的 JNI 函数可统一改用 Rust 2024 的 `unsafe(no_mangle)` 风格（与 `apps/android/src/lib.rs` 一致），减少 `#[allow(improper_ctypes)]` 噪声。

## 1.7 项目文档吸收价值

**高。** osmosis 的 12 个 ADR 是"为什么"级别的决策记录，写作结构（Context / Decision / Consequences / Alternatives）与 torvox 现有 ADR 同源但更细，尤其：
- `0005-wgpu-device-as-shared-base.md`：多渲染器共用一个 device 的利弊 + 失败案例（曾把共享代码放错 crate 导致 Send 爆炸）——对 torvox 未来"GPU 层扩展"（截图、特效）是必读。
- `0002-send-boundary-lives-inside-api-crate.md`：把 async 线程边界封装在 crate 内部、调用方无感——对应 torvox 的 JNI 边界（Kotlin 不应感知 Rust 线程）。
- `0011-bevy-is-a-hard-dependency-not-a-feature.md`：反对 feature 矩阵的论证——支持 torvox 的 ADR-0001（单 crate、少 feature）。
- `docs/note/slint-bevy-architecture-and-direction.md`：双渲染器架构方向笔记，可类比 torvox 的 `docs/lessons/02-gpu-render.md`。
- 反面教材：`apps/desktop` 退出需 `libc::_exit` 规避 wgpu TLS 崩溃——提示 torvox 的 exec-bin 若做桌面退出也要处理（Android 无此问题）。

---

# 2. redterm（GlobalTechInfo/RedTerm）— proot 发行版终端

## 2.1 项目定位

一个**通过 proot 在 Android 上运行完整 Linux 发行版（Ubuntu/Debian/Kali/Alpine/Arch 等）**的终端 App，风格向 Termux 看齐但主打"桌面发行版体验"：内置发行版下载/安装/修复、bash 模板、指纹锁、夜间模式、桌面小组件、快捷设置磁贴、文件浏览、搜索高亮。**UI 是传统 View 体系（非 Compose）**，终端渲染是"PTY 读 + TextView 追加"（无 VT 状态机、无 GPU）。

## 2.2 完整架构

```
app/src/main/java/com/redtermapp/
├── RedTermApp.kt            Application（崩溃处理器、全局状态）
├── MainActivity.kt          欢迎页：发行版选择/指纹锁/导航
├── ui/
│   ├── TerminalActivity.kt  主终端页（1700+ 行）：会话、抽屉、搜索、快捷面板、分屏、导出
│   ├── TerminalBackend.kt   PTY 线程封装（读循环 + 输出回调）
│   ├── TerminalViewModel.kt 会话数据（LiveData）
│   ├── SettingsActivity.kt  设置页（字体/颜色/行为/按键/proot/DNS）
│   ├── WelcomeActivity.kt   首启向导
│   ├── BashTemplatesActivity.kt  bash 模板管理（别名/函数）
│   ├── FileBrowserActivity.kt    文件浏览器
│   ├── SearchHighlightOverlay.kt 搜索高亮覆盖层（Spannable）
│   ├── NightModeReceiver.kt      夜间模式广播接收
│   ├── QuickSettingsTile.kt      快捷设置磁贴
│   ├── RedTermWidgetProvider.kt  桌面小组件
│   └── WidgetConfigActivity.kt   小组件配置
├── distro/
│   ├── Distro.kt            Distro 数据类（名称/rootfs URL/大小）
│   ├── DistroRegistry.kt    内置发行版清单（含下载 URL 与校验信息）
│   └── DistroInstaller.kt   下载→校验→解压→setupRootfs()/repairRootfs()
├── proot/
│   ├── ProotInstaller.kt    proot 二进制安装
│   └── ProotRunner.kt       buildProotCommand()（--link2symlink/--kill-on-exit/--bind 列表）+ 启动
├── service/TerminalService.kt  前台服务（保活终端会话）
├── util/                     AppLock（指纹锁）、CrashHandler、StoragePermission
├── DnsHelper.kt             resolv.conf 写入
├── assets/init.sh            rootfs 内引导脚本（/dev、resolv.conf、PS1、ash）
├── assets/init-host.sh       宿主侧预启动初始化
└── native/build-proot.sh     交叉编译 proot 静态二进制
```

数据流：`TerminalActivity` → `TerminalBackend`（spawn `Runtime.exec` proot 进程 + 读线程）→ 行文本 → `TerminalViewModel` → `SearchHighlightOverlay`/TextView 显示；输入走 `TerminalBackend.write()`。

## 2.3 文件级功能说明

### RedTermApp.kt（1.7KB）
- `onCreate()`：注册 `CrashHandler`、初始化全局状态。

### MainActivity.kt（15.3KB）
- `onCreate()`：欢迎页/发行版列表、`AppLock` 指纹校验、进入 `TerminalActivity` 前导航逻辑。

### ui/TerminalActivity.kt（70KB，约 1700 行）
- `onCreate()`（171 行起）：全屏沉浸、设置加载、创建会话、工具栏/FAB/抽屉初始化。
- `writeShellConfigs()`（442 行起）：把 `.bashrc`/`.profile`/别名/环境变量写进 rootfs。
- `createNewSession()`（539 行起）：`TerminalBackend.start()` + ViewModel 注册。
- `switchToSession()` / `handleSessionFinished()`（642 行起）：多会话切换与退出清理。
- `setupSearch()`（846 行起）：搜索 UI + 高亮。
- `setupQuickPanel()`（910 行起）：快速面板（常用命令）。
- `setupSplitScreen()` / `setupExport()`：分屏与导出输出文本。
- `onResume()`/`onPause()`：会话挂起/恢复（`TerminalService` 协调）。

### ui/TerminalBackend.kt（9.3KB）
- `TerminalBackend`：`start()`（spawn proot + `initShell()`）、读线程循环（`read()` → `onOutput`）、`write()`、`resize()`（`setTerminalSize`）、`onExit` 回调。
- 注意：**无 VT 解析**，输出按行追加显示；`initShell()` 注入 `init-host.sh` 环境。

### ui/TerminalViewModel.kt
- `ViewModel`：会话列表、当前会话、输出 LiveData（供 UI 订阅）。

### distro/DistroRegistry.kt（8.4KB）
- `object DistroRegistry`：`distros` 列表（Ubuntu/Debian/Kali/Alpine/Arch 等），每项含 rootfs 下载 URL、压缩包大小、解压后大小、包管理器类型。
- `getDistro(name)` 等查询。

### distro/DistroInstaller.kt（约 18KB）
- `install()`：下载（进度回调）→ sha256 校验 → tar 解压 → `setupRootfs()`（448 行起：目录骨架、/dev、resolv.conf、用户配置）→ 写安装标记。
- `repairRootfs()`：损坏后修复（重新生成 /dev、linkerconfig 等）。

### proot/ProotInstaller.kt（9.4KB outline）
- `installProot()`：从内置/远端取 proot 静态二进制（`native/build-proot.sh` 产物），chmod +x，写入 app 私有目录。

### proot/ProotRunner.kt（10.6KB）
- `buildProotCommand()`：构造 `proot --link2symlink --kill-on-exit --rootfs=<rootfs> --bind=/dev --bind=/proc --bind=/sys --bind=<sdcard>:/sdcard ... /bin/ash`。
- `start()`：`ProcessBuilder` 启动 + 环境注入（PATH/HOME/TERM=xterm-256color）。

### service/TerminalService.kt
- 前台服务：通知 + 保持终端进程（屏幕关闭时续跑）。

### ui/SettingsActivity.kt（41KB）
- 设置分组：外观（字体大小/主题/背景）、行为（回车发送/长按粘贴）、按键（音量键/Enter）、proot 选项（--link2symlink 开关、bind 列表编辑）、DNS（`DnsHelper`）、存储权限引导。

### util/（AppLock.kt / CrashHandler.kt / StoragePermission.kt）
- `AppLock`：指纹/图案解锁校验（`BiometricPrompt`）。
- `CrashHandler`：未捕获异常写日志文件。
- `StoragePermission`：Android 11+ `MANAGE_EXTERNAL_STORAGE` 引导页。

### 其他
- `SearchHighlightOverlay.kt`：`Spannable` 高亮当前搜索词（在 TextView 之上）。
- `NightModeReceiver.kt`：监听夜间广播切换深色背景。
- `QuickSettingsTile.kt`：`TileService` 快捷开关（打开终端/切发行版）。
- `RedTermWidgetProvider.kt` / `WidgetConfigActivity.kt`：桌面小组件（快捷命令、会话状态）。
- `BashTemplatesActivity.kt`：模板库（别名、函数、PS1 样式）写入 `.bashrc`。
- `FileBrowserActivity.kt`：rootfs 内文件浏览。
- `DnsHelper.kt`：把系统 DNS 写入 `/etc/resolv.conf`。
- `assets/init.sh`（31 行）：rootfs 引导——建 `/dev`、`resolv.conf` 兜底（8.8.8.8）、`PIP_BREAK_SYSTEM_PACKAGES=1`、PS1 注入、无参数时进 `/bin/ash`，有参数时 `exec "$@"`。
- `assets/init-host.sh`：宿主侧预启动（挂载准备、proot 参数计算）。
- `native/build-proot.sh`：用 NDK 交叉编译 proot（静态链接、musl）。
- `app/build.gradle.kts`：传统 View 栈（appcompat + material），**无 Compose**；`settings.gradle.kts` 双模块（app + native）。

## 2.4 与 torvox 功能对比

| 功能 | torvox 有没有 | 对比结论 |
|---|---|---|
| PTY 会话（forkpty/exec） | ✅ 有（nix 实现，更强） | redterm 用 `Runtime.exec` + proot，无 TIOCSWINSZ 精确控制；torvox 的 PTY 层完胜，无需吸收 |
| VT 解析 | ✅ 有（libghostty-vt） | redterm **没有** VT 解析器（TextView 追加，多色输出会乱）；torvox 全面碾压，无需吸收 |
| GPU 渲染 | ✅ 有（wgpu） | redterm 是 TextView 渲染；torvox 完胜，无需吸收 |
| proot 发行版支持 | ❌ 无 | **redterm 的最大独有价值**：发行版下载→校验→解压→setupRootfs→proot 启动全流程（见 2.6） |
| 指纹锁 | ❌ 无 | torvox 无安全锁；可吸收（低优先） |
| 桌面小组件/快捷磁贴 | ❌ 无 | torvox 无；可吸收（低优先，参考 termx 的 Widget 亦可） |
| 搜索高亮 | ✅ 有（TextSearchBar + 高亮） | torvox 已实现且更强（支持滚动缓冲+OCR 验证）；redterm 的 Spannable 方案不吸收 |
| 夜间模式 | ⚠️ 部分（主题切换） | redterm 用系统广播；torvox 主题系统已覆盖，不吸收 |
| 前台服务保活 | ✅ 有（TerminalForegroundService） | 两者都有，torvox 实现更完整（含通知助手） |
| 会话多开/切换 | ✅ 有（SessionDrawer） | 都有；redterm 抽屉实现是传统 View，torvox Compose 版本更优 |
| 输出导出 | ⚠️ 未知（无明确导出功能） | redterm 有"导出会话输出到文件"；可作小功能补丁参考 |
| 崩溃日志 | ✅ 有（LogcatDumpWriter 等） | torvox 的监控体系更强 |

## 2.5 依赖分析

| 依赖 | 激进程度 | 适用于 torvox？ |
|---|---|---|
| appcompat/material（传统 View） | 常规、偏旧 | ❌ torvox 是 Compose；无参考 |
| androidx.biometric（指纹） | 常规 | ⚠️ 若 torvox 加锁屏可用，API 简单 |
| androidx.work（widget 刷新） | 常规 | ⚠️ 同上 |
| 无 Compose、无协程、无 DataStore | 保守 | ❌ 落后于 torvox 现有栈 |

结论：**依赖不激进、不先进**，纯 AndroidX 常规栈。redterm 的参考价值在"功能/流程设计"（proot 全流程、引导脚本），不在依赖。

## 2.6 可吸收到 torvox 的具体内容

1. **proot 发行版支持（唯一值得认真评估的大项）**——但**建议先不做**：
   - torvox 定位是 `com.termux` 的 Android shell 终端（bootstrap 安装器已有），proot 发行版是另一条产品线（rootfs 数个 GB、需要 `MANAGE_EXTERNAL_STORAGE`、与 Termux 包管理冲突）。
   - 若未来要做，直接复用 redterm 的流程骨架：`DistroRegistry`（发行版清单+URL）→ `DistroInstaller`（下载/校验/解压/setupRootfs）→ `ProotRunner.buildProotCommand`（`--link2symlink --kill-on-exit --bind=...`）→ `init.sh` 引导。
   - 其中 **`init.sh` 的写法可直接抄**（`assets/init.sh`）：`/dev` 兜底、`resolv.conf` 兜底、`exec "$@"` 透传——torvox 若做容器支持，这份脚本是最小可用版。
2. **输出导出**：`TerminalActivity.setupExport()` 的思路（把会话 scrollback 写入 `/sdcard`）可补进 torvox 的会话菜单（torvox 有 GridSnapshot，导出质量会远好于 redterm 的文本追加）。
3. **指纹锁**：`util/AppLock.kt`（BiometricPrompt 封装）约 150 行，若 torvox 未来有"隐私模式"设置可吸收。
4. **发版流程参考**：无（redterm 无 CI/签名流程文档）。

## 2.7 项目文档吸收价值

**低。** README 是功能列表 + 截图；无 ADR、无架构文档、无测试文档。唯一有吸收价值的是 `assets/init.sh`（可执行脚本即文档）与 `native/build-proot.sh`（交叉编译 proot 的完整命令序列，若 torvox 未来要编译 proot 可直接复用）。CHANGELOG 可作功能演进参考（看它踩过的坑：proot 版本兼容、Android 11 存储权限）。

---

# 3. terminator（8dmusichannels-star/terminator）— Kotlin Compose 终端

## 3.1 项目定位

一个**纯 Kotlin + Jetpack Compose 的自研终端**：手写 VT 解析状态机（约 500 行）、自研环形滚动缓冲、Compose `Canvas`/`drawTextRun` 逐行渲染、多会话抽屉、前台服务保活、DataStore 设置、9 个设置页面、虚拟功能键栏、选择工具栏。双模块结构（`terminal-emulator` 库 + `app`）。与 torvox 同为 Compose 终端，但渲染走 CPU 文本路径——是"不依赖 GPU/不依赖 Ghostty 的最小可行终端"范本。

## 3.2 完整架构

```
terminal-emulator/  （库模块：可独立复用）
├── emulator/TerminalEmulator.kt   VT 状态机（NORMAL/ESC/CSI/OSC/CHARSET + UTF-8 解码）
├── emulator/TerminalBuffer.kt     screen + scrollback + 选择 + 光标 + 滚动区
├── emulator/TerminalSession.kt    会话（进程启动/输入/resize/信号）
├── emulator/TerminalView.kt       Compose AndroidView 包装 + 渲染/触控/滚动/选择
├── emulator/NativePty.kt          JNI 声明（open/read/write/setWindowSize/sendSignal/closeFd）
└── cpp/pty.c                      forkpty + exec shell + 读线程（C 实现）

app/（Compose UI）
├── TerminatorApp.kt               Application 初始化
├── ui/MainActivity.kt             主界面（80KB：会话、手势、输入、虚拟键、选择、抽屉、标题栏）
├── ui/MainViewModel.kt            StateFlow 会话状态机
├── ui/VirtualKeyBar.kt            虚拟功能键栏（Esc/Ctrl/Alt/Tab/方向/Fn）
├── ui/SelectionToolbar.kt         选择操作栏（Copy/Paste/Cancel）
├── ui/SessionDrawer.kt            会话抽屉
├── ui/TerminatorTitleBar.kt       标题栏（菜单/快速添加）
├── ui/FontResolver.kt             字体解析（缓存 ttf）
├── ui/theme/Theme.kt              Compose 主题
├── session/SessionRepository.kt   会话增删改查（持久化）
├── session/SessionModels.kt       SessionEntry/SessionType
├── session/SessionForegroundService.kt  前台服务
├── settings/SettingsRepository.kt DataStore（类型安全 key）
└── ui/settings/*.kt               9 个设置页（Appearance/Keymapper/Keyboard/Sessions/Theme/Display/Sound/Storage/KeyboardSettings）
```

数据流：`pty.c` 读线程 → `TerminalSession` → `TerminalEmulator.process()` → `TerminalBuffer`（screen+scrollback）→ `TerminalView` 每帧 `drawTextRun` 绘制可见区；输入反向：`VirtualKeyBar`/硬件键盘 → `TerminalSession.sendInput/sendCtrl/sendEscapeSequence` → `NativePty.write`。

## 3.3 文件级功能说明

### terminal-emulator/src/main/java/com/terminator/emulator/

**TerminalEmulator.kt（23.8KB）** — VT 解析器
- `enum State`：NORMAL / ESC / CSI / OSC / CHARSET（状态机核心，`:12` 附近）。
- `process(buffer: ByteArray)`：对外入口，逐字节喂 `processByte()`。
- `processByte()`：UTF-8 多字节序列解码（pending 字节缓冲）→ 按 State 分发到 `processNormal()`/`processEsc()`/`processCsi()`/`processOsc()`/`processCharset()`。
- `processNormal()`：可打印字符入 buffer；控制字符：BEL（`onBell` 回调）、BS、HT（tab stop）、LF/VT/FF（换行+必要时滚动）、CR。
- `processEsc()`：ESC 序列（`7` 保存光标、`8` 恢复、`[` 进 CSI、`]` 进 OSC、`(`/`)` 进 CHARSET 等）。
- `processCsi()`：CSI 解析——`parseParams()`（`:455` 附近，分号参数表解析）、SGR（颜色/样式，含 256 色与真彩 `38;2;r;g;b`）、光标移动（`A/B/C/D/H/f/G/E/F`）、清屏/清行（`J/K`）、滚动区（`r`）、插入/删除行（`L/M`）、`h/l` 模式设置（光标可见/自动换行）。
- `processOsc()`：OSC 0/1/2（窗口/图标标题 → `onTitleChanged`）、OSC 4/10/11（调色板/前景/背景色）。
- `processCharset()`：G0/G1 字符集切换（`0`/`B`/`A`）。
- 能力边界：**无** DECRQSS/鼠标 SGR/焦点事件/六元组报告——只覆盖日常 shell 交互所需子集。

**TerminalBuffer.kt（11.8KB）**
- `data class TerminalCell`：char + fg/bg（Int）+ bold/italic/underline/strikethrough/blink/inverse 标志。
- `class TerminalBuffer`：
  - `screen: Array<Array<TerminalCell>>`（行列网格）、`scrollback: MutableList<Array<TerminalCell>>`（滚动缓冲，容量上限）。
  - `cursor`（row/col）、`savedCursor`、`scrollRegion`（上下边界）。
  - `getCell()`/`setCell()`/`putChar()`：写格（自动换行处理）。
  - `scrollUp()`/`scrollDown()`（区域滚动，溢出进 scrollback）、`clearLine()`/`clearScreen()`（`eraseBelow` 等参数化）、`reset()`、`setScrollRegion()`、`scrollToBottom()`/`scrollOffset`（查看历史）。
  - `selectionStart/End`：选择区间存储。

**TerminalSession.kt（10KB）**
- `class TerminalSession`（`sessionId`）：`start()`（`NativePty.open` + shell 启动）、`write()`/`sendInput()`（文本编码）、`sendCtrl()`（`0x01..0x1A` 映射）、`sendEscapeSequence()`、`sendArrowKey()`（`\x1b[A` 等）、`resize(rows, cols)`、`getChildPid()`、`isRunning`。

**TerminalView.kt（13KB）**
- Compose `AndroidView` 包装 `TerminalView`（自绘 View）：`onDraw` 里按可见行 `drawTextRun`（逐行、逐 run 着色）、光标闪烁、选择高亮（inverse video）、滚动手势（fling/overscroll）、`scrollOffset` 变化回调、`onSizeChanged` → `session.resize`。

**NativePty.kt（5.6KB）**
- `object NativePty`（`:13`）：`external fun open(...)`、`read(fd)`、`write(fd, bytes)`、`setWindowSize(fd, rows, cols)`（`:43`）、`sendSignal(pid, signal)`（`:49`）、`closeFd(fd)`（`:52`）。JNI 直连（`System.loadLibrary("pty")`）。

**cpp/pty.c（6.7KB）**
- `openpty`/`forkpty` 会话创建（子进程 `setsid` + `TIOCSCTTY` + `execvp` shell）。
- 读线程：`read()` 阻塞循环 → 回调 Java（`onOutput`）。
- `setWindowSize`：`TIOCSWINSZ`（rows/cols）。
- `sendSignal`：`kill(-pid, sig)` 进程组信号。

### app/src/main/java/com/terminator/app/

**TerminatorApp.kt（3.2KB）**
- `onCreate()`（`:24`）：DataStore 初始化、主题预加载、崩溃兜底。

**ui/MainActivity.kt（80KB，约 1060 行）**
- `onCreate()`（60 行起）：全屏、`setContent` → `TerminalScreen`（Compose）、ViewModel 注入。
- `TerminalScreen()`：主组合——标题栏、`TerminalView`、`VirtualKeyBar`、`SelectionToolbar`、`SessionDrawer`、输入处理。
- 输入：`onKeyEvent`（硬件键盘：Ctrl/Alt 组合 → `sendCtrl`/转义序列）、IME（`BaseInputConnection` 自定义 `commitText` 编码）、滑动手势切换会话。
- 选择：长按进入选择模式 → 拖拽扩展 → `SelectionToolbar`（Copy/Paste/Cancel，`ToolbarAction` 定义在 SelectionToolbar.kt `:37`）。
- `SessionDrawer`：会话列表（新建/切换/关闭，`SessionEntry` 驱动）。
- `TerminatorTitleBar`（`:29` 菜单按钮、`:34` 快速添加按钮）。
- 设置导航到 9 个设置屏。

**ui/MainViewModel.kt（17KB）**
- `MainViewModel`：`StateFlow<UiState>`（sessions、activeSessionId、selection、scrollOffset…）、`onCommand()`（open/close/switch/rename 会话、paste、select-all 等）、`observe()` 订阅 `SessionRepository`。

**session/SessionRepository.kt（6.2KB）**
- `SessionRepository`：`addSession()`/`removeSession()`/`getSession(id)`/`update()`，内部 `MutableStateFlow` + 持久化（DataStore 存会话元数据，进程生命周期由 Service 管理）。

**session/SessionModels.kt（1.9KB）**
- `enum SessionType`（`:5`）：COMMAND_ARG / FILE_BASE。
- `data class SessionEntry`（`:13`）：id/name/type/command/args/workdir/rows/cols 等。
- `resolvedExecutable()`（`:31`）：按类型解析最终可执行文件（命令参数模式 / 文件打开模式）。

**session/SessionForegroundService.kt（3.3KB）**
- 前台服务：持活会话进程（`startForeground` + 通知），Activity 销毁时继续运行；提供 `sendCommand` 广播接口。

**settings/SettingsRepository.kt（3.9KB）**
- `object SettingsKeys`（`:21`）：DataStore `Preferences.Key` 全集（fontSize/theme/fontFamily/keepScreenOn/cursorBlink/bellVibrate/extraKeys 布局等）。
- `class SettingsRepository`（`:69`）：`set()`（`:74`）/`get()`/`observe()`——类型安全 DataStore 封装。

**ui/VirtualKeyBar.kt**
- 虚拟功能键栏：ESC / CTRL / ALT / TAB / ←↑↓→ / Fn（F1-F12 第二层）/ Del / Home / End / PgUp / PgDn；修饰键 sticky（点击保持按下状态，再点弹起）。

**ui/FontResolver.kt**
- `resolve(fontFamily)`（`:19`）：从系统字体/内置字体解析 `Typeface`，`cached`（`:24`）落盘缓存 ttf。

**ui/settings/*.kt（9 个文件）**
- `AppearanceSettingsScreen`：字体大小/字族/行距。
- `KeymapperScreen`：物理键盘键位重映射（`keycode → 序列`）。
- `KeyboardSettingsScreen`：IME 行为（回车发送、自动显示）。
- `SessionsSettingsScreen`：默认 shell/工作目录/新会话命令。
- `ThemeSettingsScreen`：明暗主题 + 强调色。
- `DisplaySettingsScreen`：光标样式/闪烁、滚动缓冲行数。
- `SoundSettingsScreen`：响铃行为。
- `StorageSettingsScreen`：存储权限/导出路径。
- `Theme.kt`：Compose Material 主题包装。

## 3.4 与 torvox 功能对比

| 功能 | torvox 有没有 | 对比结论 |
|---|---|---|
| Compose UI | ✅ 有 | 同栈；terminator 无 Compose 之外的桥接层，torvox 有 TextureView+JNI 桥，复杂度不同但收益不同 |
| VT 解析 | ✅ 有（libghostty-vt 完整） | terminator 手写 ~500 行只覆盖常用子集（无 DECRQSS/鼠标/焦点报告/连字）；**torvox 不该退回到手写**，但 terminator 的状态机结构（NORMAL/ESC/CSI/OSC/CHARSET 分派 + UTF-8 解码器）与 torvox 的 libghostty-vt 输入侧可互为对照测试用例 |
| 渲染 | ✅ 有（wgpu GPU） | terminator 用 `Canvas.drawTextRun`（CPU 逐行）；torvox GPU 渲染在性能/连字/emoji 上全面占优。**terminator 的价值是"无需 GPU 的兜底渲染"——torvox 是刻意 GPU-only（ADR-0008），不吸收** |
| 滚动缓冲 | ✅ 有（Ghostty scrollback + GridSnapshot） | terminator 用自研 `scrollback: MutableList` + scrollOffset；torvox 的 ghostty 滚动查询更完整（含选择扩展） |
| 选择复制 | ✅ 有（完整选择系统） | terminator 只有"长按→拖拽→Copy/Paste"三步；torvox 有词选择/拖拽手柄/系统菜单——torvox 完胜 |
| 虚拟功能键栏 | ✅ 有（ModifierBar + NerdKeyLabels） | 都有；torvox 的 ModifierBar 支持 Nerd Font 标签与 sticky 修饰键，terminator 的 VirtualKeyBar 支持 Fn 第二层（F1-F12）。**可对照补 Fn 层** |
| 多会话 | ✅ 有（SessionDrawer） | 都有；terminator 支持"会话配置文件（COMMAND_ARG/FILE_BASE 类型）"快速启动，torvox 会话创建更简单——配置文件化是可选增强 |
| 前台服务 | ✅ 有（TerminalForegroundService） | 都有，实现同构 |
| 设置页 | ✅ 有（SettingsScreen 单页） | terminator 拆 9 个独立页面；torvox 是单页分组。交互无所谓优劣，torvox 不必抄 |
| DataStore 设置 | ✅ 有（SettingsRepository + SettingsDataStoreProvider） | 同构；terminator 的 `SettingsKeys` 集中式 key 定义值得参考（torvox 也有类似） |
| 会话持久化（元数据） | ⚠️ 部分（bootstrap 状态持久化） | terminator 持久化会话元数据（重启恢复会话列表）；torvox 会话不跨启动保留——**可吸收**（见 3.6） |
| 响铃处理 | ✅ 有（Ghostty 事件 → 通知） | 都有；terminator 支持铃声设置项，torvox 有 TerminalNotificationHelper |
| 输出导出 | ⚠️ 无 | terminator 无；同 redterm 一样可补 |

## 3.5 依赖分析

| 依赖 | 激进程度 | 适用于 torvox？ |
|---|---|---|
| Compose BOM + Material3 | 常规 | ✅ 与 torvox 同栈 |
| androidx.datastore | 常规 | ✅ torvox 已在用 |
| androidx.lifecycle（ViewModel/StateFlow） | 常规 | ✅ 同栈 |
| JNI + 手写 C（pty.c） | 常规 | ⚠️ torvox 用 nix crate 管理 PTY，更安全；不吸收 C 方案 |
| 无 kotlinx-serialization/无 Hilt/无 Room | 保守 | ❌ torvox 栈更完整 |

结论：**依赖不激进**，全部是稳定常规库；torvox 依赖面已覆盖 terminator 全部依赖。它的可吸收点在架构/交互设计，不在依赖。

## 3.6 可吸收到 torvox 的具体内容

1. **会话元数据持久化 + 重启恢复**（`SessionRepository` + `SessionModels.SessionEntry`）。
   - 场景：torvox 重启 App 后会话列表清空（进程随 Activity 结束）。若要"会话续跑/恢复"，参考 terminator：把 `SessionEntry`（id/name/type/command/workdir/rows/cols）序列化进 DataStore，前台服务持活进程，Activity 重建时按元数据重挂。
   - 注意与 torvox ADR-0007（session lifecycle）对齐——torvox 的 Session 生命周期由 Rust 侧管理（SessionRegistry），Kotlin 侧只做事件订阅；恢复时需通过 JNI 重新 attach，而非照抄 Kotlin 侧重建。
2. **Fn 第二层虚拟键**（`VirtualKeyBar` 的 Fn 层 F1-F12）：torvox `ModifierBar` 已有 Esc/Ctrl/Alt/Tab/方向；补 Fn 层时可对照 terminator 的 sticky 实现（修饰键点击置位、再点复位、随按键消费）。实现建议：
   ```kotlin
   // android/app/src/main/java/terminal/emulator/ui/ModifierBar.kt（扩展）
   // Fn 层实现要点（参照 terminator VirtualKeyBar）：
   // 1) 增加 Fn 状态到 ModifierState（torvox input/ModifierState.kt）
   // 2) Fn+数字/字母 → sendEscapeSequence("\u001b[15~" 等 F1-F12 码)
   // 3) Fn sticky：点击置位 → 下一个按键消费后自动复位（terminator 同款语义）
   ```
3. **选择工具栏的"粘贴"直达**（`SelectionToolbar` Copy/Paste/Cancel 三键）：torvox 用系统菜单（ActionMode），粘贴走 PasteChipOverlay——交互不同，无需抄；但"选择态快速 Copy"的一键语义可作 torvox 未来 ActionMode 定制按钮参考。
4. **VT 解析测试对照**：terminator 的状态机简单，可把它的行为当作"最小正确性"基线：torvox 的 libghostty-vt 集成测试（integration-tests/）可加入同款用例（如 `\x1b[2J` 清屏后光标归位、滚动区 `\x1b[r` 后 LF 行为），验证 ghostty 行为与朴素实现一致（防回归参考，非功能需求）。

## 3.7 项目文档吸收价值

**低。** README（3.4KB）只是功能列表 + 截图；无 ADR/架构文档/测试文档。但其**代码结构本身**（双模块拆分、emulator 与 app 解耦）可作为 torvox 未来拆 `terminal-emulator` 库模块的命名/边界参照（torvox 现在是单 crate + 单 app 模块）。`fastlane/` 有基础 CI 配置可忽略。

---

# 4. termx（mwmQi/TermX）— 类 Termux 超级终端

## 4.1 项目定位

一个**功能密度极高的 Termux 风格终端**：自研 VT 解析 + Canvas 渲染、自研 X11 服务器（C 实现 83KB + Kotlin 双轨 + VNC 服务器 + 虚拟帧缓冲）、SSH/SFTP/SCP 服务器（61KB+56KB+45KB 纯 Kotlin 实现）、Cron 调度、宏录制播放、HTTP 服务器（含 CGI/Basic Auth/Range）、端口转发/SOCKS、网络工具集、进程监控、蓝牙/USB 串口/加密/录屏 API、30+ 系统能力（电池/相机/联系人/指纹/NFC/短信/STT/位置/传感器/电话/媒体…）以 `termx-*` shell 命令暴露，外加 Termux 风格包管理（Repository 索引 + 依赖解析）。**传统 View 体系（非 Compose）**；约 90 个 Kotlin 文件，总代码量 1MB+。

## 4.2 完整架构

```
app/src/main/
├── java/com/termx/app/
│   ├── MainActivity.kt          主界面（Tab 会话、附加键栏、菜单、intent 分发）
│   ├── TermXApp.kt              Application（AssetInstaller/TermXPackageManager 初始化）
│   ├── terminal/                终端核心（9 文件，见 4.3）
│   ├── session/SessionManager.kt 会话表（terminal/display 两类 + 切换）
│   ├── config/                  termux.properties 风格配置（TermXProperties/ConfigDirectory/BellHandler）
│   ├── keyboard/ExtraKeysView.kt 附加键栏（sticky 修饰键 + 自定义布局）
│   ├── x11/                     X11 显示（X11Manager/X11DisplayServer/VirtualFramebuffer/VncServer/X11DisplayView/X11DisplayActivity）
│   ├── pkg/                     Termux 风格包管理（Repository/TermXPackageManager/BootstrapInstaller/PackageInfo）
│   ├── power/                   SSH/SFTP 服务器、Cron、宏、HTTP 服务器、网络/进程/蓝牙/USB/加密/录屏 API、隧道、ProfileManager
│   ├── api/                     TermXApiReceiver（广播分派）+ TermXApi + 14 个领域 API（Battery/Camera/Contact/Fingerprint/Nfc/Notification/Screenshot/Sms/Stt/AppInstall/FilePicker…）
│   ├── receiver/                BootReceiver（开机脚本）/RunCommandReceiver/ShareReceiver/TermXWidgetProvider
│   ├── media/ storage/ telephony/ location/ sensor/  系统能力封装
│   ├── ui/                      SettingsActivity/FileBrowserActivity/FloatingTerminal（悬浮窗终端）/SessionPager/TabAdapter
│   └── utils/                   AssetInstaller/FontManager/FullscreenManager/PreferenceManager/ShellUtils
├── cpp/
│   ├── termx_pty.c (16.9KB)     PTY JNI（forkpty/termios/select 读循环/信号）
│   └── termx_x11.c (83KB)       X11 服务器 JNI（协议/窗口/绘图/framebuffer）
└── assets/                      shell 命令包装（termx-* 脚本）
```

数据流（终端）：`PtySession`（native JNI 或 fallback `Runtime.exec` 双轨）→ `TerminalEmulator.process()`（VT 状态机）→ `TerminalBuffer`（screen+scrollback 5000）→ `TerminalView.onDraw` → `TerminalRenderer.drawTextRun`（Canvas CPU 渲染）；输入：`ExtraKeysView`/硬件键盘/触控 → `PtySession.write`/`sendCtrl` 等 → PTY。X11 数据流独立：`X11Manager.startDisplay` → `termx_x11.c`（native）或 `X11DisplayServer`（Kotlin）→ `VirtualFramebuffer` 像素 → `X11DisplayView`（SurfaceHolder 60fps 拉取）或 `VncServer`（RFB 协议对外）。

## 4.3 文件级功能说明

### terminal/（终端核心，9 文件）

**TerminalEmulator.kt（17.5KB）**
- `enum State`：NORMAL / ESC / CSI / OSC / CHARSET + UTF-8 解码状态（pending 字节）。
- `process(byte)` 主入口；`processNormal()`：BEL（`onBell` 回调，由 BellHandler 消费）、BS/HT/LF/CR、可打印字符、UTF-8 多字节续读。
- `processEsc()`：`7/8`（保存/恢复光标）、`[`→CSI、`]`→OSC、`(`/`)`→CHARSET、`M`（REP）。
- `processCsi()` + `parseParams()`（`:455`）：SGR（含 256 色 `38;5;n`、真彩 `38;2;r;g;b`、隐藏/反显/闪烁）、光标移动族、`J/K` 清屏清行（参数化 eraseBelow/eraseAbove）、`r` 滚动区、`L/M/P/S` 插入删除、`h/l` 模式（`?25` 光标显隐、`?1049` 备用屏、`?47` 切换屏）。
- `processOsc()`：OSC 0/1/2（标题）、OSC 4/10/11（颜色）、OSC 52（剪贴板，`onClipboard` 回调）。

**TerminalBuffer.kt（7KB）**
- `TerminalCell`（char/fg/bg/bold/italic/underline/strikethrough/blink/inverse）。
- `TerminalBuffer`：`screen` + `scrollback`（上限 5000 行，`scrollbackSize` 可配）、`putChar()`（换行/自动换行/光标推进）、`scrollUp/Down`（滚动区溢出进 scrollback）、`clearScreen`/`eraseLine`/`insertLines`/`deleteLines`/`reset`、`selection` 区间。

**TerminalRenderer.kt（6.2KB）**
- `TerminalRenderer`：`textSize`/`typeface`/`fontWidth`/`fontLineSpacing`、`asciiMeasures`（**每字符宽度缓存数组**，`:?`）、`render(canvas, buffer, ...)`、`drawTextRun`/`drawRun`：`Canvas.drawTextRun` + 前景/背景色（inverse 交换）+ 下划线/删除线/粗体模拟 + 光标块绘制。**纯 CPU 文本路径**。

**TerminalView.kt（16.9KB）**
- `View` 子类：持有 buffer/session/emulator；`onSizeChanged` → resize；`Scroller` 滚动 scrollback；光标闪烁 `Runnable`（500ms 间隔 postDelayed，可见性由 `?25` 控制）；触控：单击（开键盘）、长按（进入选择，`startSelection`）、拖拽（扩展选择）、双指缩放（`scaleFactor` 持久化）；`onDraw`：背景填充 → `TerminalRenderer.render` → 光标 → 选择高亮（inverse + 半透明覆盖）；IME：自定义 `BaseInputConnection`（`commitText` 前查 `isCtrlPressed` 等，`sendCtrl`/方向键/`IME_ACTION_SEND` 编码）。

**TerminalSession.kt（4.6KB）**：轻量包装——委托 `PtySession`，加进程监视线程（`isRunning` 轮询）。

**PtySession.kt（17.5KB，会话核心）**
- 双轨设计：`startNativePty()`（`JniPty.nativeCreatePty` → fd）+ `startFallback()`（`Runtime.exec(shell)`，无 TIOCSWINSZ 的简化路径，供无 native 库时兜底）。
- `readNativePtyOutput()`：`nativeRead`（select+read 非阻塞循环）→ `TerminalEmulator.process` → 刷新回调；`monitorNativeExit()`：`nativeIsChildAlive`/`nativeGetExitCode` 轮询。
- 写侧：`write()`（同步 + 失败重试）、`sendCtrl()`（`0x01-0x1A` + `\x1b[27;5;n~` 替代）、`sendEscapeSequence()`、`sendArrowKey()`、`sendFKey()`、`sendSignal()`（native 进程组信号）、`resize()`（`nativeResize` TIOCSWINSZ）、`close()`（`nativeClose` + fallback destroy）。

**JniPty.kt（5.6KB）**：`external fun` 全集——`nativeCreatePty`/`nativeRead`/`nativeWrite`/`nativeIsChildAlive`/`nativeGetExitCode`/`nativeGetMasterFd`/`nativeResize`/`nativeSendSignal`/`nativeClose`。

**JniX11.kt（7.2KB）**：`nativeStartServer`/`nativeStopServer`/`nativeIsRunning`/`nativeGetFramebufferHandle`/`nativeReadFramebuffer`/`nativeTakeScreenshot`/`nativeSendKeyEvent`/`nativeSendPointerEvent`/`nativeResize`/`nativeGetClientCount`/`nativeGetWidth`/`nativeGetHeight`/`nativeGetDisplayNum`。

**TerminalColors.kt（5.6KB）**：5 个主题（Catppuccin/Dracula/Monokai/Solarized/Nord），`getAnsiColor(index)` 映射（0-15 + 256 色 + 真彩直通）。

**cpp/termx_pty.c（16.9KB）**
- `PtyProcess` 结构（master_fd/child_pid/rows/cols/exited/exit_code）。
- `set_nonblocking()`、`setup_child_pty()`：子进程 `setsid()` + `ioctl(TIOCSCTTY)` + `TIOCSWINSZ` + **termios 完整配置**（`cfmakeraw` 变体：`ISIG|ICANON|ECHO` 关闭、`IXON` 关闭、`VINTR`/`VQUIT`/`VSUSP`/`VEOF`/`VEOL` 控制字符表——这是可移植到 torvox 的"Android shell PTY termios 标准配置"）。
- `nativeCreatePty`：`posix_openpt`/`grantpt`/`unlockpt`/`forkpty` 路径 + `execvp("/bin/sh")`（PATH 查找 bash）。
- `nativeRead`：`select()` 超时 + `read`；`nativeWrite`：`write` 循环；`nativeResize`：`TIOCSWINSZ`；`nativeSendSignal`：`kill(-child_pid, sig)`（负 PID = 进程组）；`nativeIsChildAlive`/`nativeGetExitCode`：`waitpid(WNOHANG)`；`nativeClose`：关 fd + kill。

**cpp/termx_x11.c（83KB）**：完整 X11 服务器——协议握手/请求分发（CreateWindow/MapWindow/GC/图形上下文/绘制原语：线段/矩形/弧/文本）、字体加载、事件队列（Expose/Key/Button/Motion）、framebuffer 管理、`nativeGetFramebufferHandle` 暴露像素给 Kotlin 侧。规模大、与终端无关，仅记录不展开。

### session/SessionManager.kt（3.5KB）
- 单例：`terminalSessions`/`displaySessions` 两个 map、`currentSessionId`、`activeSession()`、`switchToSession()`/`switchToDisplay()`、`removeAtPosition()`、`closeAllSessions()`、`renameSession()`、`resizeActiveSession()`；`SessionKind`（TERMINAL/DISPLAY）。

### config/
- `TermXProperties.kt`：termux.properties 风格解析（`parseProperties()` 键值对 → 覆盖默认值；`ensureDefaultConfig()` 首启生成）。
- `ConfigDirectory.kt`：`.termux/`/`.termx/` 目录初始化 + 示例文件（bash.bashrc、colors.properties、termux.properties）。
- `BellHandler.kt`：响铃模式（声音/振动/可视/静音，`onBell` 消费）。

### keyboard/ExtraKeysView.kt（9.9KB outline）
- 附加键栏：ESC/CTRL/ALT/FN **sticky 修饰键**、自定义布局（`extra-keys` 属性解析，可配 `|` 分隔列与 `[]` 包裹 sticky 键）、`sendCtrl`/`sendEscapeSequence`/`sendArrowKey`/`sendFKey`/`sendDelete`/`sendInsert`/`sendHome`/`sendEnd`/`sendPageUp`/`sendPageDown`。

### x11/（8 文件）
- `X11Manager.kt`：显示生命周期（`startDisplay`（native C 或 Kotlin 实现二选一）、`stopDisplay`/`stopAllDisplays`、`getDisplayEnv`（DISPLAY=:N）、`readFramebuffer`、`resizeDisplay`、`handleCommand`（`termx-x11 start/stop/list/resize/status/vnc`）、`DisplayInfo`）。
- `X11DisplayServer.kt`：Kotlin 版 X11 服务器——`ServerSocket` accept、X11 握手（auth cookie）、客户端线程（请求分发）、`setupX11Dirs`（`.X11-unix`/lock 文件）、`handleVncKeyEvent`/`handleVncPointerEvent` 注入。
- `VirtualFramebuffer.kt`：软件帧缓冲——`IntArray` 像素、`putPixel`/`putRect`/`putBitmap`/`fillRect`、`DirtyRect` 脏矩形跟踪、`moveCursor`、`resize`。
- `VncServer.kt`：VNC/RFB 服务器——握手（版本协商/安全/ServerInit）、`protocolLoop`、`readFrameBufferUpdateRequest`/`readKeyEvent`/`readPointerEvent`/`readClientCutText`、`sendFramebufferUpdate`（**脏矩形增量**）。
- `X11DisplayView.kt`：X11 画面显示——`SurfaceHolder` + 渲染线程（60fps 拉 framebuffer 画 Bitmap）、fit 缩放、双指缩放、点击注入指针事件。
- `X11DisplayActivity.kt`：X11 查看器 Activity。

### pkg/（Termux 风格包管理，4 文件）
- `Repository.kt`：包索引——`fetchIndex`（HTTP + 缓存）、`parseIndexJson`、下载（sha256 校验）、`installed` 数据库、`markInstalled`/`markUninstalled`。
- `TermXPackageManager.kt`：`init`/`checkBootstrap`/`update`/`install`（**依赖解析拓扑排序**）/`uninstall`/`upgrade`。
- `BootstrapInstaller.kt`：bootstrap 初始化（目录结构、shell 配置、`pkg` 命令脚本、helper 脚本）。
- `PackageInfo.kt`：包元数据模型。

### power/（核心服务与工具）
- `ssh/SshServer.kt`（61KB）：完整 SSHv2 服务器——协议常量、密钥交换、认证（密码/公钥）、channel 多路复用、session/sftp/scp/direct-tcpip、端口转发、host key 管理。
- `ssh/SshSession.kt`（56KB）：SSH 会话处理——握手、加密、channel 数据流、PTY 连接。
- `ssh/SftpSubsystem.kt`（45KB）：SFTP 协议实现。
- `ssh/HostKeyManager.kt`：host key 生成/持久化；`ssh/SshServerService.kt`：SSH 前台服务。
- `cron/CronScheduler.kt`（36KB）：cron 调度——cron 表达式解析、`AlarmManager` 精确闹钟、`jobs.json` 持久化、前台服务 + WakeLock、`@every_`/`@reboot` 快捷语法、`termx-cron` 命令。
- `cron/CronExpression.kt` / `CronJob.kt` / `CronReceiver.kt`：表达式解析器 / 任务模型 / 闹钟接收。
- `MacroSystem.kt`（14KB）：宏录制/播放——`MacroCommand`（command/delayAfterMs/description/ignoreError）、`Macro`（name/variables/commands/playCount）、`startRecording`/`stopRecording`/`play`（`isPlaying` 中断语义）/`loop`/`list`/`delete`/`rename`/`edit`/`export`（转 shell 脚本 + 变量替换）/`import`；JSON 持久化到宏目录。
- `WebServerApi.kt`（31KB）：HTTP 服务器——`HttpRequest`/`HttpResponse` 模型、`acceptLoop`/`handleClient`（keep-alive 多请求）、GET（目录列表/文件，Range 206 支持）/POST（上传，`MAX_UPLOAD_SIZE` 413）/PUT/DELETE、Basic Auth、CGI handler、路径穿越防护（canonicalPath 前缀检查）、访问日志。
- `NetworkToolsApi.kt`（51KB）：ping（Socket 探测 + RTT 统计）、traceroute（TTL 逐跳）、nslookup（DNS 解析链）、端口扫描（范围/并发）、接口/ARP/路由表读取、HTTP 请求（header 定制）、下载（进度/速度）、TLS 证书链检查（SAN）、子网计算器。
- `ProcessMonitorApi.kt`（42KB）：`/proc` 解析——`ProcessInfo`（stat/status/cmdline/environ）、CPU 采样（两次快照差值）、`top`/`ps`/`pstree`（进程树）/`threads`/`meminfo`/`cpuinfo`（每核频率）。
- `BluetoothApi.kt`（33KB）：蓝牙扫描/配对/传输工具。
- `ProfileManager.kt`（47KB）：配置文件——shell/theme/font/PATH/环境变量/别名、`termx-profile` 命令。
- `WebServerService.kt`/`ScreenRecorderApi.kt`/`ScreenRecorderService.kt`/`UsbSerialApi.kt`/`UsbAttachReceiver.kt`/`EncryptionApi.kt`/`IntentBridgeApi.kt`/`CallLogApi.kt`/`FlashlightApi.kt`/`WakeLockManager.kt`：各自领域能力（录屏前台服务、USB 串口读写、AES/RSA 加密、intent 桥、通话记录、手电、唤醒锁）。
- `tunnel/`：`PortForwarder.kt`/`SocksProxy.kt`/`TunnelConfig.kt`/`TunnelStatistics.kt`：本地端口转发 + SOCKS 代理。

### api/（命令分发与系统能力）
- `TermXApiReceiver.kt`（68KB）：**广播接收器分派器**——`onReceive` 解析 `termx-*` 命令名与参数，分派给 30+ 能力（URL/文件/分享/剪贴板/存储/电池/位置/传感器/电话/媒体/音量/TTS/WiFi/包管理/X11/VNC/相机/短信/指纹/联系人/STT/NFC/通知/文件选择器/截图/应用管理/SSH/Cron/隧道/蓝牙/USB/WebServer/Macro/录屏/加密/意图桥/手电/通话记录/唤醒锁），shell 侧由 `termx` 命令包装转发。
- `TermXApi.kt`：`openUrl`/`openFile`/`shareText`/剪贴板/通知/对话框工具。
- 其余 14 个 API 文件（Battery/Camera/Contact/Fingerprint/Nfc/Notification/Screenshot/Sms/Stt/AppInstall/FilePicker 等）：每个一个 `object` + `execute(context, args, output)` 模式。

### receiver/ / media/ / storage/ / telephony/ / location/ / sensor/
- `BootReceiver.kt`：开机自启执行 `boot` 脚本（`BootCompleted` 广播）。
- `RunCommandReceiver.kt`：外部 App 命令广播（`allowExternalApps` 白名单校验）。
- `ShareReceiver.kt`：文本分享进终端；`TermXWidgetProvider.kt`：桌面小组件。
- `MediaManager.kt`：`MediaPlayer` 播放（网络/本地，播放/暂停/seek/音量，进度回调）。
- `StorageSetup.kt`：`MANAGE_EXTERNAL_STORAGE` 引导、`storage/` 符号链接（`~/storage/shared` 等）、`setupHomeEnvironment`（`.bashrc`/`.profile` 生成）、`SetupResult`。
- `TelephonyInfo.kt`/`LocationProvider.kt`/`SensorProvider.kt`：电话信息、GPS 定位、传感器（加速度/光感/温度）读取。

### ui/ / utils/
- `SettingsActivity.kt`（~220 行）：**程序化 UI**（LinearLayout 手写）——字体大小 SeekBar、主题 Spinner、shell 选择、开关（附加键/光标闪烁/响铃振动/保持亮屏/退出关会话）、滚动缓冲大小 SeekBar。
- `FloatingTerminal.kt`：悬浮窗终端（`TYPE_APPLICATION_OVERLAY`，可展开/收起/拖动/关闭，迷你 TerminalView + 标题栏）。
- `FileBrowserActivity.kt`/`SessionPagerAdapter.kt`/`SessionTabAdapter.kt`：文件浏览、会话 Tab。
- `utils/AssetInstaller.kt`：assets 里的 `termx-*` shell 包装安装到 `$PREFIX/bin`；`FontManager.kt`：字体（`.termux/font.ttf` 优先）；`FullscreenManager.kt`：沉浸全屏；`PreferenceManager.kt`：SharedPreferences（font_size/theme_name/shell_path/scrollback_size/show_extra_keys/back_key_escape/allow_external_apps）；`ShellUtils.kt`：`execute`/`runCommand`/`findBash`/`listFiles`/`readFile`/`writeFile`。

### MainActivity.kt / TermXApp.kt
- `MainActivity.kt`（约 700 行）：`onCreate`——夜间模式、Toolbar、会话 Tab（TabLayout + SessionTabAdapter）、`setupTerminal`/`setupExtraKeys`/`setupKeyboardToggle`/`setupBellHandler`；`handleIntent`（`ACTION_VIEW` 脚本/分享文本/`run_command`/`action` extra）；`attachToTerminalSession`/`attachToDisplaySession`、`openDisplayViewer`、`updateTitle`（OSC 标题）；菜单（新建会话/切换/设置/存储设置/唤醒锁/全屏）；`onKeyDown`（音量键翻页、Ctrl 组合）、`onBackPressed`（`back_key_escape` 属性 → ESC）；`showOpenUrlDialog`/`showSessionSwitcher`/`performStorageSetup`。
- `TermXApp.kt`：`onCreate`——后台线程跑 `AssetInstaller.installIfNeeded()` + `ConfigDirectory.init()` + `TermXPackageManager.init()`。

### 工程文件
- `build.gradle.kts`：`appcompat` + `material` + `constraintlayout` + `security-crypto` + `work-runtime` + `lifecycle`；NDK CMake 编 `termx_pty.c`/`termx_x11.c`。
- `AndroidManifest.xml`（17.5KB）：权限全开（INTERNET/STORAGE/CAMERA/RECORD_AUDIO/BLUETOOTH/SMS/LOCATION/SENSORS/USB 等）+ X11 服务 + 前台服务 + 悬浮窗 + 小组件声明。
- `setup-release.sh`：签名/发布脚本；`worklog.md`：开发日志。

## 4.4 与 torvox 功能对比

| 功能 | torvox 有没有 | 对比结论 |
|---|---|---|
| VT 解析 | ✅ 有（libghostty-vt 完整） | termx 手写状态机覆盖常用子集（含 OSC 52 剪贴板）；torvox 的 ghostty 更完整（含 DECRQSS/鼠标/焦点）。**termx 的 `?1049` 备用屏/`?47` 切换屏行为可作 torvox ghostty 测试对照** |
| 渲染 | ✅ 有（wgpu GPU） | termx Canvas `drawTextRun` + `asciiMeasures` 字符宽度缓存；torvox GPU 渲染性能与质量占优（ADR-0008 GPU-only 决策正确） |
| 滚动缓冲 | ✅ 有 | termx scrollback 5000 行上限（可配）；torvox ghostty scrollback 更完善。torvox 可考虑给 scrollback 行数做设置项（对照 termx/terminator 都有） |
| 选择复制 | ✅ 有 | termx 仅长按拖拽；torvox 完整（词选择/手柄/系统菜单） |
| 附加键栏 | ✅ 有（ModifierBar） | termx ExtraKeysView 支持**自定义布局属性**（`extra-keys` 配置字符串）——torvox 没有布局定制；**可吸收**（见 4.6） |
| 多会话 | ✅ 有（SessionDrawer + Tab） | termx 是 Tab 视图（terminal/display 两类会话同一表管理）；torvox 抽屉更现代。termx 的 `SessionKind` 概念（终端 vs 显示）torvox 不需要 |
| 配置系统 | ⚠️ 无 properties 文件 | termx 有 termux.properties 风格文件配置；torvox 全部走 DataStore UI 设置。**不建议**引入文件配置（增加用户心智负担），除非要兼容 Termux 生态 |
| X11 服务器/VNC | ❌ 无 | torvox 定位不同（Android shell 终端）；83KB C X11 服务器 + Kotlin 双轨实现是巨大工程量，**明确不吸收** |
| SSH/SFTP 服务器 | ❌ 无 | 与 torvox 无关；torvox 的 MCP（Unix socket）已覆盖"外部接入"需求且更安全 |
| Cron | ❌ 无 | torvox 无；如需"定时任务"应交给 shell 侧（bootstrap 里装 cron 包），而非 App 内实现 |
| 宏系统 | ❌ 无 | termx 的宏（命令序列+变量+导出脚本）本质是 shell 脚本生成器；torvox 若要可让用户把"会话命令序列"存为快捷方式，参考其 JSON 模型即可 |
| HTTP 服务器 | ❌ 无 | 与 torvox 无关（有 MCP 提供外部接口） |
| 系统 API 命令集（30+） | ❌ 无 | termx 的广播接收器模式（TermXApiReceiver）是"无 MCP 时代的 Termux API"；torvox 已有 MCP + JNI，**不需要广播方案**。但"电池/网络状态查询"若想进 MCP 工具集，可参考其实现 |
| 包管理 | ✅ 有（BootstrapInstaller） | termx 自研 Repository+依赖解析；torvox 用官方 Termux bootstrap（更好）。不吸收 |
| 开机脚本（BootReceiver） | ❌ 无 | 小功能；torvox 若要"开机启动终端"可参考（低优先） |
| 悬浮窗终端 | ❌ 无 | torvox 无；可作为未来"浮动终端"特性参考（FloatingTerminal.kt 约 180 行，含展开/拖动） |
| 字体切换 | ✅ 有（FontUtils + SystemFonts） | 都有；termx 支持 `.termux/font.ttf` 文件覆盖——torvox 已有系统字体选择，不吸收 |
| 响铃 | ✅ 有 | 都有；termx BellHandler 支持 4 模式，torvox 有通知助手 |
| 前台服务 | ✅ 有 | 都有 |
| PTY termios 配置 | ✅ 有（nix 实现） | termx C 侧的 termios 控制字符表（VINTR/VQUIT/VSUSP/VEOF/VEOL + ISIG/ICANON/ECHO/IXON 关闭）可对照 torvox 的 PTY 设置（见 4.6） |

## 4.5 依赖分析

| 依赖 | 激进程度 | 适用于 torvox？ |
|---|---|---|
| appcompat/material/constraintlayout | 常规（传统 View） | ❌ torvox 是 Compose |
| security-crypto（Keystore 加密） | 常规 | ⚠️ 若 torvox 做"加密的 MCP token 存储"可用 |
| work-runtime | 常规 | ❌ 无后台任务需求 |
| lifecycle | 常规 | ✅ 同栈 |
| 无 Compose/无协程/无 ktor/无 Hilt | 保守 | ❌ 比 torvox 栈落后 |
| NDK CMake（手写 C） | 常规 | ⚠️ torvox 用 Rust + jni crate，更安全 |

结论：**依赖保守、不激进**；全部能力靠自研 Kotlin/C 堆出来（这正是它 1MB+ 代码量的原因）。对 torvox 无依赖面价值，有价值的是个别交互/配置模式。

## 4.6 可吸收到 torvox 的具体内容

1. **附加键栏自定义布局**（`ExtraKeysView.kt` 的 `extra-keys` 属性解析）。
   - 场景：torvox `ModifierBar` 固定布局；用户（尤其 vim/emacs 用户）想要自定义键位。
   - 建议：在 torvox 设置里加一个"修饰键栏布局字符串"（默认 `ESC|CTRL|ALT|TAB|←|→|↑|↓`），复用 termx 的解析约定：`|` 分列、`[]` 包裹 sticky 键、`KEY_NAME` 白名单（ESC/CTRL/ALT/FN/TAB/UP/DOWN/LEFT/RIGHT/HOME/END/PGUP/PGDN/DEL/INS）。实现放 `ModifierBar.kt`（解析布局字符串 → 渲染按钮组），数据放 SettingsRepository。
2. **PTY termios 配置对照**（`termx_pty.c` 的 `setup_child_pty`）。
   - 场景：验证 torvox 的 PTY 是否漏配控制字符。对照清单：`ISIG|ICANON|ECHO|IXON` 关闭、`VINTR=0x03`/`VQUIT=0x1C`/`VSUSP=0x1A`/`VEOF=0x04`/`VEOL=0`、`IUTF8` 开启、`OPOST` 关闭（避免 \n→\r\n 双转换）、`TIOCSCTTY` 需要。torvox 的 `terminal/` PTY 初始化（nix）应与此表等价；可加一个集成测试断言 `stty -a` 输出。
3. **scrollback 行数设置项**：termx（5000 可配）与 terminator（设置项）都有；torvox 的滚动缓冲目前跟随 ghostty 默认——加一个 `SettingsRepository` 键并在 JNI 会话创建时传给 Rust（`setScrollbackLines`），成本低、用户可感知。
4. **悬浮窗终端**（`FloatingTerminal.kt`）：若 torvox 未来做"浮动小终端"，参考其结构（`TYPE_APPLICATION_OVERLAY` + 手势拖动 + 展开/收起 + 迷你 TerminalView）。torvox 的渲染是 Rust 侧 wgpu + TextureView，悬浮窗需要新 surface——列为远期特性，不做优先级。
5. **BellHandler 四模式**：torvox 已有响铃通知；若用户要求"静默/振动"选项，参考其枚举设计（声音/振动/可视/静音）。
6. **Cron 表达式解析器**（`CronExpression.kt`）：不吸收到 App；若 torvox 的 MCP 未来要"定时执行"，应由宿主侧（agent 或 shell cron）承担，避免 App 内维护调度器。

## 4.7 项目文档吸收价值

**低。** README 是特性列表；`worklog.md`（3.3KB）是开发日志（有"为什么选 Canvas 而非自定义字体渲染"等少量决策记录）；无 ADR/架构文档。**可吸收的是"反面教材"**：1MB+ 自研代码（X11 服务器、SSH 服务器、SFTP）在 torvox 语境下属于重复造轮子——torvox 的"vendored ghostty + MCP"路线（少量高质量依赖替代海量自研）被 termx 验证为更优选择。另外 `AndroidManifest.xml` 权限全开（30+ 权限）可作为**安全反面案例**：torvox 应保持最小权限（当前仅终端所需），引以为戒。

---

# 5. 综合结论与吸收优先级

## 5.1 四仓库横向一览

| 维度 | osmosis | redterm | terminator | termx |
|---|---|---|---|---|
| 定位 | Slint 跨端应用骨架 | proot 发行版终端 | Compose 自研终端 | 类 Termux 超级终端 |
| UI 栈 | Slint | 传统 View | Compose | 传统 View |
| 渲染 | wgpu（共享 device）+ Bevy | TextView | Canvas drawTextRun | Canvas drawTextRun |
| VT 解析 | 无（非终端） | 无 | 手写 ~500 行 | 手写 ~500 行 |
| 与 torvox 共性 | wgpu、Rust 分层 | PTY 会话、前台服务 | Compose、DataStore、会话 | PTY、附加键栏、滚动缓冲 |
| 依赖激进度 | **激进**（Slint fork/Bevy） | 保守 | 保守 | 保守 |
| 文档价值 | **高**（12 ADR） | 低 | 低 | 低 |
| 吸收优先级 | 中（模式） | 低（功能） | 中（交互） | 低（个别配置） |

## 5.2 建议吸收清单（按优先级）

**P1（低成本高价值，可直接立项）**
1. 会话元数据持久化 + 重启恢复（terminator `SessionRepository`/`SessionEntry`）——对齐 torvox ADR-0007，Kotlin 侧持久化 + Rust 侧重挂。
2. 附加键栏自定义布局字符串（termx `extra-keys` 解析约定）——`ModifierBar.kt` 扩展。
3. PTY termios 配置对照测试（termx `setup_child_pty` 清单 vs torvox nix 初始化）——集成测试。
4. scrollback 行数设置项（termx/terminator 均有）——SettingsRepository + JNI 参数。

**P2（值得设计后吸收）**
5. MCP `screenshot` 工具（osmosis `extract_texture` framebuffer 读回模式）——为 agent 提供 OCR/验证能力。
6. wgpu 共享 device + 外部纹理导入的架构笔记（osmosis ADR-0005）——torvox 未来 GPU 层扩展（特效/截图）的前置阅读。
7. `unsafe(no_mangle)` 显式标注统一（osmosis apps 入口风格）——Rust 2024 代码卫生。
8. 输出导出到文件（redterm/terminator 均有思路）——小功能补丁。

**P3（远期/条件性）**
9. 指纹锁（redterm AppLock）——若做隐私模式。
10. 悬浮窗终端（termx FloatingTerminal）——远期特性。
11. 开机脚本（termx BootReceiver）——若做"开机自启终端"。
12. proot 发行版支持（redterm 全流程 + init.sh）——产品线扩展，**当前不建议**（与 Termux bootstrap 定位冲突）。

**明确不吸收**：X11/VNC 服务器、SSH/SFTP 服务器、Cron、HTTP 服务器、系统 API 广播集（termx）；Bevy/Slint/webrtc（osmosis）；手写 VT 解析器（terminator/termx，torvox 的 libghostty-vt 已覆盖且更完整）；TextView 渲染（redterm）。

## 5.3 方法论收获（对 torvox 自身的印证）

1. **GPU-only 决策被验证**：三个"普通"终端（redterm/terminator/termx）全部走 CPU 文本渲染且各有性能/字形短板；torvox 的 ADR-0008（wgpu-only）方向正确。
2. **vendored ghostty 决策被验证**：两个自研 VT 解析器（各 ~500 行）都只覆盖常用子集（无 DECRQSS/鼠标/焦点报告）；libghostty-vt 的投入回报明确。
3. **MCP 优于广播 API**：termx 用 68KB 广播接收器实现 30+ 系统能力；torvox 用 ~400 行 tower-mcp 实现同类"外部接入"且类型安全——架构选择被再次印证。
4. **文档差距**：三个 Kotlin 仓库都没有 ADR/架构文档，导致大量设计意图只能从代码反推；torvox 的 `docs/adr/` + `docs/lessons/` 体系应继续坚持，osmosis 的 ADR 写作质量是标杆。
5. **权限最小化**：termx 的 30+ 权限清单是反面案例；torvox 应保持最小权限面。


## termx/reterminal/ply/neotermux deep-v1 增量（2026-08-07 精读确认）

### termx SensorProvider.kt（177 行）
- termux-sensor 等价物（传感器列表/事件监听，SensorData.toFormattedString）——**torvox 无传感器集成**（P3：MCP 未来可加传感器工具，参考 termx + termux-api）

### reterminal（多模块 Compose）
- UpdateManager/MainActivityNavHost——torvox 无更新检查（P3 记录）；其余已覆盖

### ply（单文件 MainActivity）
- 低价值确认（之前 small-repos §2 已覆盖）

### neotermux（Compose 重写）
- ProcessManager/GitScreen/TerminalViewModel——torvox 无进程管理器 UI（P3 记录）
