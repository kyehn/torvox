# logcat W/E 审计（2026-10-22）

## 数据来源与方法

- 原始文件：[`logcat_2026-10-22_18-37-49.txt`](https://raw.githubusercontent.com/kyehn/torvox/kyehn-patch-1/logcat_2026-10-22_18-37-49.txt)（3816 条应用日志、13214 行，625 KB）。
- 解析规则：以 `[时间 Uid(...):pid:tid 级别/标签]` 为一条日志，紧随其后的非时间戳行并入该条；
  只统计 `W/`、`E/`，共 **215 条**（W 184、E 31）。
- 逐条清单：`logcat-audit-appendix.md`（215 行，按原始顺序，未删减）。
- 设备/进程：ZTE 折叠屏（`FeatureOpt: zte_foldable`），Mali-G57 / Vulkan，
  pid 1829 与 2300 两次应用进程，会话 1/2。

## 总表

| 级别/标签 | 条数 | 归类 | 处置 |
| --- | --- | --- | --- |
| `W/Runtime` | 106 | 慢帧/慢循环观测 | 记录证据；本变更不改渲染参数 |
| `E/mali_gralloc` | 24 | 厂商 GPU 分配器 | 已显式选 Rgba8Unorm；待新日志确认 R8 探测来源 |
| `W/fish` | 17 | SELinux granted | 外部包启动 proot；无需处理 |
| `W/Parcel` | 9 | 系统 Binder | 平台侧；无应用栈 |
| `W/libc` | 7 | 系统属性读取被拒 | 平台侧；无应用栈 |
| `W/prooted-2448-cB` | 7 | SELinux granted | 外部包；无需处理 |
| `W/ziparchive` | 6 | Unisoc 系统框架缺失文件 | 平台侧；无应用栈 |
| `W/native::terminal::session` | 6 | 收尾日志误分级 | **已修复** |
| `W/wgpu_core::instance` | 4 | wgpu downlevel 能力提示 | 预期告警；Mali 驱动能力 |
| `E/SurfaceSyncer` | 4 | SurfaceFlinger 同步 | 平台侧；无应用栈 |
| `W/stream` | 4 | Ghostty 未实现的 CSI | 上游解析器；不影响终端 |
| `W/kitty_gfx` | 4 | kitty graphics 响应错误 | 终端协议应答；已按错误处理 |
| `W/login` | 3 | SELinux granted/denied | 外部包探测 `/proc`；无需处理 |
| `W/TerminalSurface` | 2 | Surface 尚未就绪 | 启动期正常 defer；有重试 |
| `W/FeatureOpt` | 2 | 系统特性查询 | 平台侧；无应用栈 |
| `W/DefaultDispatch` | 2 | SELinux granted | 外部包；无需处理 |
| `W/InputEventReceiver` | 2 | 输入事件已释放 | 平台侧；无应用栈 |
| `W/proot` | 2 | SELinux granted | 外部包；无需处理 |
| `E/com.termux` | 1 | 调试器无法加载 | 系统启动信息；非缺陷 |
| `E/Typeface` | 1 | 系统字体回退 | 平台侧；无应用栈 |
| `W/eza` | 1 | SELinux granted | 外部二进制；无需处理 |
| `E/native::terminal::session` | 1 | 退出码误判 execve | **已修复** |

`mali_gralloc` 的 24 条是 3 个 8 条突发（18:35:30.129、18:35:40.323、18:36:01.917），
都落在 surface 创建/重建时刻；消息固定为：

```text
Requested R8 format is not supported with this allocator. R8 format is only supported with the AIDL allocator
ERROR: Unrecognized and/or unsupported format 0x38 and usage 0xb00
```

这是 Mali gralloc 对 R8 格式的拒绝。日志同一时刻的 `SurfaceConfiguration` 已是
`Rgba8Unorm`，说明拒绝发生在 `attachWindow` 内部、交换链格式选定之前；可能是驱动/平台
探测，也可能来自旧构建。当前代码的 `select_surface_format` 已显式优先 `Rgba8Unorm`，
并在新增的 `attach_surface: configured ... format=... present=...` 日志中报告实际选择；
下一次真机日志若仍出现 R8 且 `format=Rgba8Unorm`，则确认是驱动内部探测，应用侧无开关。

## 应用自身问题与修复

### 1. `exit 254` 被误判为 `execve errno=154`

原文：

```text
2026-10-22 18:37:14.152 E/native::terminal::session
wait thread: child exited with code 254 (possible execve errno=154; see PTY output for the authoritative marker)
```

上下文：用户执行 `comma exit 254`。等待线程把 `code >= 100` 一律当作
`100 + errno`，254 被解成 154。Linux/Android 的 errno 上限是 133，故 254 不可能是
execve 编码。

修复：`execve_errno_from_exit_code` 只接受 `1..=133` 的 errno；子进程侧也把 errno
封顶到 133（退出码 ≤233），与 254/255 区分。`session.rs` 单测
`execve_errno_only_decodes_real_errno_range` 覆盖 102/113/233 与 254/255/100。

### 2. 收尾日志把正常退化写成「进程组不可用」

原文（每个关闭的会话各 2 条，共 3 个会话）：

```text
W/native::terminal::session: session exit: 进程组不可用，仅直杀子进程 2448
W/native::terminal::session: session exit: 进程组不可用，仅直杀子进程 2755
W/native::terminal::session: session exit: 进程组不可用，仅直杀子进程 2795
```

子进程通常与宿主同组，`exit_signal_target` 按设计退化为直杀子进程；这是安全路径，不是
故障。原日志把「同组」「子进程已回收」「本进程组号取不到」都写成同一句 warn。

修复：按原因分级——同组、已退出记 debug；只有真正取不到组号才记 warn。同时
`request_exit` 以 `exited` 原子交换短路，已回收后不再按旧 PID 发信号。

### 3. 重复拆卸的隐患

原 `request_exit` 每次都无条件发 HUP/CONT/KILL，和 `pty-lifecycle` 的「重复拆卸只发一组
信号」不一致。日志里每个会话恰好 2 条同文案警告，说明关闭路径至少进入了两次。

修复：首次调用置位并继续；后续调用断通道后直接返回；`getpgid` 为 `ESRCH` 时也不再发信号。
这不改变首次关闭的信号顺序与宽限期。

## 非应用侧条目

### SELinux（32 条）

`fish` 17、`prooted-2448-cB` 7、`login` 3、`DefaultDispatch` 2、`proot` 2、`eza` 1。
其中 30 条 `avc: granted`，2 条 `denied`：

- `login` 读取 `/proc/modules`（`proc_modules`）；
- `login` 读取 `/proc/bus/pci/devices`（`proc`）。

两条都是 `untrusted_app_27` 启动期硬件探测，被 SELinux 拒绝后 proot 继续运行；终端、
shell、文件访问均正常。应用没有权限修改自己的 SELinux 域，也不应绕过。

### 平台/驱动（`mali_gralloc`、`Parcel`、`libc`、`SurfaceSyncer`、`Typeface`、`ziparchive`、`FeatureOpt`、`InputEventReceiver`）

- `Parcel: Expecting binder but got null!` 9 条，调用栈全在系统 Binder 线程，应用无栈。
- `libc: Access denied finding property` 7 条，读 `persist.unipnp.debug`、
  `persist.vendor.gpu.fbc`、`vendor.MB.running`，均为系统/厂商属性。
- `SurfaceSyncer: Failed to find sync` 4 条，SurfaceFlinger 在 surface 重建期的同步缺失。
- `Typeface: updateDefaultTypeface` 1 条，系统字体资源路径不可读后的回退。
- `ziparchive: Unable to open ... unipnp/unisoc-framework.dm` 6 条，Unisoc 系统框架缺文件。
- `FeatureOpt: ensureGetRight for getInt, key=zte_foldable` 2 条，折叠屏特性探测。
- `InputEventReceiver: ... already been disposed` 2 条，输入通道销毁竞态。

### 渲染库/终端协议（`wgpu_core`、`stream`、`kitty_gfx`）

- `wgpu_core::instance` 4 条（每次进程启动 2 条）：Mali 驱动缺少
  `DownlevelFlags(SURFACE_VIEW_FORMATS)`，wgpu 提示不能完整实现 WebGPU。当前渲染路径
  不使用该能力，属预期告警。
- `stream: unimplemented CSI action: ESC [ * { } x` 4 条与
  `kitty_gfx: erroneous kitty graphics response: EINVAL: unsupported medium` 4 条成对出现，
  是 `comma`/图片预览发出的 kitty graphics 查询，Ghostty 解析器对不支持的 medium 回
  `EINVAL`；应用已把错误写回客户端，不影响文本终端。

### 启动期 `TerminalSurface`

```text
W/TerminalSurface: applySurfaceResize: surface not valid yet, deferring
```

2 条都发生在首帧前；代码把尺寸存入 `pendingRetryWidth/Height`，`surfaceChanged` 后重试。
日志后紧接 `attach_surface 480x819`，说明重试成功，不是丢 resize。

## 慢帧调查（未闭环）

`W/Runtime` 106 条 = 80 条 `SLOW_FRAME` + 26 条 `SLOW_LOOP`（同一次慢循环会同时命中两者）。

- 时间跨度 18:35:30.199–18:37:34.561，pid 2300 占 79 条，pid 1829 仅 1 条。
- `SLOW_FRAME` 的 `render`：最小 30.48ms、中位 60.10ms、p95 67.75ms、最大 88.78ms。
- `SLOW_LOOP` 各阶段最大值：`gate=14.88ms`、`pre=0.154ms`、`render=88.79ms`、
  `poll=0.88ms`、`tail=0.039ms`；中位 `total=59.79ms`、`render=59.65ms`。
- 80 条中 75 条 `newOutput=false`，5 条为启动/命令输出；61 条 `scrollOffset>0`，
  覆盖 0–35，滚动期间每一步都触发整屏重绘。
- `count=1`，没有同帧重复渲染；慢帧呈「一帧 60ms、下一帧 3ms」的交替节奏。
- 60 余条集中在 18:37:27.6–18:37:34.5：`scrollOffset` 从 0 递增到 35 再回到 0，
  `newOutput=false`，单帧 55–67ms，相邻滚动事件间隔约 63ms，实际约 16fps。
- 每个滚动事件通常对应两帧：一帧约 2ms，紧接一帧约 60ms。新加的
  `scrollRemainderPx` 字段用于确认第二帧是否只是亚像素余量重绘。

结论边界：应用侧 `pre/poll/gate/tail` 合计通常小于 2ms，时间几乎全在原生 `render`
（建实例、提交、`get_current_texture`/`present` 之一）。原日志没有原生分段计时，
无法区分是 GPU 光栅化、acquire 等待还是 present 阻塞。`mali_gralloc` 的 3 次突发都在
surface 创建/重建时刻，与滚动慢帧不同刻，不能直接归因。

为确认打点能区分阶段，在 API 35 x86_64 SwiftShader 模拟器上复现了三帧（数值来自
`SLOW_RENDER_PHASE`）：`acquire=24.417/118.538/223.957ms`、`encode=62.828/62.833/92.220ms`、
`submit=4.815/96.076/158.547ms`、`present=0.592/5.009/0.975ms`。可见 acquire 与 submit
都可能单独成为主因；这组数据不代表 Mali 真机，但证明打点足以在下次真机日志中定段。
同一轮验证的 `attach_surface` 日志为
`configured 1080x2209 format=Rgba8Unorm present=Mailbox latency=3`，确认实际交换链参数可观测。

因此本变更**不**盲改 `PresentMode`、`desired_maximum_frame_latency` 或渲染管线；那会把
一个未定位的设备/合成器问题换成不可回滚的渲染回归。已在 `render_frame_with_plan` 增加
`SLOW_RENDER_PHASE`，总耗时 ≥30ms 时输出：

```text
SLOW_RENDER_PHASE acquire=...ms encode=...ms submit=...ms present=...ms total=...ms partial=... instances=...
```

`acquire` 含 `begin_frame` 内的 uniform 刷新与纹理获取。下一次复现时：

1. 同一份日志应同时出现 `SLOW_FRAME` 与 `SLOW_RENDER_PHASE`，可直接看四段占比；
2. 同时抓 `dumpsys SurfaceFlinger --latency` 与 `gfxinfo`，确认是应用提交慢还是合成器慢；
3. 若 `present` 占大头且 Mailbox 被驱动降级为 FIFO，再评估切换 present mode；
   若 `encode` 占大头，回到 GPU 光栅化与实例上传。

## 复现与取证

真机复现时按下面的顺序抓取，避免把冷启动慢帧混进滚动样本：

1. 启动应用，等 shell 提示符出现并静置 2 秒；
2. 在终端里产生至少两屏历史（例如执行 `seq 1 200`）；
3. 用手指在终端内容区连续上滑/下滑，持续 3–5 秒；
4. 立即执行：

```sh
adb logcat -d -v threadtime > torvox-scroll.log
```

筛选 `SLOW_FRAME`、`SLOW_LOOP`、`SLOW_RENDER_PHASE` 与 `attach_surface: configured`。
判读规则：`acquire` 占大头查交换链/合成器；`encode` 占大头查 GPU 光栅化与实例构建；
`submit` 占大头查驱动队列；`present` 占大头查合成器/驱动呈现。只有四段占比明确后，
才允许调整 `PresentMode` 或 `desired_maximum_frame_latency`。

## 修复后预期

- `E/native::terminal::session` 的 254 误报消失。
- `W/native::terminal::session` 的正常关闭不再出现；只有真正取不到组号才 warn。
- 其余 208 条（平台/驱动/外部包、渲染观测与上游协议提示）保持不变；它们不是本次
  日志误报的应用侧缺陷，其中慢帧已加分段打点等待复现。
