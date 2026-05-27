# Torvox 技术规范

> Torvox 实现什么、目标是什么、合规性如何衡量的精确规范。

---

## 1. VT 标准合规性

Torvox 追求所有 Android 终端模拟器中最全面的 VT 标准覆盖。合规性通过 `vttest` 套件、`xterm` 源码和参考实现验证。

### 核心 (MVP — 阶段 1)

| 标准 | 覆盖 | 备注 |
|------|------|------|
| **VT100** | ✅ 完整 | DEC AWB, CKM, DECCOLM (80/132), 原点模式, 滚动区域, 换行模式 |
| **VT220** | ⚠️ 部分 | Paul Williams 解析器全部 15 状态, DECSC/DECRC, DECSTBM (已实现); DECDHL ❌ |
| **VT320** | ❌ 未实现 | 8-bit 控制, DECRQSS, DECRQM, 选择性擦除, 矩形区域操作 — 均未实现 |
| **ECMA-48** | ❌ 未实现 | 仅实现部分 CSI 序列; "所有 CSI 序列" 和完整私有模式未实现 |
| **xterm** | ⚠️ 部分 | 256 色 ✅, 真彩色 ✅, 括号粘贴 ✅; DECSET/DECRST 仅少数模式 ❌ |

### 现代扩展 (MVP — 阶段 1)

| 扩展 | 状态 | 测试 |
|------|------|------|
| 真彩色 (24-bit) | ✅ | `vttest true-color` |
| 256 调色板 | ✅ | `Colin's 256colors2.pl` |
| 粗体/斜体/下划线/删除线 | ✅ | SGR 1-9, 21-29, 51-55 |
| 彩色下划线 (5 种样式) | ❌ | SGR 58:2/59:2 — 未实现。基本下划线存在 (SGR 4), 但彩色下划线未实现 |
| 双宽字符 | ❌ | DECDHL, DECSWL, DECDWL — 均未实现 |
| 括号粘贴模式 | ✅ | DEC 2004 |
| 鼠标跟踪 | ✅ | X10, VT200, SGR, SGR-pixels (DEC 1016) |
| DEC 定位器 | ❌ | CSI DECELR, DECRQLR — 未实现 |
| 焦点事件 | ✅ | CSI I/F |
| 同步输出 (DEC 2026) | ❌ | 未实现 |
| Sixel 图形 | ❌ | 未实现 |
| Kitty 图形协议 | ❌ | 未实现 |
| iTerm2 图像协议 | ❌ | 未实现 |
| **Kitty 键盘协议** | ✅ | 渐进增强: CSI u, push/pop/restore 配置。完整按键编码。 |
| **OSC 8 超链接** | ❌ | 未实现 |
| **OSC 52 剪贴板** | ❌ | 未实现 (未来工作) |
| **OSC 133 Shell 集成** | ❌ | 未实现 |
| **OSC 7 CWD** | ❌ | 未实现 |
| **OSC 4/10/11/12/17/19/110/111/112/708** | ❌ | 未实现 |
| **DECRQSS** | ❌ | 未实现 |
| **DECRQM** | ❌ | 未实现 |

### 阶段 1 之后

| 扩展 | 优先级 | 备注 |
|------|--------|------|
| 正则 URL 检测 | 高 | 无需 OSC 8 的可点击 URL |
| 像素级平滑滚动 | 中 | 需要视口重新架构 |
| GPU 计算着色器渲染 | 低 | Ferrum 灵感, 稳定后实施 |
| Sixel 动画帧 | 低 | 小众用例 |
| Tektronix 4014 仿真 | 低 | 遗留绘图仪 |

## 2. Unicode 支持

| 特性 | 级别 | 备注 |
|------|------|------|
| UTF-8 编码 | ✅ 完整 | Rust 字符串原生 UTF-8 |
| 字形簇聚类 | ⬜ 未实现 | UAX#29 — cosmic-text 处理基本聚类, 但完整 UAX#29 未验证 |
| Emoji (ZWJ 序列) | ⬜ 未测试 | 通过 cosmic-text + swash/skrifa 彩色 emoji, 但 ZWJ 序列未测试 |
| CJK 宽字符 | ⚠️ 部分 | East Asian Width 部分处理, 行双宽未实现 |
| 双向文本 | ❌ 未实现 | Unicode BIDI 算法未实现 |
| 零宽连接符 | ⬜ 未测试 | ZWNJ, ZWJ, 区域指示符 — 未测试 |
| 组合字符 | ⬜ 未测试 | COMBINING 标记, 包围标记 — 未测试 |
| 变体选择符 | ⬜ 未测试 | VS1-VS16 用于 emoji/文本呈现 — 未测试 |
| 私用区 | ⬜ 未计划 | 通过捆绑字体文件的 Nerd Font 字形 (阶段 2+) |

### Emoji 渲染策略

| 格式 | 状态 | 备注 |
|------|------|------|
| CBDT/CBLC | ⬜ | 遗留 Android emoji 格式, swash 原生支持 — 未验证 |
| sbix | ⬜ | Apple 格式, cosmic-text 支持 — 未验证 |
| COLR v0 | ⬜ | Windows/Chrome 格式, swash 支持 — 未验证 |
| COLR v1 | ⬜ | 现代格式, skrifa 0.42 支持, 需集成验证 — 未验证 |

**风险缓解**: 如果 COLRv1 集成失败, 捆绑遗留 CBDT 字体作为回退。

## 3. 渲染规范

### GPU 管线

| 属性 | 要求 | 验证 |
|------|------|------|
| 帧率 (空闲) | 0 FPS (无工作) | 性能分析: CPU 使用率 >0.5% |
| 帧率 (活跃) | 120 FPS 最低 | `find /` 基准 (典型终端操作负载) |
| 帧率 (突发) | ≥60 FPS @ 饱和 PTY 输出 | `cat /dev/zero` 基准 (Android 上实际 PTY 吞吐约 50-200MB/s, 非 1GB/s) |
| 输入 → 像素 | <5ms P95 | 自定义延迟探针 |
| 图集命中率 | >99% 稳态 | 图集缓存统计导出 |
| 空闲 GPU | <1% 利用率 | GPU 帧性能分析 |
| 内存 (图集) | <64MB 上限 | OOM@64 → LRU 驱逐 |

### 字体管线

| 阶段 | 库 | 配置 |
|------|-----|------|
| 发现 | `fontdb` | 系统字体 + 捆绑 `JetBrainsMono-Nerd-Font.ttf` |
| 成形 | `cosmic-text` 0.19 | 默认: `cursive`, `kern`, `liga`, `dlig`, `rlig`, `calt` |
| 缩放+光栅化 | `swash` 0.2.7 | 缩放 via 内部 skrifa 0.42 (`scale` feature), 光栅化 via zeno。无需单独依赖 skrifa crate。CBDT/COLR 彩色 emoji, 可变字体 @ 请求像素 |
| 图集 | `guillotiere` 0.7 | 2048×2048 或 4096×4096, 货架打包 |
| 缓存 | LRU | 按最后帧访问排序, OOM 时驱逐 |

### 色彩空间

| 空间 | 位深 | 用途 |
|------|------|------|
| sRGB | 8-bit/通道 (×4) | 默认色彩空间, 字形图集 |
| Display P3 | 阶段 5+ | HDR/广色域支持, 非初始目标 |

### GPU 驱动兼容性

| GPU | 已知问题 | 缓解 |
|-----|----------|------|
| Qualcomm Adreno | ✅ 无已知问题 | Vulkan 后端正常 |
| ARM Mali | ⚠️ `glPushDebugGroup` SIGSEGV | `DISCARD_HAL_LABELS` 标志, 优先 Vulkan |
| Intel | ✅ 无已知问题 | — |
| PowerVR | ❌ 大部分不支持 Vulkan 1.1 | minSdk 29 已过滤, 这些设备市场份额 <1% |

## 4. PTY 规范

### 本地 PTY

| 参数 | 值 | 备注 |
|------|-----|------|
| 接口 | `nix` 0.31 crate (直接 `forkpty()`) | `portable-pty` 不支持 Android, 必须直接使用 nix |
| Unix 后端 | `openpty()` / `forkpty()` | 通过 nix crate syscall |
| 子进程 | `/system/bin/sh` | 默认; 可配置 |
| Termios | Raw mode | UTF-8, 无流控, 无回显 |
| SIGWINCH | `ioctl(TIOCSWINSZ)` | 每次调整大小时 |
| W^X 变通 | Rust 多调用二进制 | `torvox-exec` — 单一二进制, 所有符号链接指向它 |
| 进程组 | `setsid()` | 每个 PTY 在自己的会话中 |
| kill 语义 | `kill_on_drop` | Drop 时发送 SIGHUP → SIGCONT → SIGKILL (递增式) |

### Android PTY 特殊处理

| 问题 | 解决方案 |
|------|----------|
| Android bionic `openpty()` 与 glibc 差异 | nix crate 直接 syscall, 已测试 |
| Android 14+ 后台 `exec()` 限制 | `torvox-exec` 多调用二进制 (Termux 模式) |
| SELinux 策略限制 | 应用沙盒内操作, 无需 root |
| PTY 从设备权限 | `grantpt()` + `unlockpt()` 通过 nix |

### 会话隔离

| 层 | 机制 |
|----|------|
| 进程 | 每个会话在自己的进程组中, `kill_on_drop` |
| 文件系统 | 标准 Android 沙盒 (`/data/data/io.torvox`) |
| 用户身份 | `SHELL` → 生成进程。无 root 执行。 |
| 环境 | 过滤环境: `TERM=torvox-direct`, `COLORTERM=truecolor`, `TERMINFO` |

## 5. 会话管理

### 状态持久化

| 数据 | 格式 | 位置 | 同步 |
|------|------|------|------|
| 打开的会话 | JSON | `app/sessions.json` | 会话创建/关闭时 |
| 终端状态 | postcard | `app/state/{id}.postcard` | 每 60s + 前台丢失时 |
| 配置 | TOML | `app/config.toml` | 变更时 |
| Shell 历史 | — | Shell 原生 | — |

**注意**: 不使用 `bincode`。bincode 3.0.0 被故意破坏 (RUSTSEC-2025-0141)，永久停止维护。使用 `postcard 1.1` 替代 (仅 dev-dependency，用于测试序列化 roundtrip)。

### 前台服务

- **保活**: Android 前台服务，类型 `FOREGROUND_SERVICE_SPECIAL_USE`
- **通知**: "Torvox — 2 个会话活跃"，含停止操作
- **OOM 优先级**: 前台服务 → 进程不太可能被杀
- **Wake lock**: 长时间运行命令期间的部分 wake lock (屏幕关闭 I/O)
- **Android 16 要求**: 必须在 Play Console 提交前台服务使用理由

## 6. MCP 服务器规范

Agent 传输协议是本地回环上的 JSON-RPC 2.0 服务器。**注意: 本节描述的 MCP 服务器是阶段 2+ 功能, 尚未实现。**

### 启动

- 端口: 动态分配, 写入 `app/agent.sock`
- 认证: Unix 域套接字权限 (0700)
- 默认: **关闭**。通过 Settings → Agent Access 启用。

### 核心工具 (阶段 2+)

| 工具 | 参数 | 返回 | 备注 |
|------|------|------|------|
| `session.create` | `{shell: "bash"}` | `{session_id}` | 生成新终端会话 |
| `session.write` | `{session_id, input}` | `{accepted}` | 写入 PTY 输入 |
| `session.read` | `{session_id, timeout_ms}` | `{output}` | 阻塞读取带超时 |
| `session.resize` | `{session_id, rows, cols}` | `{accepted}` | 终端调整大小 |
| `session.state` | `{session_id}` | `{grid, cursor, ...}` | 可见状态快照 |
| `session.list` | `{}` | `{sessions: [...]}` | 所有活跃会话 |
| `session.close` | `{session_id}` | `{accepted}` | 优雅终止会话 |

## 7. 性能目标 (Android Pixel 7)

| 指标 | 目标 | 测量方法 |
|------|------|----------|
| 冷启动 → shell | <500ms | `adb logcat` 首次 PTY 输出 |
| 热启动 (app bg→fg) | <50ms | 帧时间线 |
| PTY 输入 → PTY 回显显示 | <5ms P95 | `PANEFLOW_LATENCY_PROBE=1` 式仪器 |
| `find /` FPS | >90 FPS | `gfxinfo` |
| `cat /dev/zero` FPS | >60 FPS | `gfxinfo` |
| 空闲 CPU (无输出, 无输入) | <0.5% | `top` / `battery-historian` |
| 空闲内存 (1 会话) | <10MB | `adb shell dumpsys meminfo` |
| 滚动 10K 行 | <16ms | `Choreographer` |
| 字体冷启动 (首次渲染) | <200ms | 帧时间线 |

## 8. 目标平台

| 属性 | 值 | 备注 |
|------|-----|------|
| **minSdk** | 33 (Android 13) | Vulkan 1.3 支持从此版本起覆盖 95%+ 设备 |
| **targetSdk** | 36 (Android 16) | |
| **compileSdk** | 36 | |
| **GPU 要求** | Vulkan 1.3 (Android 13+ 原生支持) | wgpu 29 Vulkan 后端最低要求 |
| **CPU** | arm64-v8a (主要), x86_64 (模拟器) | |
| **推荐 RAM** | 4GB+ | |
| **Rust 工具链** | stable (固定在 `rust-toolchain.toml`) | Edition 2024 |
| **NDK** | r29 | |
| **Kotlin** | 2.3.21+ | K2 编译器稳定 |
| **Compose BOM** | 2026.05.00 | Material 3 |
| **AGP** | 9.0.1 | |

### minSdk 33 的理由

原方案 minSdk 26 依赖 OpenGL ES 3.1 作为 GPU 回退。但这与"GPU 从第一天起"的哲学矛盾——OpenGL ES 3.1 的 wgpu 后端不支持完整特性集。Vulkan 1.3 从 Android 13 (API 33) 起可用，覆盖 95%+ 活跃设备。对于 2026 年新项目，坚持 Vulkan-only 是正确选择。

## 9. 合规性测试

| 测试 | 目标 | 频率 |
|------|------|------|
| `vttest` | 100% 通过 | CI 每次提交 |
| `xterm` 256color 测试 | 100% 通过 | CI 每次提交 |
| `ESR-test` (转义序列) | 100% 通过 | CI 每次提交 |
| 模糊测试 (原始字节) | 100% 无崩溃 | CI 夜间 (1B 迭代) |
| 模糊测试 (OSC 序列) | 100% 无崩溃 | CI 夜间 |
| 模糊测试 (UTF-8 边缘情况) | 100% 无崩溃 | CI 夜间 |
| 延迟基准 | <5ms P95 | CI 每次提交 (回归门) |
| 吞吐量基准 | >90 FPS 负载下 | CI 每次提交 (回归门) |
| 内存泄漏检测 | 0 泄漏 | CI 每次提交 (MIRIFLAGS) |
| `cargo geiger` | 解析器中 0 `unsafe` | CI 每次提交 |
| `cargo nextest` | 所有测试通过 | CI 每次提交 (替代 cargo test) |
| `MIRI` | 无未定义行为 | CI 夜间 (关键路径) |
| `proptest` | 属性测试 10K+ 案例 | CI 每次提交 |
