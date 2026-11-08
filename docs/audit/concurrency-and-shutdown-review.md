# 会话看门狗与渲染线程拆卸的并发审查

本文记录一次针对「会话存活看门狗」与「渲染线程拆卸」两条链路的并发审查：逐处读取
看门狗的判定、闸门、停止与处置路径，以及渲染线程的启动、停止与退出标记路径，
用可复算的扫描核对高风险形状，再把每一处命中逐条判定。审查不修改
`docs/STATUS.md`，不改动受保护文件。

## 审查范围与证据

- 看门狗本体：`android/app/src/main/java/terminal/emulator/monitor/RenderWatchDog.kt`，
  判定与恢复决策：`monitor/SessionLiveness.kt`。
- 处置方：`runtime/TerminalRuntime.kt` 的 `RenderSupervisor`（`startRenderThread`、
  `stopRenderThread`、`stopDeadRenderThreadResources`、`handleSessionHang`、
  `closeDeadSession`）与会话条目 `SessionEntry`。
- 拆卸守卫：`runtime/SessionBridgeCloser.kt` 的 `closeBridgeUnlessRenderThreadAlive`。
- 形状扫描：`semgrep scan --config .semgrep/rules.yml`（54 条规则，1285 个受跟踪文件，
  0 处命中）；`jscpd 4 --min-lines 12 --min-tokens 60` 扫 `native/src` 与
  `android/app/src`（8 处 / 127 行 / 0.15%）；Rust 侧 `cargo clippy --all-targets
  -- --deny warnings`（0 处错误）与 `cargo fmt --check`（通过）。
- 判定依据：现场代码的事实（锁序、写入时机、线程注册关系），不依赖「看起来安全」。

## 发现

### 看门狗回调持闸门锁与会话锁形成 AB-BA 死锁

`RenderWatchDog.watchLoop` 原先在 `synchronized(fireLock)` 内调用挂起处置回调，而
回调要取会话锁（复核身份、领取关闭许可、关闭会话）；看门狗的停止
（`RenderWatchDog.stop`）则在持有会话锁的路径上被调用，它要取闸门锁。两把锁的获取
顺序相反：看门狗线程持闸门锁等会话锁，会话线程持会话锁等闸门锁，整个会话子系统
冻住——正是本能力要治的「终端会话概率卡死」。

处置：闸门缩到只判定「本代是否仍然有效」，回调移到闸门之外调用；锁序固定为
「会话锁 → 闸门锁」。回归钉：`RenderWatchDogTest` 的
`hang callback must not hold the gate lock while taking the session lock`——把闸门锁
重新裹回回调，该用例即由「完成」变「超时」。

### 陈旧看门狗的迟到判定作用在新渲染线程上

代次闸门只能证明「该实例自己尚未被停止」，证明不了「它仍是会话当前的看门狗」。
渲染线程重启会换上全新的看门狗实例，旧实例的迟到判定照样过闸：渲染层会把新渲染
线程标记为死亡，VT 层与输出投递层会关闭一个已经恢复的会话。

处置：挂起回调改为携带触发它的看门狗实例，处置方在会话锁内复核「本实例仍是该会话
当前的看门狗」，复核与状态变更在同一次会话锁获取内完成。回归钉：
`SessionHangVerdictTest` 的两条用例——去掉身份复核后，渲染层用例的
`renderThreadExited` 由假变真，关闭层用例的自动关闭许可由未领取变领取。

### 挂起关闭路径在渲染线程仍存活时销毁原生会话

`closeDeadSession` 直接销毁原生会话，不先停渲染线程。VT 层与输出投递层的挂起恰恰
意味着渲染线程仍卡在原生代码里，它随时可能回到那段代码继续触碰 bridge 与 surface，
此时销毁即 use-after-free。既有守卫 `closeBridgeUnlessRenderThreadAlive` 只看
`renderThreadPossiblyAlive`，而该标志只在停止渲染线程且 join 超时时才置位；挂起关闭
路径从不停渲染线程，标志恒为假，守卫形同虚设。

处置：挂起关闭路径先走停止渲染线程，join 超时即置位标志，守卫随之跳过销毁并记
错误日志；原生会话待进程消亡时回收（宁可泄漏也不在其下销毁）。

### 拆卸路径的中断信号从未送达目标线程

`SessionEntry.wakeRenderThread` 按 `renderThreadRef` 取线程再中断，而两条拆卸路径
（`startRenderThread` 的旧线程 join、`stopRenderThread`）都先把 `renderThreadRef`
置空（新线程随即接管这个引用），那一刻读到的引用恒是空，中断信号从未送达，join
只能白等满超时。该方法的注释明写「必须先通知，再中断」，实现与自述不符。

处置：目标线程改由调用方显式给出，签名使「按引用取线程」在编译期不再可能。回归钉：
`RenderThreadShutdownTest` 的 `waking the render thread delivers the interrupt
signal to it`。

### 旧渲染线程的退出写入覆盖新线程的存活状态

渲染线程退出时无条件写 `renderThreadExited = true`，而换线程的路径先把
`renderThreadRef` 换成新线程再 join 旧线程，旧线程随后的退出写入把新线程误标为
已死，监视器随即把健康的渲染线程杀掉重启。GPU 挂起后的恢复路径恰好就走这条路
（旧线程在 join 超时后才恢复）。

处置：退出标记只在「本线程仍是会话注册的渲染线程」时才置位。回归钉：
`RenderThreadShutdownTest` 的 `a replaced render thread must not mark the new one
exited`。

## 明确不作处置的项

- **`Bridge.close` 捕获 `Throwable`**：销毁会话是清理路径，失败绝不能外逃——调用方
  只捕获 `Exception`，`Error`（含库部分加载时的 `UnsatisfiedLinkError`）会直达全局
  处理器并杀掉进程。该处已记 error 级日志，不是掩盖。
- **Rust 生产代码的 `expect`**：逐处读过，均为「构造即成立」的不变式（字体族计数
  不可能超出 u16 上限、渲染状态首次访问必然初始化完成、硬编码字符串不含 NUL），
  或已在 `Once` 之后只可能成功一次的位置。Rust 侧无 `unwrap()` 出现在生产路径。
- **Rust 的 `unsafe` 块**：逐处核对，每一处都有紧邻的 `SAFETY` 说明其前置条件。
- **GPU 初始化在调用线程上阻塞**：`pollster::block_on` 只出现在首次初始化，而首次
  初始化发生在启动序列的 IO 调度器线程上（算网格时取字体度量），不在主线程。

## 扫描命中逐条判定

### 重复率（`jscpd 4 --min-lines 12 --min-tokens 60`）

当前代码树的重扫结果与
`openspec/changes/archive/2026-11-07-duplicate-fixture-convergence/audit.md` 的
「扫描后」逐项一致：Kotlin 6 处 / 102 行 / 0.25%，markup 1 处 / 12 行 / 1.74%，
Rust 1 处 / 13 行 / 0.03%，合计 8 处 / 127 行 / 0.15%。本次改动未引入新的命中：
新增的三份用例各自断言不同的不变式（闸门锁与回调的锁序、判定来源的身份复核、
拆卸信号与退出标记的归属），命中段不构成真重复。既有八处的保留理由沿用该留档的
逐条判定（import 块、图标资源、钉住不同不变式的并列回归钉）。

### 静态形状（`semgrep --config .semgrep/rules.yml`）

54 条规则扫 1285 个受跟踪文件，0 处命中。

## 复现方法

- 形状扫描：`semgrep scan --error --dataflow-traces --config .semgrep/rules.yml`；
  `npx --yes jscpd@4 --min-lines 12 --min-tokens 60 --ignore
  "**/build/**,**/target/**,**/node_modules/**" native/src android/app/src`。
- 行为验证：`android/` 下运行 `:app:testDebugUnitTest`（含
  `terminal.emulator.monitor.RenderWatchDogTest`、
  `terminal.emulator.runtime.SessionHangVerdictTest`、
  `terminal.emulator.runtime.RenderThreadShutdownTest`）与
  `:app:connectedDebugAndroidTest`。
- 判别力验证：把上述修复逐条还原（回调裹回闸门锁、去掉身份复核、还原按引用取
  线程、还原无条件写退出标记），对应的回归钉即判红。
