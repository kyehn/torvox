# Design

## Context

平台回收 SurfaceView 的 Surface 时会废弃它的 BufferQueue；应用侧仍持有旧的
`ANativeWindow`，渲染线程对它取帧只会得到 `ERROR_SURFACE_LOST_KHR`。宿主已有两条
自愈路径：渲染线程随帧上报 `surfaceInvalidated`，以及 `surfaceCreated` 后用新窗口
重建交换链。两条都能收敛，但报错与重建本身都是浪费。

根因是**顺序**：切后台那次暂停与「Surface 销毁」那次暂停被同一个恢复入口一并解除，
而新窗口要到 `surfaceCreated` 才挂上原生。实测两者相差 27ms，那 27ms 里渲染线程
已经在废弃队列上出了一帧。

暂停记账（`RenderPauseLedger`）是唯一真源，五类来源共用一个计数；本次不引入第二本
账，而是把「Surface 销毁」那次持有交给一个专管挂载时序的门闸。

## Goals / Non-Goals

**Goals:**

- 恢复渲染 MUST NOT 早于新窗口挂上原生。
- 门闸的不变量可 JVM 单测覆盖（设备上无法稳定构造那条时序）。
- 不新增日志噪音：暂停期间不得逐帧记「render paused」。

**Non-Goals:**

- 不改原生侧：取帧失败后的自愈链路（`surfaceInvalidated` → 换窗口）保持原样。
- 不改设置页那条暂停的语义：它与本次无关，仍由界面层独立持有。
- 不动 kitty 协议、上游 VT 解析与厂商驱动行为——审计已逐条归因为非应用缺陷。

## Decisions

**决定一：门闸只管「解除时机」，不新增计数。**

`SurfaceAttachGate` 持有 `RenderPauseLedger` 的一个凭证：Surface 销毁时领一次，新窗口
挂上时归还一次。它不自己判计数，计数仍由账本唯一负责——两本账必然分叉。

替代方案：给 `TerminalRuntime` 加一个 `surfaceAwaitingAttach` 布尔，再由各恢复入口
自行判断「能不能恢复」。否决：那把同一个不变量抄成三处判断，任一处漏判即回到本次的
故障形态。

**决定二：解除点放在「窗口已交给原生」之后，而不是「Surface 有效」时。**

`TerminalRuntime.attachSurface` 在 bridge 存在时同步走完 JNI 挂载，此时原生已持有新
窗口；bridge 尚未就绪（会话 spawn 中）时，窗口由 `attachPendingSurface` 挂载，解除点
随之落在那里。两处都要求「挂载调用已返回」，故不会出现「解除早于挂载」。

替代方案：在 `surfaceCreated` 回调里解除。次选——它与实际挂载分离，bridge 缺席时
会提前解除。否决。

**决定三：恢复入口查门闸状态，而不是无条件恢复。**

`TerminalScreen` 的切前台分支与 `TerminalSurface.postDelayedUnpause` 都改为：
门闸仍在等新窗口时，**不**重启渲染线程。这样既不会在废弃队列上取帧，也不会让渲染
线程在暂停态逐帧跑（那会把「render paused」写成逐帧诊断）。

**决定四：删除两个恢复入口。**

`clearLifecycleRenderPause`（切前台）与 `clearSurfaceDestroyedRenderPause`（延迟解除
暂停）在新模型下要么与 `resumeRenderingFromBackground` 等价，要么与门闸冲突。保留
同义入口只会让后来者挑错那个调用。

**决定五：门闸的解除只在主线程发生。**

暂停账本的持有集合是普通可变状态，非线程安全。门闸的销毁侧在主线程（Surface 回调），
但**挂载侧有两条**：视图重建那条走主线程，启动序列那条（会话 spawn 期间窗口由
`attachPendingSurface` 挂载）跑在 IO 调度器上。后者 MUST 先切回主线程再解除——后台
线程归还会与主线程的领用交错，计数会算错：多归一次会提前放开别人还在等的暂停，
少归一次则让渲染永远停在暂停态（终端全黑）。

## Risks / Trade-offs

- **风险：Surface 销毁后新窗口迟迟不来，渲染保持暂停。** 表现是终端停在最后一帧而
  非黑屏——比在废弃队列上取帧（永久黑屏）安全。`surfaceCreated` 到达即自动恢复。
- **风险：切后台时 Surface 被保留（不触发 `surfaceDestroyed`）。** 该路径没有门闸
  持有，恢复照旧立即发生——这正是既有的「Surface 保留时交换链通常仍可用」形态。
- **代价：多一处状态。** 门闸只有「是否在等新窗口」一个布尔加一个凭证，无额外抽象。
- **代价：恢复入口多一次查询。** 纯内存读，无 IPC。
