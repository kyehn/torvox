# 焦点上报统一经串行通道派发

## Why

切会话补发的 focus-in 此前就地调用 `Bridge.focusEvent`，而该调用在原生侧要经 VT 线程做
1004 模式查询（有界等待），再写子 PTY（排空上限 5s，且每有部分写入就重置期限——持锁时间
无上界）。该就地调用发生在全局会话锁内：一次慢写就把全部会话操作（新建/切换/关闭/退出
处理）冻住同样长，而这条路径恰恰在「活动会话刚换人」时最容易被撞上。

就地调用同时绕过了窗口焦点变化专用的串行派发通道：focus-in 与 focus-out 可能并发进入
原生，先后顺序随线程调度漂移。远端 TUI 的焦点状态就此与真实窗口相反，且没有自愈触发点
（最后一次事件落地即尘埃落定）。

## What Changes

- 新增唯一的焦点派发入口：窗口焦点变化与切会话补发的 focus-in 共用同一条单并发通道，
  全部焦点事件按派发顺序落地（xterm 语义）。
- 切会话（`switchSession`）与接替会话激活（`activateReplacementSession`）两处补发的
  focus-in 改为经该通道派发，MUST NOT 再在会话锁内就地调用。
- 派发体内的异常就地记录而不外抛：焦点上报丢失会让 vim 一类应用停在错误的焦点模式，
  必须出声；而未捕获异常在 `SupervisorJob` 作用域里会直达默认处理器（进程崩溃）。
- 新增 semgrep 门禁 `no-inline-focus-event`：任何就地 `focusEvent` 调用即判红，把
  「焦点事件只能经串行通道派发」这条不变式钉在门禁上。

## 非改动

- 焦点序列本身（`CSI I`/`CSI O`）、只在子进程启用 1004 时才发送、模式查询窗口宽于 VT
  空闲 tick 这三条既有口径均不改。
- 原生侧不改：focus_event 的直写路径、查询超时与排空期限保持原样。

## Capabilities

### New Capabilities

无。

### Modified Capabilities

- `key-input-encoding`：焦点上报的要求补上「统一经串行通道派发」「MUST NOT 在会话锁内
  就地写入」两条约束与对应场景。

## Impact

- 代码：`android/app/src/main/java/terminal/emulator/runtime/TerminalRuntime.kt`
  （新增 `dispatchFocusEvent`，改写 `focusChange`、`switchSession`、
  `activateReplacementSession` 三处调用点）。
- 门禁：`.semgrep/rules.yml` 新增一条 Kotlin 规则。
- 依赖：无新增、无移除。
