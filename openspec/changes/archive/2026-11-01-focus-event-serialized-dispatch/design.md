# Design

## Context

原生侧 `focus_event` 的形状决定了宿主的调用代价：先在会话锁内做一次 VT 模式查询（有界
等待，上限为 VT 空闲 tick 的四倍），拿到序列后再在**会话锁外**写子 PTY。写入走
`WRITE_DRAIN_TIMEOUT`（5s），且每有部分写入就重置期限，故持锁时间无上界。

宿主侧已有的窗口焦点变化走 `TerminalDispatchers.inputOutput.limitedParallelism(1)`：
多线程池上直接派发会让相邻两次 focus-in/focus-out 并发进入原生，顺序随线程调度漂移。

切会话的两条补发路径此前绕过这条通道，且都发生在 `sessionLock` 内：
`switchSession` 的阶段 3 与 `activateReplacementSession`（后者由 `handleSessionExit`、
`closeDeadSession`、`closeSession` 三处在锁内调用）。

## Goals / Non-Goals

**Goals:**

- 焦点事件只有一个派发入口，且该入口的持锁时间与调用方线程无关。
- 全部焦点事件（窗口变化、切会话补发）共用一条串行通道，顺序即派发顺序。

**Non-Goals:**

- 不改原生侧 focus_event 的查询窗口与写入期限。
- 不改焦点序列本身与「只在子进程启用 1004 时才发送」这条口径。
- 不处理「切会话时旧会话是否该收到 focus-out」——那是焦点语义的产品决定，不在本次
  缺陷的范围内；本次只保证已发出的上报按顺序落地。

## Decisions

**决策一：新增单一派发入口 `dispatchFocusEvent(bridge, focused)`，而非在两处补发点各自
`scope.launch(focusDispatcher)`。**

理由：就地 `launch` 会让「派发必须走串行通道」这条不变式散落在三个调用点，任何一处
漏改即退回旧缺陷。单一入口同时承载异常处理（见决策三）。

备选方案：把三处调用点各自改成 `scope.launch(focusDispatcher) { bridge.focusEvent(...) }`。
改动更小，但异常处理要写三遍，且不变式无处可钉。

**决策二：串行通道用既有 `focusDispatcher`（`limitedParallelism(1)`），不自建队列。**

理由：`limitedParallelism(1)` 保证同时至多一个任务在跑，且任务按派发顺序取出——这正是
焦点事件要的语义。自建队列要重写同样的不变式，且多一处并发原语。

备选方案：给每会话一个独立的焦点通道。焦点是窗口级而非会话级的（同一窗口只有一个活动
会话收到上报），按会话分通道会破坏跨会话的顺序保证。

**决策三：派发体内捕获异常并记日志，不外抛。**

理由：作用域是 `SupervisorJob`，未捕获异常直达默认处理器即进程崩溃；而焦点上报丢失
（原生会话已销毁、native 异常）必须出声——vim 一类应用会停在错误的焦点模式。取消信号
仍按惯例重抛。

**决策四：用 semgrep 规则而非单元测试钉住不变式。**

理由：不变式是「调用点不得就地调用」，要测它就得对源码做结构断言；本仓已有同族做法
（`no-android-in-terminal`、`no-render-in-terminal` 等边界规则），且 `scripts/check.nu`
已经把 semgrep 纳入门禁。单元测试无法在不做反射与源码扫描的前提下覆盖「某处新增了
就地调用」。

备选方案：抽出可注入的 `FocusEventDispatcher` 类并对其写单测。能测「派发不阻塞调用方」
与「顺序保持」，但测不出「有人绕过它就地调用」，反而多一层一次性抽象。

## Risks / Trade-offs

- [切会话后的 focus-in 变成异步，落点比此前晚若干毫秒] → 通道是单并发线程池，延迟在
  毫秒量级；TUI 的 FocusGained 恢复对此不敏感，而顺序保证与锁内阻塞的代价高得多。
- [semgrep 的 Kotlin 模式匹配可能漏掉某些写法（如经局部变量间接调用）] → 规则命中
  `…​.focusEvent(…)` 的单参调用形态，与缺陷的实际形状一致；漏掉的是「刻意绕开」的写法，
  那种写法本身即应被评审拦下。
