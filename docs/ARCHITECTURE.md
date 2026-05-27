# Torvox 架构设计

## 概览

Torvox 是分层架构的终端模拟器，Rust 核心引擎 + Kotlin/Compose Android 外壳。

```
┌──────────────────────────────────────────────────────────────────────┐
│ torvox-android (Kotlin)                                              │
│ ┌─────────────────┐ ┌────────────────┐ ┌──────────────────────┐     │
│ │ TerminalActivity │ │ TerminalView   │ │ SettingsActivity     │     │
│ │ (Lifecycle)      │ │ (SurfaceView  │ │ (DataStore, Theme)   │     │
│ │ ForegroundService│ │  Host)        │ │ Session Management   │     │
│ └────────┬────────┘ └───────┬────────┘ └──────────┬───────────┘     │
│          │                  │                      │                 │
│ ┌────────┴──────────────────┴──────────────────────┴───────────┐    │
│ │ boltffi Bridge (torvox-gui-android)                       │    │
│ └───────────────────────────────────────────────────────────────┘    │
└──────────────────────────────────────────────────────────────────────┘
         │
         ▼
┌──────────────────────────────────────────────────────────────────────┐
│ Rust Engine                                                          │
│ ┌─────────────────────────────────────┐ ┌─────────────────────────┐  │
│ │ torvox-terminal                     │ │ torvox-renderer          │  │
│ │ Session PTY Parser Terminal Keyboard│ │ FontPipeline GpuContext  │  │
│ └──────────┬──────────────────────────┘ └──────────┬──────────────┘  │
│            │                                      │                  │
│            └──────────┬───────────────┬────────────┘                  │
│                      ▼                ▼                              │
│            ┌────────────────────┐ ┌──────────────┐                   │
│            │ torvox-core (no_std)│ │ torvox-exec  │                   │
│            └────────────────────┘ └──────────────┘                   │
└──────────────────────────────────────────────────────────────────────┘
```

## Crate 架构

```
torvox/
├── Cargo.toml              # Workspace 清单
├── rust-toolchain.toml     # 固定 Rust stable 版本
├── docs/                   # 文档
│
├── torvox-core/             # [no_std] 核心类型
│   ├── src/
│   │   ├── lib.rs          # crate 根, no_std 声明
│ │ ├── cell.rs # Cell, Attrs (含全部 SGR), Color (ANSI 256 + TrueColor), DirtyMask (Vec<u64> 分区位标志, 支持任意行数)
│ │ ├── grid.rs # Grid, Scrollback 环形缓冲, DirtyMask 集成
│   │   ├── line.rs         # Line 结构, 属性跨度编码
│   │   ├── ansi.rs         # ANSI 调色板, SGR 属性枚举
│ │ ├── config.rs # TerminalConfig, Shell (SystemDefault|Custom(String)), RenderConfig, FontConfig
│   │   ├── selection.rs    # Selection 类型 (字符/词/行/块), SelectionAnchor
│   │   ├── cursor.rs       # CursorState (位置, 样式, 可见性)
│   │   ├── unicode.rs      # UnicodeWidth 表, EastAsianWidth 查找
│   │   └── event.rs        # TerminalEvent 枚举 (供跨 crate 事件传递)
│   └── Cargo.toml
│
├── torvox-terminal/         # 终端仿真引擎
│   ├── src/
│   │   ├── lib.rs          # crate 根
│   │   ├── pty.rs          # PTY 会话 (nix 0.31 forkpty — 非 portable-pty)
│   │   ├── parser.rs       # Ghostty VT 集成 (libghostty-vt 0.1 crate)
│   │   ├── terminal.rs     # Terminal 状态机 (Ghostty VT Terminal + RenderState)
│   │   ├── keyboard.rs     # Kitty 键盘协议编码器 + VT 传统编码 + 鼠标 SGR
│   │   └── session.rs      # Session 编排器 (线程管理, 通道)
│   └── Cargo.toml
│
├── torvox-renderer/         # GPU 渲染
│   ├── src/
│   │   ├── lib.rs          # crate 根
│   │   ├── font.rs         # 字体管线 (cosmic-text 0.19 + swash 0.2.7 + guillotière)
│   │   └── gpu.rs          # wgpu v29 GPU 管线 (Instance/Device/Queue/Surface)
│   ├── shaders/
│   │   ├── cell.wgsl       # 单元格着色器 (实例化四边形)
│   │   └── cursor.wgsl     # 光标着色器 (纯色矩形)
│   ├── examples/
│   │   └── basic_render.rs # 桌面渲染示例 (winit + wgpu)
│   └── Cargo.toml
│
├── torvox-gui-android/      # Android GUI 桥接
│   ├── src/
│   │   ├── lib.rs          # crate 根
│ │ ├── bridge.rs # boltffi 导出: TorvoxBridge, BridgeCell(+BridgeAttrs), Shell(Enum), TerminalConfig, TerminalEvent(8变体), TerminalError
│   │   └── surface.rs      # wgpu → Android Surface 共享 (P1.5)
│   └── Cargo.toml
│
├── torvox-exec/             # W^X 多调用二进制
│   ├── src/
│   │   └── main.rs         # 根据 argv[0] 执行对应命令
│   └── Cargo.toml
│
├── torvox-fuzz/             # 模糊测试 (pending crate; targets 待建)
│   ├── src/
│   │   └── lib.rs          # 存根
│   └── Cargo.toml
│
├── torvox-integration-tests/ # 跨边界集成测试 (pending crate)
│   ├── src/
│   │   └── lib.rs          # 存根
│   └── Cargo.toml
│
├── torvox-bench/            # 基准测试 (empty)
│   ├── src/
│   │   └── lib.rs          # 存根
│   └── Cargo.toml
│
├── android/                # Kotlin Android 应用
│   ├── app/
│   │   ├── src/main/java/io/torvox/
│   │   │   ├── TorvoxApp.kt                # Application 类 (Hilt)
│   │   │   ├── MainActivity.kt            # 单 Activity 宿主
│   │   │   ├── TerminalViewModel.kt       # 会话状态管理 (StateFlow)
│   │   │   ├── ui/
│   │   │   │   ├── TerminalScreen.kt      # 主 Compose 屏幕
│   │   │   │   ├── TerminalSurface.kt     # SurfaceView 宿主
│   │   │   │   ├── ExtraKeysBar.kt        # 屏幕修饰键
│   │   │   │   ├── SettingsScreen.kt      # 配置 UI
│   │   │   │   └── theme/
│   │   │   ├── service/
│   │   │   │   └── TerminalForegroundService.kt  # FOREGROUND_SERVICE_SPECIAL_USE
│   │   │   └── bridge/
│   │   │       └── TorvoxBridge.kt         # boltffi 生成绑定
│   │   └── build.gradle.kts
│   ├── gradle/
│   ├── settings.gradle.kts
│   └── build.gradle.kts
│
└── Cargo.lock
```

## 技术版本锁定

| 技术 | 版本 | 备注 |
|------|------|------|
| Rust | stable (rust-toolchain.toml) | Edition 2024 |
| wgpu | 29 | `InstanceDescriptor.display` 为 Option (Vulkan 后端不使用); Surface 创建: `SurfaceTarget::DisplayAndWindow` 或 `SurfaceTarget::Window` |
| cosmic-text | 0.19 | 文本成形, COLR/COLRv1 emoji |
| swash | 0.2.7 | 光栅化 (via zeno), 缩放功能已完全迁移到 skrifa |
| skrifa | 0.42 | Google 字体缩放库 (swash 0.2.x `scale` feature 的内部依赖, 无需单独声明) |
| guillotière | 0.7 | 货架打包图集 |
| libghostty-vt | 0.1 | Ghostty VT 解析器 (SIMD优化, VT100-520) |
| nix | 0.31 | Unix API (forkpty, openpty, ioctl) |
| libc | 0.2 | C 语言 FFI (PTY syscall 支持) |
| serde | 1 | 序列化框架 (可选, via features) |
| bytemuck | 1 | 安全字节 reinterpret (GPU instance 数据) |
| bitflags | 2 | 位标志类型 |
| flume | 0.11 | 无锁 SPSC 通道 (PTY→解析器) |
| raw-window-handle | 0.6 | 仅 gpu.rs 内部用于 Android Surface 创建; 非公开 API 依赖 |
| boltffi | 0.25 | 类型安全 Rust↔Kotlin 绑定; 所有 boltffi 类型在 gui-android/src/bridge.rs (单一导出位置) |
| cargo-ndk | v4 | **重大变更**: v4 重写了 CLI, 与 v3 不兼容 |
| postcard | 1.1 | 测试序列化 (dev-dependency) |
| thiserror | 2 | 错误类型派生 |
| pollster | 0.4 | 阻塞运行时, 用于 wgpu 同步初始化 (gpu.rs 内部使用) |
| proptest | 1.11 | 属性测试 |
| cargo-fuzz | 0.13 | 模糊测试 (libFuzzer) |
| cargo-nextest | 0.9 | 增强测试运行器 |
| Kotlin | 2.3.21 | K2 编译器稳定 (2.4.0 ~2026 年 6 月) |
| Compose BOM | 2026.05.00 | Material 3 + Compose UI |
| AGP | 9.0.1 | Android Gradle Plugin |
| Hilt | 2.59.2 | 依赖注入 (需 AGP 9.0+) |
| NDK | r29 | Android NDK |
| compileSdk | 36 | Android 16 编译目标 |
| targetSdk | 36 | Android 16 |
| minSdk | 33 | Android 13 (Vulkan 1.3 起始) |

> **已弃用/移除**: `rust-android-gradle 0.9.6` — AGP 9.0 移除了 `AppExtension`, 不兼容。改用 `scripts/build-android-libs.sh` + `cargo-ndk v4`。`glyphon 0.11` — 参考实现, 未使用, 已移除。

### 关键技术修正

| 原方案 | 修正 | 原因 |
|--------|------|------|
| `bincode` | → `postcard 1.1` | bincode 3.0.0 被作者故意破坏 (RUSTSEC-2025-0141), postcard 仅用于测试 |
| `swash` 缩放 | → `skrifa 0.42` | swash 缩放已完全迁移到 skrifa; swash 仍负责光栅化 (via zeno) |
| `portable-pty` | → `nix` crate forkpty() | portable-pty 不支持 Android |
| `AHardwareBuffer` | → `SurfaceView` | wgpu 原生支持 Surface, 零复制, 游戏引擎标准模式 |
| minSdk 26 | → minSdk 33 | Vulkan 1.3 从 API 33 起原生支持 |
| `rust-android-gradle 0.9.6` | → `scripts/build-android-libs.sh` | AGP 9.0 移除了 AppExtension, rust-android-gradle 不兼容。用 cargo-ndk v4 直接交叉编译 |
| `torvox-bridge-types` boltffi | → 类型合并到 `gui-android/src/bridge.rs` | boltffi 库模式仅允许一个导出位置; 跨 crate derive 导致 Kotlin 生成重复脚手架 |
| `TerminalError.message` | → `TerminalError.detail` | Kotlin `Throwable.message` 冲突, boltffi Error 枚举字段名不能为 `message` |
| `glifo` | 不采用 (待 1.0) | Linebender 新项目, 未稳定; swash 0.2.x 长期依赖稳定 |

### 已知风险

| 风险 | 影响 | 缓解 |
|------|------|------|
| **COLRv1 emoji** | cosmic-text 可能无法正确渲染 COLRv1 彩色 emoji (swash 0.2.x 已内部集成 skrifa 缩放, 但 cosmic-text 集成未验证) | 捆绑遗留 CBDT 字体作为回退 |
| **Mali GLES 驱动崩溃** | `glPushDebugGroup` SIGSEGV 影响预算三星设备 | 使用 `DISCARD_HAL_LABELS` 标志, 优先 Vulkan 后端 |
| **Android 16 前台服务** | `FOREGROUND_SERVICE_SPECIAL_USE` 需要 Play Store 理由 | 正确声明前台服务类型 + 提交理由 |
| **wgpu 29 Surface API** | `InstanceDescriptor.display` 为 Option, `SurfaceTarget` 两种变体 | Android/Vulkan: display=None, 使用 `SurfaceTarget::Window` 仅传 WindowHandle |
| **cargo-ndk v4** | CLI 重写, 与 v3 脚本不兼容 | 更新所有构建脚本到 v4 语法 |

## PTY 实现

详细决策和完整实现代码见 `docs/ADR/004-pty-implementation.md`。摘要:

- **不使用 `portable-pty`** — 不支持 Android
- **直接使用 `nix` crate** — `openpty()` + `fork()` 实现 PTY
- **W^X 变通** — `torvox-exec` 多调用二进制模式 (Termux 验证)

### W^X 变通方案

Android 10+ 限制非系统库的 `exec()`。Torvox 使用 Termux 验证的模式：

1. **多调用二进制**: `torvox-exec` 单一二进制文件，所有命令作为符号链接指向它
2. `torvox-exec` 根据 `argv[0]` 的 `file_name()` 确定调用者身份
3. 以正确参数执行 `exec()`
4. 这绕过了 Android 的 W^X 限制，因为二进制文件本身由系统加载器映射为可执行

## 线程模型

```
┌──────────────────────────────────────────────────────────────────┐
│ 会话 1                                                           │
│ ┌──────────────┐  ┌──────────────┐  ┌────────────────────┐      │
│ │ PTY Reader   │─►│ VT Parser    │─►│ Grid              │      │
│ │ (block read) │  │ (async task) │  │ (Arc<Mutex>)       │      │
│ └──────────────┘  └──────────────┘  └────────┬───────────┘      │
│                                               │                  │
│ ┌──────────────┐  ┌──────────────┐           │                  │
│ │ Input Writer │◄─│ Input Encoder│           │                  │
│ └──────────────┘  └──────────────┘           │                  │
│                                               │                  │
│ ┌──────────────┐  ┌──────────────┐           │                  │
│ │ Process      │  │ OSC Handler  │           │                  │
│ │ Waiter       │  │ (52/8/133)   │           │                  │
│ └──────────────┘  └──────────────┘           │                  │
└───────────────────────────────────────────────┼──────────────────┘
                                                │
┌───────────────────────────────────────────────┘
▼
┌─────────────────────┐
│ 渲染线程             │
│ (单线程, 跨会话共享) │
│                     │
│ 轮询所有 Grid      │
│ 构建实例缓冲区      │
│ 提交到 wgpu v29    │
└──────────┬──────────┘
           │
┌──────────▼──────────┐
│ GPU (wgpu Vulkan)   │
│ 实例化四边形        │
│ → 帧缓冲           │
│ → Android Surface   │
└──────────┬──────────┘
           │
┌──────────▼──────────┐
│ Android UI 线程      │
│ Compose             │
│ SurfaceView         │
│ 输入事件 → Rust     │
└─────────────────────┘
```

### 空闲线程数 (1 会话)

| 线程 | 数量 | 用途 |
|------|------|------|
| PTY Reader | 1 | 阻塞读取 PTY fd。无数据时休眠。 |
| 渲染线程 | 1 | wgpu 设备+队列。空闲轮询 (指数退避 16→250ms)。 |
| Android Main | 1 | Compose 组合+布局。无输入时零工作。 |
| wgpu 内部 | 1-2 | GPU 驱动管理。 |
| **总计** | **4-5** | 对比: Termux 3 线程, Warp ~15 线程空闲。 |

## 数据流

### 渲染路径 (最延迟关键)

```
PTY write → kernel → read() on PTY fd
→ raw bytes [Vec<u8>]
→ flume bounded channel (lock-free, bounded 64KB)
→ Ghostty VT Terminal (SIMD优化 VT 解析)
→ Grid.apply(Delta)
→ DirtyMask (Vec<u64> 分区位标志, 任意行数)
→ RenderThread wakes (via Condvar)
→ For each dirty line:
    For each cell:
      Lookup glyph in Atlas
      Atlas miss → cosmic-text shape + swash/skrifa render → guillotiere pack
    → Atlas upload (if new glyph)
    → Instance { position, uv, fg, bg, flags }
→ wgpu submission (1 draw call, 实例化四边形)
→ wgpu present
→ Android SurfaceView 显示
```

**目标延迟**: <5ms 从 PTY 写入到像素可见。

### 输入路径

```
Touch/Key event → Android InputReader → Compose
→ TerminalSurface.onKeyDown/onTouchEvent
→ boltffi call → torvox-gui-android
→ InputEngine.process(KeyEvent/TouchEvent)
→ VT escape sequence encoding (Kitty protocol or CSI-u)
→ PTY write(fd, encoded_bytes)
```

**目标延迟**: <2ms 从按键事件到 PTY 写入。

### Android Surface 创建 (wgpu 29)

```rust
// wgpu 29: InstanceDescriptor.display 是 Option
// Vulkan 后端不使用 DisplayHandle, Android 上可省略
let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
    backends: wgpu::Backends::VULKAN,
    ..Default::default() // display = None, Vulkan 不需要
});

// Surface 创建: 两种模式
// 模式 1 — 传 DisplayAndWindowHandle (通用, 但 Vulkan 忽略 display)
// 模式 2 — 仅传 WindowHandle (如果 display 已传给 Instance)
// Android/Vulkan 推荐模式 2:
let surface = unsafe {
    instance.create_surface_unsafe(
        wgpu::SurfaceTargetUnsafe::RawHandle {
            raw_window_handle: android_raw_window_handle,
            raw_display_handle: Some(raw_display_handle),
        }
    )?
};
```

Kotlin 侧通过 `SurfaceView.getHolder().getSurface()` 传递 ANativeWindow 到 Rust。

## 关键接口

### Rust: `Session` 编排器

```rust
pub struct Session {
    pty: PtyPair,
    terminal: TerminalState,
    parser: VtParser,
    output_rx: Receiver<Vec<u8>>,
    output_notify: Arc<(Mutex<bool>, Condvar)>,
    exited: Arc<AtomicBool>,
    reader_handle: Option<std::thread::JoinHandle<()>>,
    wait_handle: Option<std::thread::JoinHandle<()>>,
}
```

## 安全模型

1. **Rust 内存安全覆盖所有 PTY/VT 代码**: boltffi 桥接是唯一 `unsafe` 边界
2. **OSC 52 剪贴板**: 用户确认 (Android toast + 接受)
3. **PTY 隔离**: 每个会话在自己的进程组中；`kill_on_drop` 语义
4. **默认无网络**: MCP 服务器仅限本地回环，默认禁用
5. **Android 沙盒**: 标准 Android 应用沙盒，无 root 要求
6. **前台服务**: `FOREGROUND_SERVICE_SPECIAL_USE` 类型 (Android 16 要求)

## 与现有项目的架构对比

| 维度 | Termux | Haven | Torvox |
|------|--------|-------|-------|
| **渲染** | View + Canvas (Java) | Compose + Canvas (Kotlin) | wgpu v29 GPU (Rust) |
| **VT 解析器** | 手写 Java FSM (2617行) | libvterm (C/JNI) | libghostty-vt 0.1 (SIMD, Rust FFI) |
| **FFI 边界** | 最小 JNI (仅 PTY) | libvterm + IronRDP + rclone + PRoot | boltffi 0.25 类型安全绑定 |
| **线程模型** | 3 线程 + Handler | mutex 保护 + 协程 | 专用解析线程 + flume lock-free 通道 |
| **脏区域跟踪** | 无 (全屏重绘) | Compose 管理 | DirtyMask (Vec<u64> 分区位标志, 任意行数) |
| **字形缓存** | 无 | 无 | guillotière 0.7 GPU 图集 |
| **内存模型** | Java 环形缓冲 (64KB) | C libvterm + Kotlin 复制 | Rust 所有权, flume SPSC 零拷贝通道 |
| **序列化** | Java Serializable | C struct | postcard 1.1 (测试用) |
| **前台服务** | 普通 Service | 普通 Service | FOREGROUND_SERVICE_SPECIAL_USE (Android 16) |
