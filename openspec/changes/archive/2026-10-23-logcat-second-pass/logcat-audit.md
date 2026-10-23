# W/E/F 审计汇总（第二轮，2026-10-23）

## 数据来源与保留原则

- 审计对象仍是用户提供的两份运行日志（2026-10-22）；原始文件只在分析期间使用，
  **不入库、不保留副本**。
- 仓内只保留聚合计数、标签类别、归因与结论；不保留原始行、时间戳、PID、设备标识、
  下载链接或可还原设备的细节。
- 统计口径：按级别行归并为一条，只统计 `W/`、`E/`；两份合计 **753 条**（W 712、E 41），
  第一份 215 条、第二份 538 条。与首轮审计的计数一致，说明两轮读的是同一批日志。

## 逐标签归因表（753 条）

| 标签 | 条数 | 归因 | 结论 |
| --- | --- | --- | --- |
| `W/Runtime` | 551 | 应用自有慢帧/慢循环诊断 | 诊断输出，非缺陷；已按 500ms 窗口限频 |
| 外部 proot 环境的 shell 标签 | 68 | 外部 proot 环境下的 SELinux 拒绝 | 平台/外部，无应用栈 |
| `E/mali_gralloc` | 32 | 厂商 GPU 分配器探测 | 平台/驱动，无应用开关 |
| `W/ziparchive` | 12 | 系统框架资源读取 | 平台侧，无应用栈 |
| `W/libc` | 11 | 系统属性读取被拒 | 平台侧，无应用栈 |
| `W/com.termux` | 11 | 隐藏 API 反射（平台标记 allowed） | 平台侧，targetSdk 兼容路径 |
| `W/Parcel` | 10 | 系统 Binder 通信 | 平台侧，无应用栈 |
| 外部 proot 运行器与子进程标签 | 8 + 2 = 10 | 外部 proot 环境 | 外部，无应用栈 |
| `W/wgpu_core::instance` | 6 | wgpu 能力提示（downlevel flags） | 预期告警，当前渲染路径不使用该能力 |
| `W/native::terminal::session` | 6 | 应用自有会话收尾日志 | **已降级**：正常退化路径改记 debug |
| `E/SurfaceSyncer` | 5 | 系统合成器 | 平台侧，无应用栈 |
| `W/stream` | 5 | 上游 CSI 未实现（DECSACE） | **上游缺口**，见下文 |
| `W/kitty_gfx` | 5 | 上游 kitty 介质拒绝 | **应用侧已启用 file/temp 介质**；共享内存为上游平台限制 |
| `W/FeatureOpt` | 4 | 系统特性查询 | 平台侧，无应用栈 |
| `W/TerminalSurface` | 3 | 应用自有 Surface 未就绪延迟处理 | **已降级**：改记 debug |
| 外部 shell 与目录列举工具标签 | 3 + 1 = 4 | 外部程序输出 | 外部，无应用栈 |
| `E/Typeface` | 2 | 系统字体资源 | 平台侧，应用不经过该 API |
| `W/DefaultDispatch` | 2 | 系统 Handler 派发 | 平台侧，无应用栈 |
| `W/InputEventReceiver` | 2 | 平台输入通道 | 平台侧，无应用栈 |

| `E/com.termux` | 1 | ART 调试器代理提示 | 平台侧，非缺陷 |

| `E/native::terminal::session` | 1 | 应用自有 execve errno 误判 | **已修复**：errno 只接受 1..=133 |
| `W/CheckTime` | 1 | 系统时间校验 | 平台侧，无应用栈 |
| `W/native::render::pass` | 1 | 应用自有渲染分段诊断 | 诊断输出，非缺陷 |

合计 753 条，与首轮一致。

## 应用自身条目：逐条结论与当前状态

| 条目 | 根因 | 当前状态 | 落地位置 |
| --- | --- | --- | --- |
| `W/native::terminal::session` 进程组不可用（6 条） | 收尾路径把四种退化原因（子进程已退出、与宿主同组、取不到组号、其他 `getpgid` 错误）都写成同一条 warn | 已按真实原因分级：前两种记 debug，取不到组号才记 warn | `native/src/terminal/session.rs` 的 `request_exit` |
| `E/native::terminal::session` execve errno 误判（1 条） | 退出码 ≥ 100 一律当 `100+errno`，把 shell 正常退出码 254 解成不存在的 errno 154 | 已修：只接受 1..=133 的 errno，子进程侧封顶 133 | `native/src/terminal/session.rs` 的 `execve_errno_from_exit_code` |
| `W/TerminalSurface` surface 未就绪（3 条） | 启动期 `surfaceCreated` 之前收到尺寸变化，属正常时序 | 已降级为 debug | `TerminalSurface.kt` 的 `applySurfaceResize` |
| `W/Runtime` 慢帧/慢循环（551 条） | 诊断输出本身；551 条中 379 条落在滚动期间 | 保留诊断但按 500ms 窗口限频；取帧等待的归因见首轮审计 | `TerminalRuntime.kt` 的 `SLOW_FRAME`/`SLOW_LOOP` |
| `W/native::render::pass` 分段诊断（1 条） | 渲染阶段计时 | 诊断输出，非缺陷 | `native/src/render/pass.rs` |

结论：应用自身条目已全部有根因与落地位置，日志中不再残留未解释的应用 W/E。

## 上游协议条目：两层事实

| 条目 | 终端上游 master | 绑定层所钉提交 | 应用侧义务 |
| --- | --- | --- | --- |
| `W/stream` DECSACE（`CSI Ps ; Ps ; Ps SP x`） | `src/terminal/stream.zig` 的 CSI 分派无 `'x'` 分支，全文无 DECSACE 字样，落入 `unimplemented CSI action` 告警分支 | 同左 | 不自研 VT 解析绕过；该协议未定义应答，故不伪造应答；日志由上游写出，应用只如实转发 |
| `W/kitty_gfx` `EINVAL: unsupported medium` | `src/terminal/kitty/graphics_image.zig` 的 `readSharedMemory` 对 Android 直接返回 `UnsupportedMedium`（注释：Android 不支持 POSIX 共享内存），`shm_open` 另需 libc | 同左 | 不自建共享内存通道绕过；kitty 协议定义了应答，故按规范回明确失败 |

取证方式：上游 master 取当日 `main` 分支源码，绑定层取 `libghostty-vt-sys` 构建时
vendored 的终端源码。两处事实都可复现。

## kitty 介质错误的两种来源（同一句日志文案）

`EINVAL: unsupported medium` 有两个来源，日志文案相同，必须分开归因：

1. **介质未启用**（应用侧可修）：终端初始化默认只开 direct 介质，客户端用
   `t=f`/`t=s` 时上游按 limits 拒绝。**已修**：初始化时启用 file、shared memory、
   temporary file 三种介质并给出临时目录（`native/src/terminal/ghostty_terminal/internal.rs`）。
2. **平台不支持该介质**（应用侧不可修）：共享内存介质在 Android 上由上游无条件
   拒绝（见上表）。

客户端侧的表现是 yazi 先探测共享内存能力、失败即回落 base64（direct）通道，
故介质启用修复后 yazi 的图像预览走 direct 通道即可显示。

## 复现与取证

1. 启动应用，等 shell 提示符出现并静置；
2. 产生至少两屏历史；
3. 连续上滑/下滑 3–5 秒；
4. 抓取包含 `SLOW_FRAME`、`SLOW_LOOP`、`SLOW_RENDER_INNER`、`SLOW_CELL_BUILD` 的日志，
   只保留聚合结果，不把原始日志加入仓库。

## 遗留（须真机复测，仓内无法判定）

- 滚动卡顿/闪烁的取帧等待占比（首轮已按分段证据收敛为两帧排队，待 Mali 真机复测）；
- kitty 图像在真实客户端（yazi、kitten icat）下的滚动跟随与占位符渲染；
- 主题跟随系统日夜切换、修饰键栏方向键的真实设备行为。
