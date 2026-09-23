# 深度研究：4 个小参考仓库合并文档（ghostling / ply / onecode / cpmdroid）

> 研究日期：2026-08-06
> 克隆位置：`repositories/refs/{ghostling,ply,onecode,cpmdroid}`（均 depth 1 完整克隆；onecode 的 `terminal-core` 子模块为独立仓库，已通过 GitHub API/raw 远端读取）
> 对比基准：torvox（Android 终端，Kotlin Compose + Rust native + wgpu + libghostty-vt，单 `native` crate，JNI 直连无 boltffi）
> 说明：onecode `terminal-core` 与 cpmdroid `MainActivity.kt`/`emu_io_android.cpp` 中个别超大文件（44–80KB）经远端抓取，中段受输出长度限制，文中已标注"（远程读取，中段按上下文推断）"；其余文件均逐行完整阅读。

---

## 0. 总览

| 仓库 | 定位 | 与 torvox 同层技术 | 总体价值 |
|------|------|-------------------|---------|
| ghostling | libghostty C API 最小终端演示（单 C 文件 + Raylib） | libghostty-vt（同一 VT 内核） | **高**：官方 API 用法教科书，effects/kitty graphics 细节可直接映射到 torvox 的 Rust 绑定 |
| ply | Rust Android 终端 REPL 原型 + proot | 无（纯 std） | **低**：非真实终端；仅环境变量设置模式可参考 |
| onecode | 模块化 Android 终端（Compose UI + 自研 ANSI 引擎 + proot Ubuntu + AIDL 服务） | 自研 VT 解析/Canvas 渲染（torvox 用 libghostty-vt + wgpu 取代） | **中高**：会话管理、AIDL 服务化、初始化状态机、交互式输入检测等**架构层**经验 |
| cpmdroid | Z80/CP/M 模拟器（qkz80 + RomWBW HBIOS + JNI + 自绘 View 终端） | VT100 最小解析 + 自绘 Canvas（与 torvox 渲染路线完全不同） | **低**：终端部分仅滚动回退/键盘遮挡处理思路可参考 |

---

## 1. ghostling（ghostty-org/ghostling）— 高价值

### 1.1 项目定位与完整架构

Ghostty 官方仓库下的演示项目，展示用 **libghostty C API**（`ghostty/vt.h`）在**单个 C 文件**（`main.c`，1604 行）里构建一个最小可用终端。窗口与渲染用 **Raylib**（2D 软件字形绘制，非 GPU 直渲），**单线程**主循环（libghostty-vt 本身支持多线程）。

数据流（与 torvox 同构）：

```
pty master fd（forkpty，非阻塞）
  → pty_read() 每帧 drain → ghostty_terminal_vt_write()（VT 解析 + 终端状态）
  → ghostty_render_state_*（渲染状态快照，脏区域跟踪）
  → render_terminal() 逐 cell 读出 grapheme/颜色/样式 → DrawTextEx 绘制
  → 键盘/鼠标 → ghostty_key_encoder / ghostty_mouse_encoder 编码 → pty_write()
```

架构分层：
- **PTY 层**：`pty_spawn` / `pty_write` / `pty_read`（main.c:43/103/132）
- **输入层**：raylib 键鼠事件 → libghostty key/mouse encoder → PTY（main.c:164–570）
- **渲染层**：RenderState API 逐 cell 快照渲染 + Kitty graphics 图片层（main.c:662–1018）
- **效果回调层（effects）**：`write_pty`/`size`/`device_attributes`/`xtversion`/`title_changed`/`color_scheme`（main.c:1104–1189）
- **主循环**：resize → focus 上报 → drain pty → reap 子进程 → scrollbar → 输入 → 渲染（main.c:1431–1590）

### 1.2 文件功能说明

#### main.c（1604 行，单文件全部逻辑）

| 行号 | 符号 | 功能 |
|------|------|------|
| 21–24 | `#include "font_jetbrains_mono.h"` | 编译期嵌入 JetBrains Mono 字体（由 CMake 的 `bin2header.cmake` 生成头文件），免运行时定位字体 |
| 43–96 | `pty_spawn(child_out, cols, rows, cell_width, cell_height)` | `forkpty()` 建 pty 对；子进程按 `$SHELL` → `getpwuid()` 的 `pw_shell` → `/bin/sh` 顺序选 shell，`setenv("TERM","xterm-256color")` 后 `execl`；父进程把 master fd 设 `O_NONBLOCK`；winsize 带像素尺寸（`ws_xpixel/ws_ypixel`，供 XTWINOPS 像素查询） |
| 103–117 | `pty_write(pty_fd, buf, len)` | 非阻塞 best-effort 写：EINTR 重试、短写推进、EAGAIN/错误静默丢弃（终端标准背压行为） |
| 120–124 | `enum PtyReadResult` | `PTY_READ_OK / PTY_READ_EOF / PTY_READ_ERROR` |
| 132–156 | `pty_read(pty_fd, terminal)` | 循环 `read()` 4KB 块喂 `ghostty_terminal_vt_write`；EAGAIN→OK、EINTR→重试、**EIO→按 EOF 处理**（Linux 上 slave 关闭常返回 EIO 而非 0，关键细节） |
| 164–208 | `raylib_key_to_ghostty(rl_key)` | raylib 键码 → `GhosttyKey` 映射（字母/数字/F 键区间连续映射 + switch） |
| 211–223 | `get_ghostty_mods()` | 读 raylib 修饰键状态 → `GhosttyMods` 位掩码（Shift/Ctrl/Alt/Super） |
| 229–255 | `raylib_key_unshifted_codepoint(rl_key)` | 返回按键的**未加修饰** Unicode 码点（Kitty keyboard protocol 按物理键识别所需；US 布局假设） |
| 256–289 | `utf8_encode(cp, out[4])` | 码点 → UTF-8 手写编码 |
| 290–306 | `raylib_mouse_to_ghostty(rl_button)` | 鼠标键 → `GhosttyMouseButton` |
| 307–322 | `mouse_encode_and_write(...)` | 调 `ghostty_mouse_encode` 并写入 pty |
| 323–448 | `handle_mouse(...)` | 鼠标上报：仅当应用启用 mouse tracking（`ghostty_terminal_mode_get` 查 `GHOSTTY_MODE_MOUSE_*`）才编码发送；**焦点上报（DECSET 1004）同样先查 mode 再发 CSI I/O**——避免向未请求的程序注入转义序列 |
| 449–571 | `handle_input(...)` | 键盘：先 `ghostty_key_event_new` 构造（key + mods + unshifted codepoint），`ghostty_key_encoder` 编码（kitty 协议在终端启用时自动选择），无编码结果时回退到 codepoint 直接写 |
| 572–632 | `handle_scrollbar(...)` | 滚动条命中测试 + 拖拽（thumb 拖动持续调整 viewport）；**在鼠标转发之前消费**，防止滚动条点击漏进 vim/tmux |
| 636–643 | `defer_unload_texture(tex)` / `flush_deferred_textures()` | 渲染中创建的 Raylib 纹理延迟到帧末统一卸载（避免在绘制循环中销毁纹理） |
| 662–777 | `render_kitty_images(terminal, kitty_gfx, placement_iter, layer, ...)` | Kitty graphics 渲染：`ghostty_kitty_graphics_placement_iterator` 遍历 placement → viewport 位置/图片尺寸/源矩形/子格偏移 → 临时纹理 `DrawTexturePro`；支持 `LAYER_BELOW_BG` 与默认层 |
| 798–1018 | `render_terminal(render_state, row_iter, cells, font, ...)` | 核心渲染：`ghostty_render_state_colors_get` 取调色板 → 行/格迭代器逐 cell：`GRAPHEMES_LEN==0` 只画背景；读 grapheme 码点（上限 16）、`FG_COLOR/BG_COLOR`（返回 INVALID_VALUE 表示无显式色）、`GhosttyStyle`；inverse 交换前后景；italic 用 x 偏移模拟剪切；**bold 用右移 1px 二次绘制模拟"伪粗体"**；每行渲染后清除 dirty 标志（`ROW_OPTION_DIRTY`），最后 `RENDER_STATE_OPTION_DIRTY=false` |
| 1026–1045 | `log_build_info()` | 启动时打印 libghostty-vt 编译信息（SIMD 开关、优化模式） |
| 1054–1090 | `decode_png(userdata, ...)` | 注册给 libghostty 的 PNG 解码器回调（kitty graphics 用）：Raylib `LoadImageFromMemory` 解码 → 转 R8G8B8A8 → `ghostty_alloc` 拷贝，输出 RGBA + 尺寸 |
| 1092–1098 | `struct EffectsContext` | 全局 effect 回调上下文（pty_fd、cell 尺寸、cols/rows） |
| 1104–1110 | `effect_write_pty(...)` | **关键 effect**：终端需回写应用的响应（DSR、模式查询、DA 等）都经此回调写回 pty——**不实现则 vim/tmux 探测能力时挂起** |
| 1114–1124 | `effect_size(...)` | 响应 XTWINOPS 尺寸查询（CSI 14/16/18 t），报 rows/cols/cell 像素 |
| 1129–1151 | `effect_device_attributes(...)` | 响应 DA1/DA2/DA3：报 VT220 级 + 132 列/选择性擦除/ANSI 色 |
| 1154–1159 | `effect_xtversion(...)` | 响应 `CSI > q`，返回应用名 "ghostling" |
| 1163–1177 | `effect_title_changed(...)` | OSC 0/2 标题变化 → 更新窗口标题 |
| 1182–1189 | `effect_color_scheme(...)` | `CSI ? 996 n` 深色模式查询：Raylib 无法查询，直接返回 false 忽略 |
| 1195–1604 | `main()` | 初始化（HiDPI 标志、字体、terminal/encoders/render state/kitty iterator 一次性创建并复用）→ 主循环（见 1.1）→ 子进程退出后打横幅（含退出码/信号）→ 逆序释放全部句柄 |

#### CMakeLists.txt（76 行）

| 行号 | 内容 |
|------|------|
| 13–14 | `find_program(zig REQUIRED)` 并把 `ZIG_EXECUTABLE` 缓存变量 FORCE 设置——**解决 Ghostty CMake 包装缓存 Zig 路径、flake 换 Zig 版本后构建树残留旧路径的问题** |
| 17–31 | Raylib 5.5：系统包优先，缺失则 `FetchContent` 拉 GitHub tag |
| 33–57 | `FetchContent` 拉 ghostty 仓库，`zig build lib-vt` 产出 `libghostty-vt` 共享库；`bin2header.cmake` 把 `fonts/JetBrainsMono-Regular.ttf` 转成 `font_jetbrains_mono.h` |
| 61–75 | 目标 `ghostling`：C11、链 raylib + ghostty-vt；macOS 链 IOKit/Cocoa/OpenGL，Linux 链 `util`（forkpty 需要） |

#### 其余文件
- `bin2header.cmake`：二进制 → C 头文件数组转换脚本（字体嵌入用）
- `AGENTS.md`：构建命令、代码约定（C 非 C++、单文件、**`assert()` 内不得放副作用调用**——release 构建会移除）、libghostty 升级流程（先改 CMakeLists 版本→清构建目录→重编译测 API 变化）
- `flake.nix` / `flake.lock`：devShell 提供 zig 0.15 + 构建依赖
- `.github/workflows/test.yml`：CI 构建矩阵
- `fonts/`：JetBrainsMono-Regular.ttf + OFL 许可
- `README.md`：libghostty 能力清单（reflow、24-bit 色、Unicode 字素、kitty keyboard/graphics、鼠标上报 X10/SGR/URxvt/UTF8、滚动回退、焦点上报）；**明确的边界声明**：libghostty 不提供 tabs/多窗口/split/会话管理/搜索 UI——消费者自建

### 1.3 功能对比 torvox

| 功能 | torvox 有？ | 对比 |
|------|-----------|------|
| PTY spawn + shell 环境 | ✅ 有（Rust `nix`，比 C forkpty 更安全） | ghostling 的 `$SHELL`→passwd→`/bin/sh` 选择链，torvox 的 shell 启动路径同样适用；`TERM=xterm-256color` 一致 |
| EIO 当 EOF 处理 | ✅ 有（Linux pty 必备） | ghostling main.c:148–151 的注释可作为 torvox `terminal/` 读取循环的对照验证 |
| 焦点上报按 mode 门控 | ✅ 有（MouseModeTracker/FocusMode 概念） | ghostling 提供**纯 C 参考实现**：查 `GHOSTTY_MODE_FOCUS_EVENT` 再编码 CSI I/O（main.c:1469–1486） |
| 鼠标上报 mode 门控 | ✅ 有 | 同上，参考 main.c:323–448 |
| Kitty keyboard 协议 | ✅ 有（libghostty-vt 编码器） | ghostling 验证了"key + mods + **unshifted codepoint**"三元组是 kitty 协议正确性的前提（main.c:229–255）；Raylib 输入不完整导致协议有缺陷的教训（README 已知问题）说明**输入事件质量决定协议质量**——torvox 的 Android 键事件需保证物理键码 |
| Kitty graphics 渲染 | ✅ 有（libghostty-vt `png` feature） | ghostling 给出完整 C 调用链：placement iterator → viewport pos / grid size / source rect / x/y offset → 图片数据。torvox 的 Rust 绑定若未实现图片渲染，此为官方参考 |
| PNG 解码器注册 | ✅ 有（libghostty-vt 需要宿主提供 decoder） | ghostling main.c:1054–1090 是标准实现样板（解码→RGBA→`ghostty_alloc` 拷贝） |
| effects（size/DA/xtversion/title） | ✅ 有（Rust 绑定） | **唯一有增量价值的对照清单**：`effect_write_pty`（main.c:1104）是"vim/tmux 不挂起"的底线；DA2 报 VT220 + 固件版本（main.c:1129）可对照 torvox 的实现 |
| 滚动条拖拽 | ✅ 有（torvox 有 scrollback + 滚动 UI） | ghostling 的"滚动条在鼠标转发前消费"顺序（main.c:1514–1518）值得 torvox 的触摸/鼠标事件分发参考 |
| 伪粗体/斜体模拟 | ✅ 有（cosmic-text + swash 真实字形样式） | ghostling 是 2D 兜底方案；torvox 的 GPU 方案更优，**不需要** |
| 渲染脏区域 | ✅ 有（libghostty render state 脏跟踪） | ghostling 展示了每行/全局 dirty 清除的两级用法（main.c:936–939、1016–1017） |
| 多会话/Tabs/搜索 UI | ✅ torvox 有会话抽屉/搜索 | ghostling 明确不做（README 边界声明），torvox 已超越 |

**结论**：功能层面 torvox 全量覆盖且更先进；ghostling 的价值在**API 使用细节与坑位**。

### 1.4 依赖分析

- Raylib 5.5、CMake FetchContent、Zig 0.15：**不适用于 torvox**（torvox 渲染用 wgpu/Vulkan，字体用 cosmic-text/swash，无窗口库）。
- libghostty-vt：**torvox 已在用**（vendored Zig 动态库，Rust 绑定），属同源依赖，非新增。
- 先进性：作为官方演示，紧随 libghostty API 演进（README 明确"更新 libghostty 后立即清理构建目录"），是**跟踪上游 API 变化的活样本**，不激进。

### 1.5 可吸收到 torvox 的具体内容

1. **effects 完整性对照检查**（直接行动）：对照 ghostling main.c:1104–1189 的 6 个 effect，逐一核对 torvox 的 Rust 绑定是否实现 `write_pty`/`size`/`device_attributes`/`xtversion`/`title_changed`/`color_scheme`；重点验证 `write_pty`（缺失会导致交互程序探测挂起）。
2. **EIO→EOF 处理**（代码注释建议）：torvox `terminal/` 的 pty 读取循环内已有类似处理时，可补充注释：
   ```rust
   // Linux 上 slave 端关闭常返回 EIO 而非干净 EOF（read 返回 0），
   // 与 ghostling main.c:148-151 的观察一致，必须同等对待。
   ```
3. **Kitty graphics 渲染管线**（若 torvox 尚未实现图片显示）：以 main.c:662–777 为蓝本实现 Rust 版 placement iterator → 源矩形/子格偏移 → 纹理绘制；PNG decoder 注册参考 main.c:1054–1090。
4. **输入编码三元组**：kitty keyboard 协议依赖"物理键 + mods + unshifted codepoint"；torvox 的 Android 键码→Ghostty key 映射层可对照 main.c:164–208 的连续区间映射思路。
5. **构建系统防缓存坑**：CMakeLists.txt:13–14 的 `ZIG_EXECUTABLE` FORCE 缓存覆盖技巧，可移植到 torvox 的 nix/构建脚本（升级 zig 版本后强制刷新路径，避免旧 store 路径残留）。

### 1.6 项目文档吸收价值

- README 的"libghostty 能力边界"清单（什么由 lib 提供、什么由宿主实现）可直接引用进 torvox 的 `docs/architecture.md` 或 SRS，作为"宿主责任"章节的权威依据。
- AGENTS.md 的"升级 libghostty 三步法"（改版本→清构建→重编译）可并入 torvox 的 vendor 升级流程文档。
- README 已知问题（Raylib 输入系统无法完整支持 kitty keyboard）是"输入事件保真度"的实证案例，可写入 torvox 的输入子系统设计文档。

---

## 2. ply（shafinthedev/Ply）— 低价值

### 2.1 项目定位与完整架构

Rust 编写的 Android "终端" 原型（v0.1.0）：**并非真实终端**——没有 PTY、没有 VT 解析、没有渲染引擎。它是一个 **REPL 演示**：Rust 二进制打印 ANSI 着色的横幅与提示符，`stdin` 读一行，`Command::new("/system/bin/sh").arg("-c")` 同步执行并把 stdout/stderr 原样转发。Android 侧 `MainActivity` 用 TextView + EditText + ScrollView 模拟终端外观，`ProcessBuilder` 起 `/system/bin/sh` 后台线程逐行读输出追加到 TextView。

代码库极小（Rust 162 行 + Kotlin 93 行 + proot.rs 41 行），另有 `src/proot.rs` 演示用 proot 启动 Linux rootfs（**未被 main.rs 调用**，属未接线原型）。

```
Android: MainActivity(TextView 终端) ──ProcessBuilder──> /system/bin/sh（Android 原生 shell）
Rust 二进制（可选替代）：main.rs REPL ──Command::new──> /system/bin/sh -c <cmd>
```

### 2.2 文件功能说明

#### src/main.rs（162 行）
| 行号 | 符号 | 功能 |
|------|------|------|
| 8–33 | `main()` | 打印 ANSI 青色横幅；循环：绿色提示符 `ply@android:~$` → 读一行 → `exit/help/clear` 内建 → `execute_command` |
| 35–47 | `setup_environment()` | `set_var`：`TERM=xterm-256color`、`SHELL=/system/bin/sh`、`HOME=/data/data/com.ply/ply-home`；创建 home/bin 目录 |
| 49–68 | `execute_command(cmd)` | `sh -c cmd` 同步 `.output()`；stdout/stderr 原样写回；失败回退 `handle_builtin` |
| 70–93 | `handle_builtin(cmd)` | 解析 `ply-pkg install/list/update` 子命令 |
| 95–118 | `pkg_install(pkg)` | `curl -s https://raw.githubusercontent.com/ply-packages/{name}/main/install.sh | sh` —— **从网络直接管道执行脚本，无校验** |
| 120–133 | `pkg_list()` / `pkg_update()` | 打印假列表 / 占位 |
| 145–162 | `show_help()` | 全角边框帮助文本 |

#### src/proot.rs（41 行）
| 行号 | 符号 | 功能 |
|------|------|------|
| 4–13 | `setup_proot()` | rootfs 不存在则下载，然后 `spawn_proot_shell` |
| 15–26 | `download_rootfs(dest)` | `curl -L` termux/proot-distro v4.6.0 rootfs.tar.xz → `tar -xf`（**未解压到 dest，实际解到 CWD——bug**） |
| 28–41 | `spawn_proot_shell(rootfs)` | `proot --rootfs ... --link2symlink -b /dev -b /proc -b /sys -w /home/ply /bin/bash` |

#### android/MainActivity.kt（93 行）
| 行号 | 符号 | 功能 |
|------|------|------|
| 22–43 | `onCreate` | `ProcessBuilder("/system/bin/sh")` 起 shell；`IME_ACTION_SEND` 触发 `sendCommand`；`setOnApplyWindowInsetsListener` 处理系统栏 padding |
| 45–69 | `startShell()` | 后台线程 `readLine` 循环 → `runOnUiThread` append 到 TextView + `scrollToBottom` |
| 71–80 | `sendCommand()` | 回显 `ply@android:~$ <cmd>` 到 TextView，写 `"$cmd\n"` 到 stdin |
| 82–85 | `scrollToBottom()` | ScrollView `fullScroll(FOCUS_DOWN)` |
| 87–92 | `onDestroy()` | 销毁进程、关闭流 |

#### 其余文件
- `Cargo.toml`：零依赖（纯 std），`[[bin]] name="ply"`
- `android/app/build.gradle`：Kotlin 1.9、minSdk 24/target 34、无 JNI 配置（**Rust 二进制与 APK 未集成**，仅独立构建）
- `android/AndroidManifest.xml`：声明了不存在的 `TerminalService`（占位）
- `tests/integration_test.rs`：两个伪测试（`test_help_command` 跑 cargo run、`test_echo_command` 跑 sh）
- `.github/workflows/build.yml`：Rust 交叉编译 x86_64/aarch64-linux-gnu（非 Android target）；`build-apk.yml`：标准 Gradle 构建 + 签名 + 上传 APK
- `scripts/setup_termux.sh` / `install-package.sh`：Termux 环境准备脚本
- `CONTRIBUTING.md` / `SECURITY.md` / `NOTICE`：模板文档

### 2.3 功能对比 torvox

| 功能 | torvox 有？ | 对比 |
|------|-----------|------|
| 真实 PTY + VT 解析 | ✅ 有 | ply **没有**（`sh -c` 同步执行、无 pty、无转义处理）——torvox 全面超越 |
| 环境变量设置（TERM/SHELL/HOME） | ✅ 有（`prefixEnvironment()`） | ply main.rs:35–47 的写法与 torvox 一致，无新意 |
| proot Linux 环境 | ❌ torvox 用 Termux 风格 native ELF + linker bootstrap | ply 的方案是 proot 用户态（**慢**，且 rootfs 依赖外网下载）；torvox 的 native 方案性能更优，**不切换** |
| 包管理器（ply-pkg） | ❌ torvox 有 apt（bootstrap 内） | ply 的 `curl | sh` 无签名校验，是**反面教材**，禁止模仿 |
| 终端 UI（TextView 模拟） | ❌ torvox 用 wgpu | ply 的 TextView 方案无光标/选择/滚动性能，无参考价值 |

### 2.4 依赖分析

- 零第三方依赖；proot 依赖 Termux 分发。对 torvox：**不适用、不先进**（proot 方案已被 torvox 架构决策排除）。
- 唯一警示：`pkg_install` 的远程脚本管道执行（main.rs:95–118）是供应链风险示范，torvox 的 bootstrap 下载校验策略可引用此反例。

### 2.5 可吸收到 torvox 的具体内容

1. **无**（功能性）。
2. 可作为"环境变量初始化顺序"的简单示例对照（ply main.rs:35–47 vs torvox `prefixEnvironment()`），仅用于文档中的教学对比。
3. 反模式记录：`curl | sh` 无校验安装（ply main.rs:99–103）、rootfs 解压目标错误（proot.rs:22–25）、manifest 声明不存在的 Service（AndroidManifest.xml:26）——写入 torvox 代码评审 checklist 的"不要这样做"清单。

### 2.6 项目文档吸收价值

低。README 仅一句话（"Native terminal emulator for Android - fast, modern, open"）。CI 矩阵（Rust 多 target + APK 双流水线）的组织方式可作参考，但 torvox 已有更完善的 CI。

---

## 3. onecode（hishow1996/OnecodeTerminal）— 中高价值

### 3.1 项目定位与完整架构

Android 上的 **Ubuntu 24（proot）终端**，被集成进"Onecode"应用。模块化设计：`app`（Compose UI）+ `terminal-core`（Git 子模块，独立仓库 `OnecodeTerminalCore`，核心逻辑）。技术栈：Jetpack Compose + Kotlin Flows + Coroutines + **AIDL 服务化** + **自研 ANSI 解析器**（Kotlin）+ **Canvas/SurfaceView 渲染** + **proot Ubuntu rootfs**（assets 内置 64MB rootfs 压缩包）。

核心架构（以 `TerminalManager` 单例为中心）：

```
app 模块（UI）
  MainActivity（沉浸式全屏 + 手势导航栏）
  TerminalScreen（NavHost：loading/setup/home/settings）
    TerminalHome（会话列表 + CanvasTerminalScreen + 修饰键键盘行 + 命令输入）
terminal-core 模块（逻辑，同进程直连；另备 AIDL 服务）
  TerminalManager（单例：环境初始化、会话生命周期、命令队列、事件流）
    ├─ SessionManager（会话状态 StateFlow）
    ├─ OutputProcessor（行分割、进度行/提示符检测、命令完成判定）
    ├─ TerminalProvider（抽象：LocalTerminalProvider=proot Ubuntu / SSHTerminalProvider）
    ├─ Pty.kt + pty.c（JNI：forkpty + winsize）
    ├─ AnsiTerminalEmulator（自研 VT 解析：screen/alt screen/history/模式）
    └─ CanvasTerminalView（SurfaceView + 渲染线程 + 手势/选择/IME）
  TerminalService + AIDL（跨进程 IPC，备用）
```

数据流：`Pty` master fd → 读线程（coroutine）→ `OutputProcessor.processOutput` → `AnsiTerminalEmulator.parse`（渲染数据源）→ `CanvasTerminalView` 渲染线程按脏标记重绘；输入走 `TerminalManager.sendInput/sendCommand` → Pty stdin。

**关键架构特征**：终端会话状态（含 ANSI 解析器实例、rawBuffer、命令队列、滚动位置）全部挂在 `TerminalSessionData`（data 类）上，由 `SessionManager` 的单个 `MutableStateFlow<TerminalState>` 驱动——Compose 通过 `collectAsState` 响应式订阅；`TerminalManager` 用 `MutableSharedFlow` 广播命令执行/目录变化事件。

### 3.2 文件功能说明

#### app 模块

**app/src/main/java/com/ai/assistance/onecode/terminal/MainActivity.kt（237 行）**
| 行号 | 符号 | 功能 |
|------|------|------|
| 36–62 | `MainActivity` 状态字段 | 沉浸式全屏：`statusBarShown`、`hideStatusRunnable`（3 秒后自动隐藏导航栏）、`showStatusRunnable`（长按屏幕边缘唤出导航栏）、`edgeWidth=32dp`/`exclusionHeight=200dp` 手势排除区、`touchSlop`、`longPressTimeout=500ms` |
| 65–128 | `onCreate` | `setDecorFitsSystemWindows(false)`；黑底；隐藏导航栏；API 29+ 用 `setSystemGestureExclusionRects` 把屏幕两侧顶部区域排除出系统手势（防误触发返回手势）；创建初始会话（"Ubuntu 1"）；`setContent { TerminalScreen(env) }`；**双击返回键 2 秒内退出** |
| 130–176 | `dispatchTouchEvent` | 边缘长按（`edgeWidth` 内 + `exclusionHeight` 内）→ 显示导航栏 3 秒；点击导航栏已显示区域则立即隐藏 |
| 177–180 | `getStatusBarHeightFallback()` | 兜底读取状态栏高度 |
| 190–213 | `showBlackOverlay()` / `hideBlackOverlay()` | **切后台隐私保护**：`onUserLeaveHint`/`onPause` 时全屏盖黑层（防截屏泄露终端内容），`onResume` 移除 |
| 215–220 | `dismissKeyboard()` | 隐藏 IME + requestLayout |
| 222–237 | `onUserLeaveHint`/`onPause`/`onResume` | 生命周期钩子组合 |

**app/build.gradle.kts（~116 行）**：AGP 8.6、Kotlin 1.9.22、compileSdk 34/minSdk 26、`abiFilters arm64-v8a`、keystore.properties 或环境变量签名、`implementation(project(":terminal-core"))`。
**gradle/libs.versions.toml**：Compose BOM 2024.04.01、coroutines 1.7.3、navigation 2.7.7、kotlinx-serialization 1.6.2（版本偏旧，2024 年初水平）。
其余：`ExampleInstrumentedTest.kt`/`ExampleUnitTest.kt`（模板）、`AndroidManifest.xml`（main 包名 `com.ai.assistance.onecode.terminal`）、`ic_launcher.xml`。

#### terminal-core 模块（OnecodeTerminalCore，独立仓库，以下为 main 分支）

**Pty.kt（~180 行）**
| 行号 | 符号 | 功能 |
|------|------|------|
| 8–17 | `class Pty` | 封装 `Process`（dummy）+ master `FileDescriptor` + stdout/stdin 流；便利构造把 master fd 包成 `FileInputStream/FileOutputStream` |
| 19–25 | `waitFor()`/`destroy()` | destroy 关闭 master 流（**注释强调：关 master fd 才能向子进程发 EOF**） |
| 30–45 | `companion init` + `start(...)` | `System.loadLibrary("pty")`；`createSubprocess(cmd, env, cwd)` 返回 `{pid, masterFd}` |
| 46–112 | 匿名 `dummyProcess` | 手工 `Process` 实现：`destroy()` 先 `sendSignal(pid, SIGHUP)` 再 `SIGKILL`（**杀整个进程组语义**）；`exitValue()` 用 `sendSignal(pid,0)` 探测存活；`waitFor()` 委托 native |
| 114–130 | `createSubprocess` external + `Reflect.getFileDescriptor(fd)` | **反射写 `FileDescriptor.descriptor` 私有字段**把 int fd 转成 FileDescriptor（免 JNI 返回 jobject） |

**TerminalSession.kt（13 行）**：`data class TerminalSession(process, stdout, stdin)` —— 最小会话载体。

**TerminalEnv.kt（~110 行）**
| 行号 | 符号 | 功能 |
|------|------|------|
| 23–46 | `class TerminalEnv` | `@Stable`，聚合 5 个 `State`（sessions/currentSessionId/currentDirectory/isFullscreen/terminalEmulator）+ `command` 可变状态 |
| 47–64 | `onSendInput(text, isCommand)` | 命令模式走 `sendCommand`（允许空命令——SSH 交互），输入模式走 `sendInput`（允许空输入——ssh-keygen 回车） |
| 66–77 | `onSetup(commands)` | 把初始化命令 `joinToString(" && ")` 交给 manager |
| 79–110 | `rememberTerminalEnv(manager)` | 收集 5 个流（含 **placeholder `AnsiTerminalEmulator(1,1,0)` 兜底**，避免空状态崩溃） |

**SessionManager.kt（~190 行）**
| 行号 | 符号 | 功能 |
|------|------|------|
| 15–20 | `class SessionManager` | 持有 `MutableStateFlow<TerminalState>` |
| 22–47 | `createNewSession(title, terminalType)` | 会话计数命名（Ubuntu/SSH/Terminal N），返回 `TerminalSessionData` |
| 49–65 | `switchToSession(sessionId)` | 校验存在性后切 `currentSessionId` |
| 67–105 | `closeSession(sessionId)` | **清理顺序**：cancel `readJob` → close `sessionWriter` → `terminalManager.closeTerminalSession` → 从列表移除并重选当前会话 |
| 107–130 | `updateSession` / `getSession` / `clearAllSessions` | 更新器/查询/全清 |

**TerminalManager.kt（44KB，远程读取；前 120 行与尾段已确认，中段按上下文推断）**
| 位置 | 内容 |
|------|------|
| 前段（1–~120） | imports；`private constructor(context)`；`coroutineScope = CoroutineScope(Dispatchers.IO + SupervisorJob())`；`envInitMutex`；目录布局 `filesDir/usr/bin`、`nativeLibraryDir`；`activeSessions: ConcurrentHashMap<String, TerminalSession>`；SharedPreferences("terminal_settings")；组件装配：`sessionManager`、`outputProcessor`（三个回调：命令事件/目录事件/命令完成→`processNextQueuedCommand`）、`sourceManager`、`sshConfigManager`、`sshdServerManager`；事件流 `_commandExecutionEvents/_directoryChangeEvents`（MutableSharedFlow）；派生流 `sessions/currentSessionId/currentDirectory/isInteractiveMode/interactivePrompt/isFullscreen/terminalEmulator`（StateFlow.map） |
| companion | `@Volatile INSTANCE` + 双重检查单例；`UBUNTU_FILENAME="ubuntu-noble-aarch64-pd-v4.18.0.tar.xz"`；`MAX_HISTORY_ITEMS=500` |
| 中段（推断，未逐行确认） | `initializeSession`（解压 assets rootfs、`createBusyboxSymlinks` 建 busybox 符号链接、`generateStartScript` 生成 `common.sh` 含 install_ubuntu/login_ubuntu/proot 启动命令、`SourceManager` 配置镜像源、以 `bin/bash -c "source $HOME/common.sh && start_shell"` 起 Pty）；`sendCommand/sendInput/sendInterruptSignal`（写 pty、队列化 `QueuedCommand`、`Mutex` 串行化）；`closeTerminalSession`；`handleRegularCommand/handleInteractiveInput`（输出分页到 `CommandHistoryItem.outputPages`）；`getFileSystemProvider` 等 |
| 尾段（已确认） | `getFileSystemProvider()`（runBlocking 取 provider）与 `getSSHDServerManager()`（反向 SSH 隧道用） |

**TerminalModels.kt（~110 行）**
| 行号 | 符号 | 功能 |
|------|------|------|
| 12–16 | `QueuedCommand(id, command)` | 命令队列项 |
| 19–55 | `CommandHistoryItem` | Compose 可变状态（prompt/command/output/isExecuting）+ `outputPages`（分页输出）+ 稳定 getter/setter（AIDL 序列化友好）；`equals` 按 id |
| 58–61 | `enum SessionInitState` | `INITIALIZING/LOGGED_IN/AWAITING_FIRST_PROMPT/READY` —— **会话初始化状态机** |
| 63–105 | `TerminalSessionData` | 巨型 data 类：id/title/terminalType/terminalSession/pty/sessionWriter/currentDirectory/currentCommandOutput/rawBuffer/交互标志/`initState`/`readJob`/`isFullscreen`/`ansiParser`/`currentExecutingCommand`/`commandQueue`/`commandMutex`/`scrollOffsetY`；`@Transient` 标记非持久字段 |
| 108–115 | `TerminalState(sessions, currentSessionId, isLoading, error)` | 顶层状态 |
| 其余 | `SSHConfig`、`MirrorSource`、`SourceConfig`（镜像源配置，`PackageManagerType`） | |

**OutputProcessor.kt（28KB，~700 行）**
| 行号 | 符号 | 功能 |
|------|------|------|
| 16–21 | `SessionProcessingState` | 每会话 CR 处理状态 |
| 24–42 | `class OutputProcessor` | 三个构造回调（命令事件/目录事件/命令完成） |
| 47–~200 | `processOutput(sessionId, chunk, sessionManager)` | 追加 rawBuffer；**先 `detectFullscreenMode`（全屏切换则提前返回）**；总是 `ansiParser.parse(chunk)`（保证 Canvas 渲染完整，含初始化输出）；行分割循环：CRLF 完整行 / 单独 CR 进度更新 / LF / 无终止符时**先判 `AnsiUtils.isProgressLine`（进度行优先）再判 `isPrompt` 提示符、`isInteractivePrompt` 交互输入** |
| ~200–~450 | `handleCarriageReturn`/`processLine`/`isPrompt`/`isInteractivePrompt` | 进度行只更新当前输出；提示符检测（`$ `、`# ` 等）触发命令完成 → `onCommandCompleted`；交互检测（sudo 密码等）设置 `isInteractiveMode` 并记录 `interactivePrompt` |
| 尾段 | `sendWelcomeMessage`（全宽边框 ASCII 艺术横幅 + 版本信息，按 `screenWidth` 自适应；先 `CSI 2J` 清屏） | |

**provider/type/TerminalProvider.kt（~50 行）**：`interface TerminalProvider { isConnected/connect/disconnect/startSession/closeSession/getFileSystemProvider/getWorkingDirectory/getEnvironment }`；`enum TerminalType { LOCAL, SSH, ADB }`。

**provider/type/LocalTerminalProvider.kt（~110 行）**：proot 本地终端；`buildEnvironment()`（**关键环境变量清单**）：`PATH=${binDir}:${PATH}`、`HOME`、`PREFIX`、`TERMUX_PREFIX`、`LD_LIBRARY_PATH=${nativeLibDir}:${binDir}`、`PROOT_LOADER=bin/loader`、`TMPDIR`、`PROOT_TMP_DIR`、`TERM=xterm-256color`、`LANG=en_US.UTF-8`；启动命令 `bash -c "source $HOME/common.sh && start_shell"`。

**provider/type/SSHTerminalProvider.kt（~260 行）**：复用本地 bash，`startSession` 走 `ssh_shell` 包装，`buildSshCommand()` 组装 ssh 参数；经 `SSHFileConnectionManager` 建连接（密码/私钥/keepalive/端口转发/反向隧道参数全量透传）。

**service/TerminalService.kt（~150 行）**：`Service` + `ITerminalService.Stub`：`createSession`（runBlocking）/`switchToSession`/`closeSession`/`sendCommand`/`sendInterruptSignal`/`registerCallback`/`unregisterCallback`/`requestStateUpdate`；`RemoteCallbackList<ITerminalCallback>` 广播 `commandExecutionEvents`/`directoryChangeEvents`；`SupervisorJob` + `Dispatchers.Main` scope。**注意**：AIDL 方法用 `runBlocking` 在 Binder 线程跑 suspend——IPC 直通单例，UI 与核心同进程时实际未启用。

**aidl/ 下 4 个文件**：`ITerminalService.aidl`（8 方法）、`ITerminalCallback.aidl`（`oneway` 两方法）、`CommandExecutionEvent.aidl`（commandId/sessionId/outputChunk/isCompleted）、`SessionDirectoryEvent.aidl`（sessionId/currentDirectory）。

**jni/pty.c（~200 行）**：`Java_..._Pty_00024Companion_createSubprocess`：termios 初始化（ICRNL/IXON/IXANY、OPOST/ONLCR、ISIG/ICANON/ECHO 族、`VINTR='C'-'@'` 等控制字符表、VMIN=1/VTIME=0）、`forkpty` → 子进程 `chdir(cwd)` + `execve` → 父进程返回 `{pid, master_fd}`；`setWindowSize`：`ioctl(TIOCSWINSZ)`。

**view/domain/ansi/AnsiTerminalEmulator.kt（38KB，~900 行）** —— 自研 VT 引擎
| 行号 | 符号 | 功能 |
|------|------|------|
| 9–38 | `data class TerminalChar(char, TextAttributes)` | 单元格：字符 + 10 种属性（bold/dim/italic/underline/blink/inverse/hidden/strikethrough） |
| 40–119 | `class AnsiTerminalEmulator` | 字段：`screenBuffer`（Array²）、`lineWrapped`、`historyBuffer`（滚动回退，默认 200 行）、**`bufferLock`（渲染线程读/IO 线程 parse 写/主线程 resize 三方同步，注释详述竞争场景）**、`altScreenBuffer`（备用屏）、光标、`savedCursor`（DECSC/DECRC）、`terminalModes` map、滚动区域、`changeListeners`/`newOutputListeners`（CopyOnWriteArrayList）、`fullContentView`、**`pendingSequence`（未终结转义序列跨块保留——修复 ANSI 序列被 PTY 块边界切断的问题）** |
| 中段（推断） | `parse(chunk)` 主解析（scanNext 循环分派）、SGR 颜色处理（256 色/24-bit）、`setMode`（DECSET/DECRST）、滚动、resize（保留内容）、`takeSnapshot/fullContent`（渲染快照） |
| 尾段 | `notifyChange`/`notifyNewOutput` | 观察者通知 |

**view/domain/ansi/AnsiScanner.kt（~380 行）**：流式扫描器：`scanNext()` 分派 `scanEscapeSequence`（CSI/OSC/DCS/DEC 单字符 `7 8 c D E H M Z`/Unknown）/`scanControlCharacter`/Text；**CSI 解析**：`?` 私有模式、参数、intermediate、final 字节；**不完整序列返回 `AnsiSequence.Incomplete`**（配合 `pendingSequence`）；尾段含 `FullscreenMode {ENTER, EXIT}` 检测辅助。

**view/domain/OutputProcessor.kt**：见上。

**view/canvas/CanvasTerminalView.kt（80KB，~1900 行；首尾已确认，中段推断）**
| 位置 | 内容 |
|------|------|
| 前段（1–~120） | `class CanvasTerminalView : SurfaceView, SurfaceHolder.Callback`；Paint 复用组（text/bg/cursor/selection/handle×2）；`textMetrics: TextMetrics`；`emulator` 引用 + change/newOutput 监听器；`autoScrollToBottom`/`isUserScrolling`/`needScrollToBottom`；`renderThread` + `ReentrantLock` + `Condition`（`isDirty` 门控）；光标闪烁（500ms）；`gestureHandler` + `selectionManager` |
| 中段（推断） | `RenderThread`（`surfaceCreated` 启动，等锁/等脏 → `drawTerminal`）；`drawTerminal`（快照 `fullContent` → 按 `PerformanceOptimizer.DirtyRegionTracker` 增量或全量绘制 → 光标/选择框/滚动条）；`TextSelectionManager` 集成（ACTION_MODE 复制菜单）；`onTouchEvent` 转 `GestureHandler`；`updatePtySize`（字号/行列变化 → pty `setWindowSize` 线程化调用）；`InputConnection`（`BaseInputConnection` 实现 commitText/deleteSurrounding/sendKeyEvent → 转义序列或原文字符） |
| 尾段 | `setWindowSize(rows, cols)` 线程包装 + `requestRender()` |

**view/canvas/GestureHandler.kt（~170 行）**
| 行号 | 符号 | 功能 |
|------|------|------|
| 11–62 | `class GestureHandler` | `ScaleGestureDetector`（onScale/onScaleBegin/onScaleEnd）+ `GestureDetector`（scroll/doubleTap/longPress/fling）；**`onScaleEnd` 一次性回调：把积累的字号变化在缩放手势结束时一次性同步终端（resize+SIGWINCH），避免手势中每帧 resize 与 TUI 竞争错位**（注释明示） |
| 65–170 | `class TextSelectionManager` | `Selection(startRow/startCol/endRow/endCol)` + `normalize()` + `contains`；`DragHandle {NONE, START, END}` 拖动端点；`setSelection/clearSelection/hasSelection` |

**view/canvas/PerformanceOptimizer.kt（~150 行）**
| 行号 | 符号 | 功能 |
|------|------|------|
| 9–60 | `DirtyRegionTracker` | 脏矩形列表 + `markDirtyCell(row,col,w,h)` + `optimizeDirtyRegions()`（按 top,left 排序，相交或 50px 内合并） |
| 62–90 | `AdaptiveFrameRateController` | 60 帧滑动窗口；**空闲自适应帧率**：>5s 无活动 10fps、>1s 30fps、活动 60fps（`getAdaptiveSleepTime`） |
| 92–150 | `PerformanceStats`/`FrameRateMonitor` | FPS 统计（`getCurrentStats` 返回 fps/avgFrameTimeMs） |

**view/canvas/TextMetrics.kt（~170 行）**
| 行号 | 符号 | 功能 |
|------|------|------|
| 10–28 | `class TextMetrics` | `LruCache<Char,Float>` 宽缓存 + `LruCache<Char,Int>` 字形归属缓存；`charWidth/charHeight/charBaseline` |
| 30–48 | `updateFromRenderConfig` | 以 'M' 测量单元宽；`descent-ascent` 行高；清缓存 |
| 51–97 | `setNerdTypeface`/`resolveFontType`/`setFont`/`selectTypefaceForChar` | **Nerd Font 回退链**：默认字体 `hasGlyph` 检查 → Nerd 字体 → 放弃（0/1/2 三态缓存） |
| 99–170 | `getCharWidth`/`applyStyle`/`resetStyle` | 带缓存宽度、粗斜体 Typeface 合成、样式复位 |

**view/canvas/CanvasTerminalCompose.kt（~210 行）**
| 行号 | 符号 | 功能 |
|------|------|------|
| 10–67 | `CanvasTerminalScreen(...)` | `AndroidView` 包装：factory 配置（emulator/pty/input/scale/sessionScroll 回调 + `requestFocus` + `onViewReady`）；**`requestDisallowInterceptTouchEvent` 防父容器抢触摸**；update 同步；**`onRelease` 中 `stopRenderThread()`（防渲染线程竞争已销毁 Surface 导致 ANR——注释明示）** |
| 69–104 | `ConfigurableCanvasTerminal` | 简化配置版（fontSize/bg/fg/cursor 色） |
| 106–210 | `PerformanceMonitoredTerminal`/`CanvasTerminalOutput` 等 | 带 FPS 回调/纯输出版 |

**main/TerminalScreen.kt（~200 行）**：`NavHost`：`loading`（LoadingScreen：CircularProgressIndicator + 设置入口）→ 首启 `SETUP_ROUTE` 或 `TERMINAL_HOME_ROUTE`；`LaunchedEffect(startDestination, isTerminalReady)` 等终端 READY 后才跳转；`UpdateChecker` 后台静默更新检查。

**main/TerminalRoutes.kt**：路由常量。

**ui/TerminalHome.kt（46KB，~1100 行；首部已确认）**：`TerminalHome(env, onNavigateToSetup, onNavigateToSettings)`：字体配置（`TerminalFontConfigManager` 持久化 `RenderConfig`，`LaunchedEffect/DisposableEffect` 双保险刷新）；会话列表 LazyRow + `CanvasTerminalScreen` 主体；命令输入 `BasicTextField` + **语法高亮**（`SyntaxHighlightingVisualTransformation`）；底部修饰键键盘行（`ModifierKeyButton("CTRL"/"ALT")` 切换 + `KeyButton` 方向键/PGDN + `OrangeKeyButton(⏎)`，发送 `\u001b[D` 等转义）；`Ctrl`/`Alt` 状态经 `onModifierChanged` 同步到输入层。

**ui/SettingsScreen.kt（55KB）/SetupScreen.kt（32KB）/SSHConfigScreen.kt（30KB）**：设置页（字体、主题、镜像源、SSH 配置）；SetupScreen 首启配置引导（rootfs 安装命令勾选）；SSHConfigScreen 连接表单（复用 SSHConfig 数据类）。远程读取仅列名。

**provider/filesystem/**：`FileSystemProvider`（抽象）+ `LocalFileSystemProvider`（26KB，SAF 文档树桥接本地文件）+ `SSHFileSystemProvider`（28KB，SFTP 文件浏览）；`provider/UbuntuDocumentsProvider.kt`（10KB，SAF ContentProvider 暴露 Ubuntu 文件系统，Uri 映射/查询/打开）。

**utils/**（按名列出，未逐行读）：`SourceManager`（apt 镜像源管理）、`SSHConfigManager`、`SSHDServerManager`（本地 sshd 反向隧道）、`SSHFileConnectionManager`、`TerminalFontConfigManager`、`UpdateChecker`、`AnsiUtils`（stripAnsi/isProgressLine）。

**assets/**：`ubuntu-noble-aarch64-pd-v4.18.0.tar.xz`（64MB proot rootfs，**直接打进 APK**）；`setup_fake_sysdata.sh`（7.3KB，模拟 sysdata 的初始化脚本）。

**res/**：`jetbrains_mono_nerd_font_regular.ttf`（2.2MB Nerd Font 资源）；`values/values-en/strings.xml` 中英双语。

### 3.3 功能对比 torvox

| 功能 | torvox 有？ | 对比 |
|------|-----------|------|
| 真实 PTY（forkpty JNI） | ✅ 有（Rust nix） | onecode 用 C JNI + 反射 fd；torvox 的 Rust 方案更安全。其"关 master fd 发 EOF"（Pty.kt:27–28）与"destroy 先 SIGHUP 再 SIGKILL"（Pty.kt:52–64）语义可对照 torvox 会话关闭路径 |
| VT 解析 | ✅ libghostty-vt | onecode 自研 Kotlin 解析器（~1300 行）覆盖 VT100/xterm 子集；**完整性远不及 libghostty-vt**。其 `pendingSequence` 跨块保留、`bufferLock` 三方同步、alt screen 等设计点已在 libghostty 内建，**不吸收代码，吸收问题意识** |
| GPU 渲染 | ✅ wgpu | onecode Canvas/SurfaceView 逐字绘制 + 脏矩形 + 自适应帧率；torvox 的 wgpu 路线性能上限更高。**脏区域合并（50px 阈值）与空闲降帧（10/30/60fps）思路**可移植到 torvox 的"是否全量重建 instance buffer"决策 |
| 会话管理 | ✅ 有（SessionDrawer） | onecode 的 `TerminalSessionData` 巨型状态对象 + `SessionManager` 单 StateFlow 模式值得对照：torvox 若按会话隔离 scrollback/emoji 字体等状态可参考其 `@Transient` 标注习惯 |
| 会话初始化状态机 | ✅ 有（bootstrap/installer 流程） | onecode `SessionInitState{INITIALIZING,LOGGED_IN,AWAITING_FIRST_PROMPT,READY}`（TerminalModels.kt:58–61）是清晰的四态机：**AWAITING_FIRST_PROMPT 等待首个提示符**——torvox 的安装器/就绪判定可借鉴"以首个 shell 提示符为就绪信号" |
| proot Ubuntu 环境 | ❌ torvox 用 Termux bootstrap | onecode 把 64MB rootfs 打进 assets（安装快、离线可用）vs ply 运行时下载；torvox 的 bootstrap 从网络拉取（有版本固定），方案不同但 onecode 的"assets 内置"值得权衡（APK 体积代价） |
| 命令队列与历史 | ✅ 有（输入批处理 InputBatchBuffer） | onecode `QueuedCommand` + `commandMutex` 串行化 + `commandQueue`（TerminalModels.kt:12–16）与 torvox 输入批处理思路同源 |
| 输出解析（进度行/提示符检测） | ❌ torvox 无此层（libghostty 全权处理） | onecode 因有"命令级"UI（回显/输出页）才需要；torvox 是流式终端 UI，**不需要**，但 `isInteractivePrompt`（sudo 密码检测）可作"输入焦点提示"产品功能候选 |
| AIDL 服务化 | ❌ torvox 同进程 JNI | onecode 的 TerminalService 为"核心脱离 UI 生命周期/跨进程复用"设计；torvox 目前单进程架构（前台服务保活）更简单。**若未来做"终端引擎作为独立可复用组件"再考虑** |
| SSH 终端 + SFTP 文件浏览 | ❌ torvox 本地终端 | onecode 的 `TerminalProvider` 抽象（LOCAL/SSH/ADB 可插拔）是扩展性示范；torvox 若加 SSH 会话可借鉴其 provider 接口分层 |
| SAF DocumentsProvider | ✅ 有（TerminalDocumentsProvider） | onecode 的 UbuntuDocumentsProvider 暴露 proot 内文件系统；torvox 已有同类组件，可对照 Uri 映射实现 |
| Nerd Font 回退 | ✅ 有（setUseNerdFontGlyphs） | onecode 三态字形归属缓存（TextMetrics.kt:51–97）是 CPU 端方案；torvox GPU 端有 fontdb+swash，思路等价 |
| 隐私黑屏覆盖 | ❌ torvox 无 | **onecode 独有亮点**：切后台盖黑层防截屏（MainActivity.kt:190–213）。torvox 是隐私敏感的终端应用，**值得吸收** |
| 手势排除区 + 沉浸式 | ✅ torvox 全屏终端 | onecode 的边缘长按唤出导航栏 + `setSystemGestureExclusionRects`（MainActivity.kt:78–85）细节可参考 |
| 双击返回退出 | ✅ 有 | 一致 |
| 语法高亮命令输入 | ✅ 有（输入框高亮） | 一致 |
| 缩放手势结束一次性 resize | ✅ torvox 有字体缩放 | onecode `onScaleEnd` 延迟同步 SIGWINCH（GestureHandler.kt:16–22 注释）是**避免 TUI 错位的正确做法**，torvox 的缩放实现应核对是否同样延迟 |

### 3.4 依赖分析

- 依赖面：AGP 8.6 / Kotlin 1.9.22 / Compose BOM 2024.04.01 / coroutines / navigation / kotlinx-serialization / okhttp（下载）/ ssh（JSch 类）+ JNI C。**均为 2024 年初水平，不激进**；无 wgpu/Rust。
- 适用性：对 torvox **无直接依赖可吸收**（torvox 的渲染/解析栈已定）；可借鉴的是**纯 Kotlin 侧架构模式**（单例 manager + StateFlow + 事件 SharedFlow + provider 抽象）。
- 先进性：自研 ANSI 解析器 vs libghostty-vt —— 从"终端正确性"看是**倒退**（onecode 是产品妥协，非技术先进）；其价值不在引擎而在外围产品层。

### 3.5 可吸收到 torvox 的具体内容

1. **隐私黑屏覆盖**（直接可做）：MainActivity.kt:190–213 模式 → torvox 的 `MainActivity`/`TerminalActivity` 在 `onUserLeaveHint`/`onPause` 加全屏黑色遮罩 View，`onResume` 移除。代码注释建议：
   ```kotlin
   // 隐私保护：切后台立即盖黑层，防止系统最近任务/截屏泄露终端内容。
   // 参照 OnecodeTerminal MainActivity.showBlackOverlay()。
   ```
2. **就绪判定用"首个提示符"**：torvox 安装器/引导流程可把 `SessionInitState.AWAITING_FIRST_PROMPT`（TerminalModels.kt:58–61）作为"引导完成"信号源之一（结合现有 bootstrap 完成检测）。
3. **缩放手势延迟同步**：核对 torvox 字体缩放是否在 `onScaleEnd` 一次性触发 resize+SIGWINCH（onecode GestureHandler.kt:16–22 注释的坑位）。
4. **自适应帧率/脏区域**（可选）：torvox 渲染线程若在空闲时仍全速提交，可参考 `AdaptiveFrameRateController`（PerformanceOptimizer.kt:62–90）的空闲降帧（5s 无输出 → 低频轮询）；`DirtyRegionTracker` 的合并阈值思想对应 torvox"脏行→重建 instance"的粒度决策。
5. **会话状态结构**（对照检查）：torvox 若需要按会话保存 scrollOffsetY/字体缩放等，参考 `TerminalSessionData` 的 `@Transient` + 状态集中管理方式（TerminalModels.kt:63–105）。
6. **AIDL 服务化**（暂缓，记录为未来方向）：若 torvox 未来要"终端引擎复用/脱离 UI 生命周期"，`ITerminalService.aidl` 的 8 方法集（createSession/switchToSession/closeSession/sendCommand/sendInterruptSignal/registerCallback/unregisterCallback/requestStateUpdate）是经过实践的最小接口集。

### 3.6 项目文档吸收价值

- README 的 AIDL 接口表（方法/参数/返回值/描述）是**优秀的接口文档样板**，torvox 的 JNI 桥（14 个 `#[no_mangle]` 函数）可仿照其表格化文档风格。
- "terminal-core 可复用模块"定位 + Git 子模块组织：torvox 若拆 native 引擎为独立仓库可参考其边界划分文档（README 的 Module Responsibilities 章节）。
- 中英双语 README（README.md + README_EN.md）与 `values/values-en` 双语言资源：torvox 的多语言资源组织可参考。

---

## 4. cpmdroid（avwohl/cpmdroid）— 低价值（非终端模拟器，是 CP/M 模拟器）

### 4.1 项目定位与完整架构

Z80/CP/M 模拟器（Android 手机/平板），基于 RomWBW HBIOS 平台：C++ 核心（qkz80 Z80 CPU + HBIOS dispatch + 内存分页，来自兄弟仓库 `cpmemu`/`romwbw_emu`）+ JNI 桥（`emu_io_android.cpp`）+ Kotlin 层（`EmulatorEngine` 封装 + `TerminalView` 自绘 VT100 终端 + 磁盘管理/帮助/设置 UI）。ROM 内置（assets `emu_avw.rom`，512KB），磁盘镜像从 ioscpM GitHub release（钉死 tag v1.4.5）下载。**注意：这是"模拟器 + 终端视图"，不是终端模拟器**——与 torvox 的可比部分只有 `TerminalView.kt`（VT100 解析 + 自绘）和输入/键盘遮挡处理。

```
Kotlin: MainActivity ── EmulatorEngine(JNI) ──> C++: AndroidEmulatorDelegate
             │                                          ├─ qkz80 (Z80 CPU)
             │                                          ├─ HBIOSDispatch (BIOS 调用)
             │                                          └─ banked_mem (内存分页)
             └── TerminalView（VT100 解析 + Canvas 自绘 + 滚动回退 + IME）
```

主循环：单线程 executor 每 `FRAME_DELAY_MS=16ms` 执行一批 50000 条指令（`runBatch`），空闲时降频 `IDLE_DELAY_MS=100ms`（10Hz）；guest 输出经 `onOutput` 回调 → `TerminalView.processOutput`；guest 输入经 `queueInput`。

### 4.2 文件功能说明

#### app/src/main/java/com/awohl/cpmdroid/TerminalView.kt（717 行）—— 唯一与 torvox 相关的文件

| 行号 | 符号 | 功能 |
|------|------|------|
| 22–31 | `class TerminalView : View` + companion | `MIN_ROWS=24`、`MIN_COLS=80`（**固定 CP/M 尺寸**） |
| 33–64 | 状态字段 | `rows/cols/visibleCols`；`screenBuffer/colorBuffer`（Array[rows][cols]）；光标；**滚动回退**：`historyChars/historyColors`（ArrayDeque，`scrollbackLines=1000`）、`userScrollUp`（用户上滚行数）、`fullHeight`（键盘隐藏时的完整高度，旋转时重置）、触摸跟踪（`touchDownY/touchDownScrollUp/isDragging`） |
| 66–106 | Paint 组 + 字体缩放 | `fontScaleSetting`（14=100%，8–24 范围）；`wrapLines` |
| 107–138 | VT100 解析状态 | `escapeState`（状态机）、`escapeParams`、`currentFgColor`；`ToneGenerator` 响铃；**CGA 16 色调色板**（main.c 风格 `Color.rgb(0,0,170)` 等） |
| 139–143 | `setInputListener(listener: (Int) -> Unit)` | 按键/触摸 → 外部（模拟器输入队列） |
| 144–156 | `playBell()` | ToneGenerator 蜂鸣 |
| 157–173 | `copyScreenToClipboard()` | 全屏字符+颜色 → 纯文本剪贴板（**颜色信息丢弃**） |
| 174–191 | `pasteFromClipboard()` | 剪贴板文本逐字符 `sendChar` |
| 192–198 | `sendChar(ch)` | 转义（`\u001b` 等）→ listener |
| 199–227 | `onTouchEvent` | **拖拽滚动**：按下记录位置，移动超过阈值进入拖拽（`userScrollUp` 增减，钳制范围）；**轻点（无拖拽）→ 弹键盘**；`performClick` 兼容 |
| 228–263 | IME 集成 | `onCheckIsTextEditor=true`；`onCreateInputConnection` 返回 `BaseInputConnection` 匿名类：`commitText`（逐字符发送，含回车）、`deleteSurroundingText`（发送 DEL）、`sendKeyEvent`；**物理键盘**走 `onKeyDown` |
| 265–353 | `handleKeyDown` | 硬件键盘映射：方向键 → CSI、Backspace → DEL（0x7f）、Enter → CR、Tab；`isCtrlPressed` 组合（Ctrl+C 等）；音量键可映射 |
| 354–373 | `onSizeChanged`/`setPadding`/`recalculateSize` | 尺寸变化 → `calculateFontSize`；**setPadding 监听键盘 insets 变化** |
| 374–434 | `calculateFontSize()` | 按视口高计算字号使 `MIN_ROWS` 全显示；**`fullHeight` 记录键盘隐藏时高度**；键盘弹出时（高度缩水）**不缩字号，改为滚动**（见 onDraw） |
| 435–462 | `resizeBuffers(newRows, newCols)` | 重建缓冲并保留内容 |
| 463–507 | `onDraw` | **双模式**：视口 ≥24 行 → 实时屏**底部锚定**，历史在屏幕上方填充，`userScrollUp` 决定偏移（`historyStart` 计算）；视口 <24 行（键盘弹出）→ 在实时屏内滚动到光标行；背景铺黑 |
| 508–520 | `drawRow` | 逐字符 `drawText`，颜色取 colorBuffer |
| 521–529 | `drawCursor` | 实心块光标 |
| 530–540 | `processOutput(data)` | 字节流逐字符 `processChar`；**新输出重置 `userScrollUp=0`（自动吸回实时屏）** |
| 541–580 | `processChar` | 状态机：普通字符/ESC/CSI 参数收集；`\r` 光标归零（不立即滚动）、`\n` `newLine`、`\b` 退格、`\a` 响铃 |
| 581–622 | `processCSI(command)` | VT100 子集：`A/B/C/D` 光标移动、`H/f` 定位、`J` 清屏（`2J` 全清）、`K` 清行（`0/1/2`）、`m` SGR（30–37/40–47/1/7）、`h/l` 模式（忽略）、`?25h/l` 光标显隐 |
| 623–641 | `putChar` | 写单元格 + 自动换行（`wrapLines` 时） |
| 642–649 | `newLine()` | 光标到底部时 `scrollUp`（推入历史） |
| 650–667 | `scrollUp()` | **历史滚动核心**：顶行推入 `historyChars/historyColors`（超 `scrollbackLines` 丢弃最旧） |
| 668–716 | 清屏系列 | `clearScreen/clearToEnd/clearToBeginning/clearLine/clearLineToEnd/clearLineToBeginning/clear` |

#### app/src/main/java/com/awohl/cpmdroid/EmulatorEngine.kt（178 行）
| 行号 | 符号 | 功能 |
|------|------|------|
| 7–21 | companion | `INSTRUCTIONS_PER_BATCH=50000`；宿主文件传输状态常量（IDLE/WAITING_READ/READING/WRITING/WRITE_READY，与 C 侧 `emu_io.h` 对应）；`System.loadLibrary("cpmdroid")` |
| 27–65 | 33 个 `external` 声明 | 分组：生命周期（init/destroy/loadRom/loadDisk/completeInit/run/stop）、输入（isWaitingForInput/queueInput/queueInputString/reset）、磁盘（slice/loaded/close）、宿主文件传输（8 个）、NVRAM（5 个）、manifest 写警告（4 个）、脏盘持久化（3 个） |
| 67–177 | Kotlin 包装 | `runBatch()` 以 `AtomicBoolean running` 门控；`onOutput(data)` 为 **native → Kotlin 回调**（JNI 反向调用） |

#### app/src/main/java/com/awohl/cpmdroid/MainActivity.kt（1025 行）
| 行号 | 符号 | 功能 |
|------|------|------|
| 36–98 | 字段 | 双频 run loop：`FRAME_DELAY_MS=16`（60fps 执行）/`IDLE_DELAY_MS=100`（10Hz 等待输入）；`runLoop` Runnable 里 `executor.execute { runBatch }`；**保存节流**：NVRAM 每 5s、脏盘定期；`loadedDiskFilenames[16]`（保存时按"实际挂载文件名"而非当前设置槽——槽位重排中途也能正确持久化）；`failedSaveUnits`（保存失败重试集合，注释说明 dirty 标志不跨重启，用字节保留重试） |
| 99–143 | `runLoop.run` | 批执行 → `checkHostFileState`（R8/W8 宿主文件传输轮询）→ 周期保存 |
| 151–281 | `onCreate` | 沉浸式 edge-to-edge；`ViewTreeObserver.OnGlobalLayoutListener` **键盘 insets 检测**（Method 2 第三方键盘回退分支：**恢复根 padding 为 systemBars + 重算**，否则第三方 IME 收起时留空白条）；控制条/工具栏/下载遮罩装配 |
| 282–288 | `wakeRunLoop` | 输入后立刻唤醒循环（不等 100ms 空闲节拍） |
| 289–318 | `setupEmulator` | 监听 `setOutputListener`；`onOutput` → `terminalView.processOutput` + `runOnUiThread` |
| 319–354 | `setupControlStrip` | **Ctrl（controlify：下一键变控制字符）/Esc/Tab/Copy/Paste 按钮条** |
| 363–393 | `setupToolbar` | 播放/重启/设置/帮助/关于 |
| 402–409 | `createVersionBanner` | 启动横幅字节（版本信息写入终端） |
| 410–456 | `loadDisksAndConfigureSlices` | 磁盘槽 → 自动分片（2/4/8 盘 → 2/4/8 slices） |
| 527–543 | `checkFirstLaunchAndLoad` | **首启判定**：`isFirstLaunchDone` 或槽内文件缺失 → 下载默认盘 |
| 545–606 | `downloadDefaultDisk` | 拉目录（catalog）→ 找 `defaultSlot==0` 的盘 → 下载 + 进度遮罩 |
| 624–693 | `loadRomAndDisks` | assets ROM + 槽位磁盘 → `loadDisk` → `completeInit` |
| 694–703 | `startEmulation` | `start()` + 首个 runLoop post |
| 704–713 | `onWindowFocusChanged` | 焦点恢复时重发当前屏幕（防状态丢失） |
| 714–754 | `stopEmulation`/`bootEmulation` | 停止 + 保存 NVRAM/脏盘；重启确认对话框 |
| 755–863 | 宿主文件传输 | `checkHostFileState`（guest R8 请求读 / W8 请求写）→ `handleHostFileRead`（Android 文件 → `nativeProvideHostFileData`）/`handleHostFileWrite`（`nativeGetHostFileWriteData` → 写 Exports 目录） |
| 864–875 | `updateStatus` | 状态栏文本（运行/等待输入/暂停） |
| 876–950 | 生命周期 | `onResume` 恢复运行 + 重新加载设置；`onPause` 暂停 + 保存；`onDestroy` 释放 |
| 973–998 | `saveNvramIfNeeded` | 周期/退出时把 NVRAM 写回磁盘 |
| 999–1024 | `saveDirtyDisks` | 脏盘 → `getDiskData` → `savePersistedDisk`（失败 Toast + 记入 `failedSaveUnits` 重试） |

#### app/src/main/cpp/emu_io_android.cpp（~1100 行，首 200 行已读）
| 行号 | 符号 | 功能 |
|------|------|------|
| 36–77 | `class AndroidEmulatorDelegate : HBIOSCPUDelegate` | 内存/HBIOS 访问、RAM bank 初始化（共享 bitmap）、halt/未实现操作码日志、debug 日志 |
| 83–112 | `class EmulatorState` | 封装 memory/cpu/hbios/delegate；`setBlockingAllowed(false)`（Android 非阻塞 I/O）；析构逆序释放 |
| 118–120 | 全局状态 | `g_emu`/`g_running`/`g_initialized` |
| 194–199 | `emu_io_init/cleanup` | 空实现 |
| 中后段（推断） | JNI 导出（`Java_com_awohl_cpmdroid_EmulatorEngine_native*`） | 与 EmulatorEngine.kt 的 33 个 external 一一对应：ROM/磁盘装载、指令批执行、输入队列、宿主文件传输状态机、NVRAM 字符串 API、脏盘字节导出 |

#### app/src/main/cpp/CMakeLists.txt
- 通过 `../../../../../romwbw_emu/src`、`../../../../../cpmemu/src` 引用**兄弟目录源码**（构建时路径校验 `FATAL_ERROR`）；`-fvisibility=hidden`；链 android/log。

#### 其余文件
- `SettingsActivity.kt`（~300 行）：磁盘槽配置、字体缩放 SeekBar、wrapLines、首启状态、manifest 写警告开关（SharedPreferences，`CURRENT_PREFS_VERSION=3` 版本化迁移）
- `HelpActivity.kt` / `HelpTopicActivity.kt`：**帮助文档从 GitHub release 下载**（`release_assets/help_*.md`：nzcom/qpm/quick_start/zpm3/zsdos 主题）
- `DiskCatalogAdapter.kt` / `data/DiskCatalogRepository.kt`（OkHttp 拉 GitHub release 目录 JSON）/ `data/DiskDownloadManager.kt`（下载 + 校验 + `getExternalFilesDir/Disks` 持久化 + `savePersistedDisk`）/ `data/DiskInfo.kt` / `data/EmulatorSettings.kt` / `data/SettingsRepository.kt`
- `docs/`：Play Store 描述、release notes、midi.md、bug 记录、测试邀请
- `app/build.gradle.kts`、`assets/emu_avw.rom`（512KB 内置 ROM）、`layout-land/activity_main.xml`（横屏布局）、`PRIVACY_POLICY.md`、`CHANGELOG.md`、`todo.txt`、`WIP.md`（见下）

#### WIP.md（重要过程文档）
记录 2026-07-25 的一次"键盘遮挡 + 滚动回退"修复：三个联动修复（键盘隐藏分支恢复 padding；视口过短时屏内滚动到光标；固定 24 行 + 1000 行历史），含**设计选项对比（3 选 1：固定屏+提示符在底+滚动回退）**、adb 调试命令、跨仓库上下文（z80cpmw/ioscpm 发布联动）。是**优秀的"未提交变更过程记录"样板**。

### 4.3 功能对比 torvox

| 功能 | torvox 有？ | 对比 |
|------|-----------|------|
| VT 解析 | ✅ libghostty-vt | cpmdroid 是 ~80 行状态机 VT100 子集（TerminalView.kt:541–622），完整度与 libghostty 差距巨大，**不吸收** |
| 渲染 | ✅ wgpu | cpmdroid Canvas 逐字绘制 + 双缓冲数组；**路线不同**，其"固定 24 行 + 底部锚定 + 历史填上方"的视口模型与 torvox 滚动回退语义等价，可对照 |
| 滚动回退 | ✅ 有 | cpmdroid 的 `ArrayDeque` 历史 + `userScrollUp` 钳制 + **新输出自动吸回实时屏**（TerminalView.kt:530–540）——torvox 的滚动行为应核对同样的"新输出复位"语义 |
| 键盘遮挡处理 | ✅ 有（TerminalSurface + IME） | cpmdroid 三方案：**键盘弹出时在实时屏内滚动到光标、不缩字号**（TerminalView.kt:374–434、463–507）——torvox 若键盘弹起时字体缩放或内容被挡，可参考此策略 |
| 物理键盘 | ✅ 有 | cpmdroid `handleKeyDown` 的 Ctrl 组合 + 方向键映射（TerminalView.kt:265–353）可作映射对照 |
| 剪贴板复制/粘贴 | ✅ 有（ClipboardAccess/PasteChunker） | cpmdroid 全屏复制（157–173）语义简单；torvox 更全（选择/分块粘贴） |
| 修饰键控制条 | ✅ 有（ModifierBar） | cpmdroid controlify（下一键变 Ctrl）与 torvox 的 CTRL/ALT 切换条思路一致 |
| 会话/滚动/搜索 | ✅ 有 | cpmdroid 无（单屏模拟器） |
| 响铃 | ✅ 有（libghostty 事件） | cpmdroid ToneGenerator 实现（144–156）可作 torvox"响铃"功能的行为参考 |
| R8/W8 文件传输 | ❌ torvox 无 | 与 torvox 场景无关（guest 是 CP/M） |
| 磁盘镜像下载目录 | ❌ | 无关 |

### 4.4 依赖分析

- 依赖：qkz80/romwbw_emu（兄弟仓库源码直引，**非包管理器**）、OkHttp、Kotlin coroutines。NDK 27。
- 适用性：**不适用** torvox——无 GPU、无 libghostty、无 Rust；其 C++ 核心是 Z80 模拟，与终端无关。
- 先进性：不先进（VT100 子集 + Canvas 绘制是 2010 年代终端方案）；但**双频 run loop（16ms 执行 / 100ms 空闲）与 NVRAM/脏盘节流保存**是嵌入式风格的良好实践。

### 4.5 可吸收到 torvox 的具体内容

1. **键盘弹出时"屏内滚动而非缩字号"**（对照检查）：cpmdroid TerminalView.kt:374–434 的思路——torvox 的 TerminalSurface 在 IME 弹出时应保持字号不变、滚动视口使光标可见；`fullHeight` 记录"键盘隐藏高度"的做法可移植。
2. **新输出复位滚动位置**：`processOutput` 重置 `userScrollUp=0`（TerminalView.kt:530–540）——torvox 滚动回退实现应保证收到新输出时回到实时屏。
3. **节流持久化**：cpmdroid 的"NVRAM 每 5s + 退出时保存 + 失败重试集合"（MainActivity.kt:973–998、999–1024）——torvox 若有需要持久化的会话状态（如上次会话目录），可参照此节流+重试模式，避免每次状态变化都写盘。
4. **WIP.md 过程记录**：torvox 的 `docs/progress/` 可借鉴其"未提交变更 + 设计选项 + 验证命令 + 跨仓库上下文"的结构。
5. 反模式警示：**manifest 写警告**（下载盘可被更新覆盖）——torvox 若从网络下载 bootstrap 资产，应对"可被替换的资产"做写保护提示（MainActivity.kt:503–525）。

### 4.6 项目文档吸收价值

- `WIP.md`：变更过程文档样板（含 adb/logcat 验证命令），torvox 的调试文档可参考。
- `CHANGELOG.md` + `docs/release_notes.txt` + `docs/PLAY_STORE_DESCRIPTION.txt`：发布文档体系完整。
- `docs/bug1.txt`/`docs/test-invite.txt`：真实 bug 复现记录与测试邀请文案，展示小型独立开发者的文档实践。
- README 的"技术细节"章节（架构 ASCII 图 + 磁盘格式说明）：面向用户的架构解释方式可参考。

---

## 5. 四项目 × torvox 功能矩阵总表

| 功能点 | torvox | ghostling | onecode | cpmdroid | ply |
|--------|--------|-----------|---------|----------|-----|
| PTY + shell 环境 | ✅ Rust nix | ✅ forkpty C | ✅ JNI pty.c | ✅（模拟器内建） | ❌ `sh -c` |
| VT 解析 | ✅ libghostty-vt | ✅ libghostty-vt | ⚠️ 自研子集 | ⚠️ VT100 子集 | ❌ |
| GPU 渲染 | ✅ wgpu | ⚠️ Raylib 2D | ⚠️ Canvas | ⚠️ Canvas | ❌ |
| Kitty keyboard/graphics | ✅ | ✅（有已知输入缺陷） | ❌ | ❌ | ❌ |
| 滚动回退 | ✅ | ✅（scrollbar 拖拽） | ✅（200 行） | ✅（1000 行） | ❌ |
| 会话管理 | ✅ SessionDrawer | ❌ | ✅ SessionManager | ❌ | ❌ |
| 搜索 UI | ✅ | ❌（lib 有内部） | ❌ | ❌ | ❌ |
| 选择/复制 | ✅ | ❌ | ✅ 选择+复制菜单 | ✅ 全屏复制 | ❌ |
| 多进程服务化 | ❌ 同进程 | ❌ | ✅ AIDL | ❌ | ❌ |
| SSH | ❌ | ❌ | ✅ | ❌ | ❌ |
| proot/distro | ❌ bootstrap | ❌ | ✅ proot Ubuntu | ❌ | ⚠️ proot 未接线 |
| 隐私黑屏 | ❌ | ❌ | ✅ | ❌ | ❌ |
| 帮助文档下载 | ❌ | ❌ | ✅ | ✅ | ❌ |
| MCP/AI 集成 | ✅ | ❌ | ❌ | ❌ | ❌ |

## 6. 综合结论与吸收优先级

**按吸收价值排序**：

1. **ghostling（高）**：与 torvox 共享 libghostty-vt 内核，是官方 API 用法与 effects 的权威参考。立即行动项：effects 完整性对照（write_pty/DA/xtversion）、EIO→EOF 注释、kitty graphics 管线核对、Zig 路径缓存坑。
2. **onecode（中高）**：架构层经验（会话状态机、AIDL 接口集、provider 抽象、隐私黑屏、缩放手势延迟同步）。直接可做：隐私黑屏覆盖；核对缩放同步与就绪判定。
3. **cpmdroid（低-中）**：键盘遮挡"屏内滚动不缩字号"、新输出复位滚动、节流持久化、WIP.md 过程文档样板。
4. **ply（低）**：无功能性吸收；作为环境变量设置对照与 `curl | sh` 反模式记录。

**总原则**：这 4 个仓库没有任何一个在"终端引擎层"（解析/渲染/输入编码）超越 torvox——torvox 的 libghostty-vt + wgpu 栈是其中技术最先进的；参考价值集中在**外围产品层**（隐私、生命周期、持久化、初始化 UX、文档组织）与 **libghostty API 细节**（ghostling）。

### ply deep-v3 增量（复核第 1 轮：SECURITY.md / CONTRIBUTING.md）

- **SECURITY.md 攻击面声明**（ply）：① PTY escape sequence parsing ② Input handling（malformed Unicode）③ Filesystem access via local shell——**torvox 可吸收此"安全范围声明"形式**（docs/security.md 或 SECURITY.md），明确攻击面便于安全审查聚焦。功能上 torvox 均已覆盖（libghostty-vt 解析、UTF-8 输入、bootstrap 文件系统）。
- CONTRIBUTING.md：开发流程（测试/构建命令）——与 torvox AGENTS.md 同功能，无新内容。
