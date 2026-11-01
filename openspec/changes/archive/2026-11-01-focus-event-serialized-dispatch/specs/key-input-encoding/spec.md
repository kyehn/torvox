## ADDED Requirements

### Requirement: 焦点上报 MUST 统一经串行通道派发

全部焦点事件（窗口焦点变化与切会话补发的 focus-in）MUST 经同一条串行通道派发，按派发
顺序落地；MUST NOT 就地写入。就地写入会把 VT 模式查询与 PTY 写入放在调用方线程上：写
入的排空期限每有部分写入就重置，持锁时间无上界，而该调用若发生在会话锁内，一次慢写就
把全部会话操作冻住同样长；就地写入同时让 focus-in 与 focus-out 并发进入原生，先后顺序
随线程调度漂移，远端 TUI 的焦点状态与真实窗口相反且没有自愈触发点。

#### Scenario: 切会话补发的 focus-in 落在串行通道上

- **WHEN** 用户切到另一个会话且窗口当前持有焦点
- **THEN** 新活动会话 MUST 收到一次 focus-in，且该次上报 MUST 经串行通道派发，
      MUST NOT 在会话锁内就地写入

#### Scenario: 焦点事件按派发顺序落地

- **WHEN** 相邻两次焦点变化（获焦/失焦）先后发生
- **THEN** 两次上报 MUST 按派发顺序写入子 PTY，MUST NOT 因线程调度而颠倒
