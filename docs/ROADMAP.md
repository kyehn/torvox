# Torvox 路线图

> **当前阶段**: 阶段 2 — P1.1–P1.6 全部完成 → 下一步 P2.1 回滚缓冲UI

---

## 阶段 0: 基础设施

**主题**: "让它编译"
**退出标准**: `./gradlew assembleDebug` 生成 APK，加载 Rust 库不崩溃。PTY 在 Android 上可生成 shell 进程。

### P0.1 — Rust Workspace 脚手架

**交付物**: Rust workspace 含 `torvox-core`, `torvox-terminal`, `torvox-renderer`, `torvox-gui-android`, `torvox-exec`, `torvox-fuzz`, `torvox-integration-tests`, `torvox-bench`。Cargo.toml 含所有依赖。编译通过。

**详细步骤**:
1. 创建 workspace `Cargo.toml` (edition 2024, resolver 2)
2. 创建 `rust-toolchain.toml` (固定 stable 版本)
3. 创建 `torvox-core/` — `no_std` crate, 骨架 `lib.rs`
4. 创建 `torvox-terminal/` — 骨架 `lib.rs`, 添加 `libghostty-vt 0.1`, `nix 0.31`, `serde`, `postcard 1.1 (dev-dependency)` 依赖
5. 创建 `torvox-renderer/` — 骨架 `lib.rs`, 添加 `wgpu v29`, `cosmic-text 0.19`, `swash 0.2.7`, `guillotiere 0.7` 依赖
6. 创建 `torvox-gui-android/` — 骨架 `lib.rs`, 添加 `boltffi 0.25` 依赖
7. ~~创建 `torvox-bridge-types/`~~ — 类型已合并到 `torvox-gui-android/src/bridge.rs`
8. 创建 `torvox-exec/` — 骨架 `main.rs` (多调用二进制)
9. 创建 `torvox-fuzz/` — 骨架 3 个模糊目标
10. 创建 `torvox-integration-tests/` — 骨架 3 个集成测试文件
11. 创建 `torvox-bench/` — 骨架 3 个基准文件
12. `cargo build --workspace` 通过
13. `cargo clippy --deny warnings` 通过
14. 添加 `Cargo.lock` 到版本控制

**验证**: `cargo build --workspace && cargo clippy --deny warnings`

### P0.2 — 核心类型定义

**交付物**: `torvox-core` 完整类型系统。编译通过。单元测试通过。

**详细步骤**:
1. `cell.rs` — 定义 `Cell`, `Attrs` (含全部 SGR), `Color` (ANSI 256 + TrueColor RGB), `DirtyMask`
2. `grid.rs` — 定义 `Grid`, `DirtyMask` (Vec<u64> 分区位标志), `Scrollback` (环形缓冲)
3. `ansi.rs` — 定义 ANSI 调色板 (0-255), SGR 属性枚举
4. `config.rs` — 定义 `TerminalConfig`, `RenderConfig`, `FontConfig`
5. `selection.rs` — 定义 `Selection` (字符/词/行/块), `SelectionAnchor`
6. `cursor.rs` — 定义 `CursorState` (行, 列, 样式, 可见性)
7. `unicode.rs` — 定义 UnicodeWidth 表, EastAsianWidth 查找
8. `event.rs` — 定义 `TerminalEvent` 枚举 (供跨 crate 事件传递)
9. 所有类型实现 `serde::Serialize`/`Deserialize` (通过 postcard)
10. 所有类型使用 boltffi 注解 (`#[data]`/`#[error]`) (在 `torvox-gui-android/src/bridge.rs` 中)
11. `#[cfg(test)]` 每个类型的单元测试
12. 验证 `no_std` 编译: `cargo build -p torvox-core --target thumbv6m-none-eabi`

**验证**: `cargo test -p torvox-core && cargo build -p torvox-core --target thumbv6m-none-eabi`

### P0.3 — Android 外壳

**交付物**: Gradle 项目含 Kotlin 2.3.21 + Compose BOM 2026.05.00。Kotlin `MainActivity` + Hilt DI。Rust 通过 `scripts/build-android-libs.sh` (cargo-ndk v4) 编译。`System.loadLibrary("torvox_android")` 成功。

**详细步骤**:
1. 创建 `android/` 目录结构 (app, gradle, settings.gradle.kts, build.gradle.kts)
2. 配置 AGP 9.0.1, Kotlin 2.3.21, Compose BOM 2026.05.00
3. 配置 `scripts/build-android-libs.sh` (cargo-ndk v4 交叉编译)
4. 创建 `TorvoxApp.kt` — Application 类 + @HiltAndroidApp
6. 创建 `MainActivity.kt` — 单 Activity, Compose 导航
7. 创建 `TerminalViewModel.kt` — 状态管理骨架
8. 创建 `TerminalScreen.kt` — Compose 屏幕, SurfaceView 占位
9. 创建 `TerminalForegroundService.kt` — FOREGROUND_SERVICE_SPECIAL_USE
10. 配置 `AndroidManifest.xml` (权限, 服务, Activity)
11. `./gradlew assembleDebug` 生成 APK
12. APK 在模拟器上启动, 加载 Rust .so 不崩溃

**验证**: `cd android && ./gradlew assembleDebug && adb install -r app/build/outputs/apk/debug/app-debug.apk && adb shell am start io.torvox/.MainActivity`

### P0.4 — 文档 + CI

**交付物**: AGENTS.md, GitHub Actions CI, 质量门脚本。

**详细步骤**:
1. 将 `AGENTS.md` 移到仓库根目录 (opencode 兼容)
2. 创建 `.opencode/` 配置目录
3. 创建 `.github/workflows/ci.yml`:
   - 触发: PR 到 main
   - 步骤: cargo fmt → cargo clippy → cargo nextest → ./gradlew lint → ./gradlew test
4. 创建 `.github/workflows/nightly.yml`:
   - 触发: 每日 cron
   - 步骤: cargo fuzz (1B 迭代), MIRI, cargo bench, proptest 10K+
5. 创建 `.github/workflows/release.yml`:
   - 触发: 标签 v*
   - 步骤: cargo ndk build → assembleRelease → 签名 → GitHub Release
6. 创建 `scripts/quality-gate.sh`
7. 创建 `rust-toolchain.toml`
8. 更新 `.gitignore` (cargo-ndk v4 输出路径等)

**验证**: `./scripts/quality-gate.sh` 通过 (cargo 部分)

### P0.5 — PTY 验证

**交付物**: `torvox-terminal::pty` 在 Android 上通过 `nix 0.31` crate 成功 forkpty + exec `/system/bin/sh`。验证 W^X 变通方案。

**详细步骤**:
1. 实现 `pty.rs` — `spawn_pty()` 使用 `nix::pty::openpty` + `nix::unistd::fork`
2. 实现 `PtyPair` 结构 (master fd, child pid)
3. 实现 Termios raw mode 配置
4. 实现 `resize()` 通过 `ioctl(TIOCSWINSZ)`
5. 实现 `read()` — 非阻塞读取 PTY master
6. 实现 `write()` — 写入 PTY master
7. 实现 `kill_on_drop` — Drop 时 SIGHUP → SIGCONT → SIGKILL
8. 单元测试: 在 Linux 上验证 forkpty + exec `/bin/sh`
9. Android 集成测试: 在模拟器上验证 `/system/bin/sh`
10. 实现 `torvox-exec` 多调用二进制骨架
11. 验证 W^X 变通: `torvox-exec` 作为符号链接执行

**验证**: `cargo test -p torvox-terminal --test pty_test && adb shell /data/data/io.torvox/torvox-exec ls /`

### P0.6 — boltffi 桥接验证

**交付物**: boltffi 0.25 生成 Kotlin 绑定。Kotlin 可调用 Rust 函数。

**详细步骤**:
1. 定义 `torvox-gui-android/src/bridge.rs` 中的跨边界类型
2. 在 `torvox-gui-android/src/bridge.rs` 中实现 `#[boltffi::export]` 函数
3. 运行 `boltffi pack android` 生成 Kotlin 绑定
5. 在 `TorvoxBridge.kt` 中调用生成的绑定
6. 端到端测试: Kotlin 调用 Rust 函数, 返回值正确

**验证**: `cd android && ./gradlew :app:assembleDebug && adb shell am instrument io.torvox.test/androidx.test.runner.AndroidJUnitRunner`

---

## 阶段 1: 终端引擎

**主题**: "让它画点什么"
**退出标准**: 打开应用 → shell 提示符可见 (GPU 渲染)。键入字符 → shell 回显输出。`ls`, `pwd`, 基本命令工作。

### P1.1 — VT 解析器

**交付物**: `torvox-terminal::parser` — 通过 `libghostty-vt 0.1` crate 的 SIMD 优化 VT 解析器。`Grid` 在输入上变更。所有光标、擦除、SGR 的 CSI 序列。通过 50% vttest。

**详细步骤**:
1. 使用 Ghostty VT Terminal API 处理所有 VT 序列
2. 光标移动: CUU/CUD/CUF/CUB/CUP/CHA/HVP
3. 擦除: ED (0/1/2), EL (0/1/2), ICH, DCH, ECH
4. 行操作: IL, DL, SU, SD
5. SGR: 0-107 (样式+颜色+真彩色)
6. 模式: DECSET/DECRST (2004 括号粘贴, 1006 SGR 鼠标, 2026 同步)
7. 字符集: GL/GR 映射, SS2/SS3
8. Tab 停止: HTS, TBC
9. 滚动区域: DECSTBM
10. 光标保存/恢复: DECSC/DECRC
11. 主/Alt 缓冲区切换: DECALTM
12. 80/132 列模式: DECCOLM
13. 单元测试: 每个序列至少 1 个测试
14. 属性测试: `proptest` 生成随机 VT 序列, 解析器不崩溃
15. vttest 50% 通过

**验证**: `cargo test -p torvox-terminal && cargo nextest run -p torvox-terminal && vttest >50%`

### P1.2 — PTY 会话集成

**交付物**: `torvox-terminal::session` — 完整 PTY 会话。读取 PTY 输出 → VT 解析器 → Grid。写入 → PTY。

**详细步骤**:
1. 实现 `Session` 编排器 (拥有 PtyPair + Grid + Parser)
2. PTY 读取线程: 阻塞 `read()` → `flume bounded channel` → 解析任务
3. VT 解析任务: 从通道读取字节 → `vte::Perform` trait → Grid 变更
4. 脏区域通知: Grid 变更 → `Condvar` → 渲染线程
5. 输入写入: `InputEngine::process()` → VT 转义编码 → PTY `write()`
6. 调整大小: `resize(rows, cols)` → `ioctl(TIOCSWINSZ)` + Grid 调整
7. 进程退出: `waitpid()` → `ProcessExited` 事件
8. 信号处理: SIGWINCH, SIGHUP, SIGCHLD
9. 集成测试: 生成 `echo hello` → Grid 包含 "hello"
10. 回放测试: 录制 PTY 输出 → 重放 → 断言 Grid 状态

**验证**: `cargo test -p torvox-terminal --test session_test`

### P1.3 — 字体管线

**交付物**: `torvox-renderer::font` — `cosmic-text 0.19` 成形, `swash 0.2.7`/`skrifa 0.42` 缩放/光栅化, `guillotiere 0.7` 图集。捆绑 JetBrains Mono Nerd Font。初始仅 ASCII。

**详细步骤**:
1. 实现 `FontPipeline` 结构 (fontdb → cosmic-text → swash/skrifa → etagere)
2. `fontdb` 字体发现: 系统字体 + 捆绑字体
3. `cosmic-text` 文本成形: 字形簇 + 光标位置
4. `skrifa` 字体缩放: 请求像素大小的字形轮廓
5. `swash` 光栅化: 轮廓 → 位图 (包括彩色 emoji)
6. `etagere` 图集打包: 2048×2048 初始, 可扩展到 4096×4096
7. LRU 缓存: 按最后帧访问排序, 64MB 上限 → 驱逐最旧
8. 字形查找: `hash(glyph_id + pixel_size) → 图集 UV`
9. 初始仅 ASCII (95 可打印字符), 预光栅化启动
10. 单元测试: 渲染 "Hello, World!" → 每字符图集条目存在
11. 基准: 字形冷启动 <200ms, 缓存命中 >99%

**验证**: `cargo test -p torvox-renderer && cargo bench -p torvox-renderer font_cache`

### P1.4 — GPU 渲染管线

**交付物**: `torvox-renderer` — wgpu v29 实例。字形图集 → 实例缓冲区 → 四边形绘制。单会话在桌面测试中渲染 (cargo run)。

**详细步骤**:
1. wgpu v29 Instance 创建 (VULKAN 后端, DisplayHandle=None — Vulkan 不使用)
2. wgpu Surface 创建 (从 ANativeWindow 或 winit 窗口)
3. WGSL 着色器: `cell.wgsl` (实例化四边形, 每实例: pos+uv+fg+bg+flags)
4. WGSL 着色器: `cursor.wgsl` (光标块/下划线/竖线)
5. 渲染管线: 单次绘制调用, 实例化四边形
6. 实例缓冲区构建: 遍历脏单元格 → 查找图集 UV → 构建 Instance
7. DirtyMask (Vec<u64> 分区) → 仅处理脏行 → 实例缓冲区 diff
8. wgpu 命令编码 + 提交
9. 帧呈现 (vsync 或 immediate)
10. 桌面测试: `cargo run` 显示空白终端 + 光标
11. 性能: 空闲帧零 GPU 工作, 活跃帧 <16ms

**验证**: `cargo run -p torvox-renderer --example basic_render`

### P1.5 — Android Surface 渲染

**交付物**: wgpu 在 Rust 线程中渲染 → 通过 `SurfaceView` 呈现到 Android Surface。手机上首次可见终端输出。

**详细步骤**:
1. `TerminalSurface.kt` — `SurfaceView` 子类, 实现 `SurfaceHolder.Callback`
2. `surfaceCreated()` → 通过 boltffi 传递 ANativeWindow 到 Rust
3. `torvox-gui-android/src/surface.rs` — `ANativeWindow` → `raw_window_handle::AndroidNdkWindowHandle`
4. wgpu v29 Surface 创建: `instance.create_surface_unsafe(SurfaceTargetUnsafe::RawHandle{...})`
5. 渲染线程: 在独立 Rust 线程运行 wgpu 事件循环
6. 帧回调: `SurfaceView` 的 `Choreographer` 同步 vsync
7. `surfaceChanged()` → 通知 Rust 调整视口
8. `surfaceDestroyed()` → 通知 Rust 释放 Surface
9. 首次可见输出: 启动应用 → 看到 shell 提示符
10. 帧率验证: `adb shell dumpsys gfxinfo io.torvox` >30 FPS

**验证**: 在真机/模拟器上启动应用, 看到 shell 提示符

### P1.6 — 输入处理

**交付物**: `torvox-terminal::keyboard` — Kitty 键盘协议编码器。按键 → Android 硬件键盘 + IME 上的 PTY。触摸 → 鼠标序列。

**详细步骤**:
1. `InputEngine::process_key(KeyEvent)` → Kitty 键盘协议编码
2. 渐进增强: CSI u → push/pop 配置
3. 修饰键: Ctrl/Alt/Shift/Meta/Super
4. 功能键: F1-F20, Home/End/PgUp/PgDn/Insert/Delete
5. IME 输入: 组合文本 → 括号粘贴模式
6. 鼠标协议: X10, VT200, SGR, SGR-pixels
7. 触摸事件: 单击/双击/长按/拖拽 → 鼠标序列
8. 选择手势: 长按开始选择, 拖拽扩展选择
9. Android 事件路由: `TerminalSurface.onKeyDown/onTouchEvent` → boltffi → Rust
10. 输入延迟测试: 按键 → PTY write <2ms

**验证**: 连接硬件键盘, 在应用中键入字符, 看到回显

---

## 阶段 2: 交互式终端

**主题**: "让它可用"
**退出标准**: 可作为日常终端使用。所有基本终端功能工作。设置持久化。

### P2.1 — 回滚缓冲

1. ✅ 环形缓冲回滚 (50K 行默认, 可配置) — Grid.scrollback
2. ✅ 触摸滚动 (fling 手势) — TerminalSurface GestureDetector
3. ✅ 滚动位置指示器 — onScrollChanged callback
4. ✅ 滚动时锁定键盘输入 — isScrolling 状态
5. ⬜ 搜索功能 (在回滚中查找文本)

### P2.2 — 选择

1. ✅ 字符/词/行/块选择模式 — SelectionMode枚举 + ViewModel状态管理
2. ⬜ 放大镜精确选择 (Android Maginifier)
3. ✅ 复制到剪贴板 (Android ClipboardManager) — TerminalViewModel.copySelectionToClipboard
4. ⬜ 检测到链接时打开 URL (Intent.ACTION_VIEW) — TerminalViewModel.openUrl已实现
5. ⬜ OSC 8 超链接悬停高亮
6. ⬜ 语义选择 (OSC 133 Shell 集成)

### P2.3 — 修饰键栏

1. 屏幕修饰键 (Ctrl/Alt/Esc/Tab/方向键)
2. 粘滞模式 (双击锁定修饰键)
3. 可配置布局 (用户自定义键)
4. Nerd Font 字形用于键标签
5. 滑动手势 (左滑=Esc, 右滑=Tab)

### P2.4 — 字体 + 主题

1. 字体大小调整 (捏合缩放, 设置滑块)
2. 字体选择器 (系统字体 + 捆绑字体 + 用户导入)
3. 主题支持 (10+ 内置主题: Dracula, Solarized, Nord, Catppuccin, Tokyo Night, 等)
4. 自定义主题 (TOML 配置文件)
5. 24-bit TrueColor 支持 (vim 语法高亮)

### P2.5 — 设置

1. Jetpack Compose 设置屏幕
2. Shell 选择 (/system/bin/sh, bash, zsh, fish)
3. 字体/字号/行间距配置
4. 配色方案选择
5. 触摸行为配置 (点击=右键/中键/无)
6. 会话管理 (创建/关闭/恢复)
7. DataStore 持久化

---

## 阶段 3: 合规性 & 性能

**主题**: "让它正确"
**退出标准**: 100% vttest 通过率。所有模糊测试目标运行 1B+ 迭代无崩溃。

### P3.1 — vttest 100%

1. 通过所有 vttest 测试用例
2. 修复所有转义序列边缘情况
3. Kitty 键盘协议完全实现 (push/pop/restore)
4. DEC 2026 同步输出完全实现
5. 双宽/双高字符完全实现
6. 矩形区域操作 (DECCRA, DECERA, DECFRA)
7. 选择性擦除 (DECSEL, DECSED)

### P3.2 — 现代扩展

1. OSC 8 超链接 (id= URI 悬停高亮)
2. OSC 52 剪贴板 (系统剪贴板读写, 用户确认)
3. OSC 133 Shell 集成 (prompt/marker/exec 语义)
4. Sixel 图形 (完整 DEC sixel 协议)
5. Kitty 图形协议 (传输/删除/合成/传输+显示)
6. iTerm2 图像协议 (OSC 1337 File=)

### P3.3 — 性能优化

1. PGO (Profile-Guided Optimization) 构建
2. 亚 5ms 输入→像素延迟
3. `find /` 下 120 FPS
4. 图集 LRU 驱驱逐策略优化
5. 空闲内存 <10MB
6. flume 通道调优 (缓冲区大小)
7. 实例缓冲区 diff (仅上传变更单元格)
8. 着色器预热 (首次帧延迟消除)

### P3.4 — 模糊测试 + 安全审计

1. 原始字节模糊测试 (cargo-fuzz, 1B+ 迭代)
2. OSC 转义序列模糊测试
3. UTF-8 边缘情况模糊测试
4. 0 崩溃目标
5. `cargo geiger` 解析器零 unsafe
6. MIRI 通过 (无未定义行为)
7. `proptest` 属性测试 10K+ 案例

---

## 阶段 4: 打磨 & 发布

**主题**: "发布它"
**退出标准**: v1.0.0-beta.1 — GitHub Releases 上的签名 APK。

### P4.1 — 前台服务 + 持久化

1. `FOREGROUND_SERVICE_SPECIAL_USE` 前台服务带通知
2. 跨死亡/OOM 的会话持久化 (postcard 序列化)
3. 自动恢复上次会话
4. Wake lock 管理 (长时间运行命令)

### P4.2 — 无障碍

1. TalkBack 终端内容朗读
2. 操作描述 (AccessibilityNodeInfo)
3. 响铃声音 (BEL → 系统 notification sound)
4. 高对比度模式

### P4.3 — 国际化

1. i18n 框架 (Compose string resources)
2. 中/英/日/韩/德/法 初始翻译
3. RTL 布局支持 (阿拉伯语, 希伯来语)

### P4.4 — MCP 服务器

1. JSON-RPC MCP 服务器 (~7 核心工具)
2. Unix 域套接字, 默认关闭
3. 会话检查, 结构化输出
4. 写入同意提示 (安全)

### P4.5 — Beta 发布

1. GitHub Release (签名 APK + AAB)
2. F-Droid 提交
3. 崩溃报告 (用户同意)
4. 更新日志

---

## 阶段 5: 高级功能 (v1 之后)

| 功能 | 备注 |
|------|------|
| SSH 传输会话 | `russh` crate, SSH 配置管理 |
| 标签管理 | 多会话, 可视化标签栏, 拖拽排序 |
| 分割面板 | 二/四分割, 调整大小手柄 |
| 会话持久化 | 跨设备重启保存/恢复会话状态 |
| 桌面平台 | `torvox-gui-desktop` 使用 `winit 0.30` + 相同 `torvox-core` |
| 插件系统 (WASM) | 会话自动化轻量插件 (wasmtime/wasmi) |
| AI 智能体集成 | 结构化会话协议, 终端中终端 |
| tmux/Zellij 检测 | 自动附加, 会话恢复, tmux 集成 |
| GPU 计算着色器渲染 | Ferrum 模式: 整个网格在 GPU 上, CPU 零参与 |
| 串口/调试控制台 | UART, JTAG, 蓝牙 SPP |
| Mosh 协议 | 移动 Shell (UDP, 状态同步) |
| Reticulum 传输 | 去中心化网络终端 |

## 排除项

- **包管理器** (Termux 生态系统)。那是另一个产品。
- **SSH 密钥管理** (发布时)。仅传输会话在阶段 5。
- **PRoot/Linux 容器** (Haven 的模型)。Torvox 是终端模拟器，不是瘦客户端 OS。
- **文件浏览器/编辑器** (Haven 的模型)。终端就是界面。
- **桌面环境** (Haven 的 Wayland)。Torvox 渲染终端输出。
- **云同步**。本地优先。需要云端时 SSH 到你的服务器。
