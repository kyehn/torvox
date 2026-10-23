# Design

## 背景

审计对象是两份用户提供的运行日志，覆盖多次应用进程启动与会话重启；按 `W/`、`E/`
前缀和续行归并后共 753 条。原始日志与分析中间产物不入库，项目内只保留聚合计数、
类别、归因和修复说明，详见 `logcat-audit.md`。

应用自身可归因的只有两类：

1. 等待线程把 shell 的正常退出码 254 解码成不存在的 `execve` errno。
2. 收尾路径把「子进程与宿主同组」的正常退化写成「进程组不可用」，并可能在重复拆卸时
   再次发信号。

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

- 系统同步、属性读取、Binder、字体回退、特性查询与输入通道销毁：平台/系统 UI 的已知噪声，
  调用栈均不在应用线程，应用侧无 API 可消除。
- 厂商 GPU 分配器的 R8 拒绝：应用交换链显式选择 `Rgba8Unorm`，字形图集也是 RGBA8；
  拒绝发生在交换链配置之前，属于驱动/系统合成路径。`attach_surface` 已记录实际
  `format/present/latency`，不为了消日志修改格式或屏蔽驱动输出。
- `wgpu_core::instance`：GPU Vulkan 驱动缺少 `SURFACE_VIEW_FORMATS` 等 downlevel 能力，
  当前渲染路径不使用这些能力。
- SELinux 68 条：绝大多数 granted，少量 denied 全在外部 proot 启动期读取 `/proc`，
  不影响终端功能；应用无权修改自身 SELinux 域。
- 外部二进制（外部 proot 运行器及其子 shell、目录列举工具）的写入失败不属于本仓。
- 帧/循环观测：第一份 80 次慢帧全部是整屏重绘，`pre/poll/gate` 均小于 1ms，时间全在
  原生 `render`；第二份 385 次慢帧中 379 次发生在滚动，单帧中位 59.2ms，只有 1 次带
  GPU 分段输出（`acquire` 32.9ms、编码 0.4ms、提交 0.8ms、呈现 3.1ms），其余耗时在
  GPU 提交之前。新增 `SLOW_RENDER_INNER`（阶段一/接收/渲染锁/GPU）与
  `SLOW_CELL_BUILD`（实例构建/GPU）两级打点，拿到复现后再定段；在此之前不调整
  present mode 或帧延迟。

## 慢帧打点的取舍

打点只在 `total >= 30ms` 时输出，正常帧只付少量 `Instant::now()` 的纳秒级成本；阈值与
Kotlin 侧 `SLOW_FRAME_LOG_THRESHOLD_MS` 一致，便于和既有 `SLOW_FRAME`／`SLOW_LOOP` 对齐。
`submit` 与 `present` 分开计时：若 `present` 占大头，优先怀疑驱动把呈现模式降级；
若 `encode` 占大头，才回到 GPU 光栅化与实例上传；若两者都很小，则由
`SLOW_RENDER_INNER` 与 `SLOW_CELL_BUILD` 定位会话锁、接收、实例构建或前置准备。

## 风险

- 若某个平台把 `execve` 失败编码为大于 233 的退出码，将不再被识别；当前子进程实现固定
  `100 + errno`，errno 不超过 133，风险可接受。
- `exited` 短路依赖等待线程在回收后置位；`request_exit` 本身先置位再发信号，若信号发送
  失败不会重试。这是原有行为，且 `terminate_target` 内部已有 HUP→KILL 升级。
