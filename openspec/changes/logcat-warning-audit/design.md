# Design

## 背景

日志来自 `logcat_2026-10-22_18-37-49.txt`，覆盖两次应用进程启动（pid 1829 / 2300）与一次
会话重启。按 `W/`、`E/` 前缀和续行归并后共 215 条，逐条清单见同目录
`logcat-audit-appendix.md`，分类与处置见 `logcat-audit.md`。

应用自身可归因的只有两类：

1. `E/native::terminal::session` 的 `possible execve errno=154`，来自 shell 正常执行
   `comma exit 254` 后的退出码。
2. `W/native::terminal::session` 的「进程组不可用」，来自子进程与本进程同组时的正常退化，
   以及子进程已回收后的重复拆卸路径。

## 决策一：execve errno 只解码 1..=133

子进程用 `100 + errno` 编码 `execve` 失败，这是 `pty-lifecycle` 既有契约。原实现把所有
`code >= 100` 都当作候选，导致 `exit 254` 被解成 errno 154。

Linux／Android 的 errno 上界为 133（`EHWPOISON`），故只接受 `100 + 1 ..= 100 + 133`。
子进程侧同时把 errno 封顶到 133（原先封到 155），使编码后的退出码不超过 233，
与 shell 的 254/255 留出明确区分空间。
判定抽成纯函数 `execve_errno_from_exit_code`，单测覆盖 102、113、233 与 254、255、100。

不改成「只有 PTY 出现 `execve failed:` 标记才算」：等待线程拿不到那行 PTY 输出，且在
shell 启动器内部 `exec` 失败的场景，退出码仍是唯一可观测信号。保留范围判断即可既消误报，
又不丢真实失败。

## 决策二：`exited` 先原子交换，再决定是否发信号

`request_exit` 现在先 `exited.swap(true, AcqRel)`：

- 第一次调用拿到 `false`，继续断通道、发信号。
- 第二次调用（含 `destroySession` 重入）拿到 `true`，断通道后直接返回。
- 等待线程已回收子进程时同样已置位，拆卸不会再碰旧 PID。

这同时补上了 `pty-lifecycle` 早已写明、实现却缺失的「重复拆卸只发一组信号」。先断输出通道
再返回的顺序不变，阻塞在 `output_tx.send` 的读线程仍会立即解除。

`getpgid(child)` 返回 `ESRCH` 时也直接返回：即使 `exited` 尚未置位，子进程也已不存在，
继续 `kill` 只会命中可能被复用的 PID。

## 决策三：进程组日志按原因分级

`exit_signal_target` 的退化条件有四种，原日志把四种都写成「进程组不可用」：

| 条件 | 信号目标 | 日志级别 |
| --- | --- | --- |
| 子进程组号取不到（`ESRCH`） | 不发信号 | debug |
| 子进程与本进程同组 | 直杀子进程 | debug |
| 本进程组号取不到 | 直杀子进程 | warn |
| `getpgid` 其他错误 | 直杀子进程 | warn |

正常关闭时子进程通常与宿主同组，因此绝大多数「进程组不可用」其实是预期退化，降为 debug；
只有真正取不到组号才保留 warn。信号目标选择函数与测试不变。

## 非应用侧条目的处置

- `SurfaceSyncer`、`libc`、`Parcel`、`Typeface`、`FeatureOpt`、`InputEventReceiver`：
  平台/系统 UI 的已知噪声，应用侧无 API 可消除。
- `mali_gralloc` 的 R8 拒绝：应用交换链已显式选 `Rgba8Unorm`（同日日志的
  `SurfaceConfiguration` 也是 `Rgba8Unorm`），拒绝发生在 `attachWindow` 内部；新增
  `attach_surface: configured ... format=... present=...` 日志用于确认是否仍出现。
- `wgpu_core::instance`：Mali Vulkan 驱动缺少 `SURFACE_VIEW_FORMATS` 等 downlevel 能力，
  当前渲染路径不使用这些能力。
- SELinux 32 条：30 条 granted，2 条 denied 全在启动期探测 `/proc`，不影响终端功能。
- `fish`、`proot`、`eza`、`login` 的写入失败来自用户安装的外部二进制，不属于本仓。
- `SLOW_FRAME` / `SLOW_LOOP`：80 次慢帧全部是整屏重绘，`pre/poll/gate` 均小于 1ms，
  时间全在原生 `render`；同进程 `mali_gralloc` 报错集中在 surface 重建期，未与慢帧同刻。
  现有证据只能定位到「设备 GPU/合成器在整屏重绘时阻塞」，不足以支持盲改 present mode。
  本变更在 `render_frame_with_plan` 增加 `SLOW_RENDER_PHASE` 分段打点，下一次真机日志
  可直接区分 acquire（含 `begin_frame` 前置准备）、encode、submit、present；拿到分项前
  不调整渲染参数。

## 慢帧打点的取舍

打点只在 `total >= 30ms` 时输出，正常帧只付四次 `Instant::now()` 的纳秒级成本；阈值与
Kotlin 侧 `SLOW_FRAME_LOG_THRESHOLD_MS` 一致，便于和既有 `SLOW_FRAME`／`SLOW_LOOP` 对齐。
`submit` 与 `present` 分开计时：若 `present` 占大头，优先怀疑驱动把 Mailbox 降级为 FIFO；
若 `encode` 占大头，才回到 GPU 光栅化与实例上传。

## 风险

- 若某个平台把 `execve` 失败编码为大于 233 的退出码，将不再被识别；当前子进程实现固定
  `100 + errno`，errno 不超过 133，风险可接受。
- `exited` 短路依赖等待线程在回收后置位；`request_exit` 本身先置位再发信号，若信号发送
  失败不会重试。这是原有行为，且 `terminate_target` 内部已有 HUP→KILL 升级。
