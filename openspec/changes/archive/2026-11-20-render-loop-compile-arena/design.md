# Design

## 判据：ART 编译 arena，而不是行数

`code-health` 明禁按行数拆分文件或方法，故判据取 ART 自己的口径：

- AOSP `runtime/jit/jit.cc` 的 `Jit::AddMemoryUsage(compiler_name, method_name, bytes, ...)`
  在 `bytes > 4 * MB` 时打出 `Compiler allocated <PrettySize(bytes)> to compile <method>`。
- 传入的 `bytes` 来自 `compiler/optimizing/optimizing_compiler.cc` 的
  `ArenaAllocator::BytesUsed()`，即该次编译的工作内存（IR、SSA、内联后的被调方法、
  寄存器分配等），不是 dex 产物的字节数。dex 产物体量由 `JitCodeCache::Reserve` 的
  `JIT allocated <大小> for compiled code of <方法>` 上报，阈值 50KB。
- 因此「该行是否出现」就是可复核的判据：出现即表示待编译方法的 arena 工作内存
  越过了 4MiB。

度量口径固定为：**同一构建、同一台设备、应用自身 tag 的 `Compiler allocated` 行数
为 0**。平台自身的方法（如 `android.view.ViewRootImpl.performTraversals`）不在判据内。

## 拆分边界：按职责，不按行数

渲染循环的一帧有八段职责，每段有自己的判据与失败语义，各自成方法：

| 方法 | 职责 | 单独成方法的理由 |
| --- | --- | --- |
| `awaitRenderWake` | 唤醒门控与闭锁超时 | 有独立的活跃/空闲判据（`shouldUseIdleLatch`） |
| `pushSelection` | 选区快照下发 | 写入可能被推迟，需单独记账 |
| `pushScroll` | 滚动偏移与亚行余量 | 偏移未接受时余量不得同拍下发 |
| `renderFrame` | 渲染一帧并发布结果 | 首帧、内容行、备用屏三种发布各有判据 |
| `handleSurfaceInvalidated` | 表面失效与换窗预算 | 与渲染发布是不同触发源 |
| `recordFrameTiming` | 帧耗时与慢帧告警 | 诊断职责，有独立的节流器 |
| `consumeNewOutput` | 真实 PTY 摄入的消费 | 空闲时钟、首次输出、滚动复位三件事同源于新输出 |
| `handleRenderFailure` | 渲染失败退避 | 失败路径有独立的账本与退避档位；退避时长返回给循环体，由循环体执行 `Thread.sleep`（线程中断可即刻唤醒） |
| `noteRenderRecovery` | 渲染恢复 | 恢复消息同样可能逐帧发生，按窗口汇总 |
| `pumpOutput` | 输出泵送与事件派发 | 退出事件优先于同帧其余事件 |
| `publishDiagnostics` | 标题刷新与帧时长窗口 | 低频诊断，必须移出渲染热路径 |
| `recordLoopTiming` | 循环计时与慢循环判定 | 慢循环按「去掉主动等待的工作时长」判定 |

跨帧存活的可变状态（计时窗口、趋势基线、节流器、失败账本、已下发的视口快照、
本帧各段时刻）收进三个每线程对象：`RenderLoopInstrumentation`、`ViewportSyncState`、
`LoopFrameMarks`。它们每渲染线程一份、逐帧复用，不产生每帧分配。

## 协程包装的处置：保留

渲染循环外层的 `runBlocking` 保留。理由有两条，都可复核：

- 判据不依赖它。拆分后同一判据（应用自身 tag 的 `Compiler allocated` 行数为 0）
  在 debug 与 release 两种构建上都已达成，ART 报出的方法名仍是协程状态机的
  `invokeSuspend`——即这条超限方法本身就是拆分的结果，协程包装只是它的宿主。
- 退避休眠必须能被线程中断立刻唤醒。渲染失败退避用 `Thread.sleep` 实现，
  `runBlocking` 的取消语义与线程中断标志互不替代；去掉协程包装并不会改变这一点，
  却会额外改动关闭路径上的中断分支（`InterruptedException` 与取消异常两条），
  没有实测收益。

因此本次只做职责拆分，不动协程包装：待编译方法的体量由职责边界决定，而不是由
是否套一层状态机决定。

## 行为等价性

逐段搬移原代码，不改判据、不改顺序、不改日志文案：

- 每帧的顺序保持为：门控 → 消费唤醒标志 → 选区 → 滚动 → 渲染 → 表面失效 →
  帧计时 → 新输出消费 → 失败处置 → 输出泵送 → 诊断 → 循环计时。
- 慢循环分解的各段时长仍按同一组边界取值（`gate`／`pre`／`render`／`backoff`／
  `poll`／`tail`），各段之和仍等于整段。
- `entry.frameMarks` 的起止时刻仍在渲染开始与成功帧/异常帧处更新，看门狗的
  「开始 > 完成」判挂起语义不变。
