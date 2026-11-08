## MODIFIED Requirements

### Requirement: 高频诊断日志必须限频

应用为定位性能问题而输出的逐帧诊断（慢帧、慢循环、渲染分段）MUST 限频：
同一会话内两次输出之间的间隔 MUST 不小于 500ms；MUST NOT 以逐帧频率刷屏。
该要求 MUST 同时覆盖宿主侧与原生侧的诊断输出。

诊断的价值在于形状（次数、分布、阶段占比），不在于条数；逐帧输出会把日志通道
变成瓶颈，并把真正的告警挤出可读范围。限频 MUST NOT 吞掉升级信号：
同一状况的累计计数与阈值告警 MUST 照常输出。

逐帧诊断 MUST 以时间窗口汇总输出：每个窗口输出一次，该行 MUST 带上窗口内的
事件计数（帧数、次数），MUST NOT 逐条输出。统计窗口已攒满而距上次输出不足
间隔下限时，该报告 MUST 延后到间隔满足时给出，MUST NOT 直接丢弃。

记录热路径单次取值的诊断（如每次手势移动的滚动偏移）MUST 记为 trace，
MUST NOT 以 debug 逐次输出。同一行给出多个取值不同的字段时，各字段 MUST 以
自身名称标识，MUST NOT 用易与相邻字段混淆的简称。

#### Scenario: 持续慢帧期间的输出密度

- **WHEN** 连续多帧都超过慢帧阈值
- **THEN** 慢帧诊断在 500ms 窗口内最多输出一行

#### Scenario: 慢循环与慢帧各自限频

- **WHEN** 渲染阶段与循环整体同时超阈值
- **THEN** 两类诊断各自按 500ms 窗口限频，MUST NOT 共用一个计数器而互相吞掉

#### Scenario: 原生侧逐帧诊断同样限频

- **WHEN** 原生侧的逐帧诊断（帧内分段、渲染分段、单元构建、采样段）持续命中阈值
- **THEN** 每一类诊断的两次输出间隔 MUST 不小于 500ms

#### Scenario: 剪贴板队列溢出的告警

- **WHEN** 程序以快于消费的速度持续发送 OSC 52 写入，待上报队列反复溢出
- **THEN** 溢出告警 MUST 按 500ms 窗口输出，MUST NOT 逐帧输出；该行 MUST 带上窗口内累计丢弃的条数

#### Scenario: OSC 52 写回诊断的告警

- **WHEN** 程序持续发送超限的 OSC 52 负载，或持续把写回通道塞满
- **THEN** 两类诊断 MUST 各按窗口输出，MUST NOT 逐条输出；每一行 MUST 带上窗口内累计的次数

#### Scenario: 剪贴板读请求洪泛削减的告警

- **WHEN** 远端持续发 OSC 52 读请求，读请求洪泛削减逐条生效
- **THEN** 告警 MUST 按窗口输出，MUST NOT 逐条输出；该行 MUST 带上窗口内累计削减的条数

#### Scenario: 超长选择器的告警

- **WHEN** 程序持续发选择器名超长的 OSC 52 读请求
- **THEN** 告警 MUST 按窗口输出，MUST NOT 逐条输出；该行 MUST 带上窗口内累计的条数

#### Scenario: 滚动指令投递失败的告警

- **WHEN** 滚动期间命令通道持续满，滚动指令逐帧投递失败
- **THEN** 告警 MUST 按窗口输出，MUST NOT 逐帧输出；该行 MUST 带上窗口内累计失败的次数

#### Scenario: 渲染遇到未知会话的告警

- **WHEN** 宿主在会话被销毁后仍逐帧渲染它
- **THEN** 告警 MUST 按窗口输出，MUST NOT 逐帧输出；该行 MUST 带上窗口内累计命中的帧数

#### Scenario: VT 应答写回失败的告警

- **WHEN** 子进程不读 stdin，VT 应答逐帧写回失败
- **THEN** 告警 MUST 按窗口输出，MUST NOT 逐帧输出；该行 MUST 带上窗口内累计失败的次数

#### Scenario: VT 应答队列溢出的告警

- **WHEN** 子进程不读 stdin 且持续查询设备状态，待回写的 VT 应答队列反复溢出
- **THEN** 溢出告警 MUST 按 500ms 窗口输出，MUST NOT 逐帧输出；该行 MUST 带上窗口内累计丢弃的条数

#### Scenario: 事件队列溢出的告警

- **WHEN** 事件队列被洪泛填满，事件持续被淘汰或丢弃
- **THEN** 告警 MUST 按窗口输出，MUST NOT 逐条输出；该行 MUST 带上窗口内累计丢弃的事件数

#### Scenario: 桌面通知队列溢出的告警

- **WHEN** 桌面通知队列被洪泛填满，通知持续被淘汰
- **THEN** 告警 MUST 按窗口输出，MUST NOT 逐条输出；该行 MUST 带上窗口内累计丢弃的通知数

#### Scenario: 首条诊断必须可见

- **WHEN** 某一类诊断在限频窗口之外首次命中
- **THEN** 该条诊断 MUST 输出，MUST NOT 因限频而丢失首条

#### Scenario: 限频不得吞掉升级信号

- **WHEN** 同一状况持续到足以触发阈值告警（如连续跳帧达到上限）
- **THEN** 该阈值告警 MUST 照常输出，MUST NOT 被限频压住

#### Scenario: 逐帧诊断以窗口汇总输出

- **WHEN** 某一类诊断每次事件都命中（如每次呈现一帧）
- **THEN** 该类诊断 MUST 每个时间窗口输出一次，该行 MUST 带上窗口内的事件计数

#### Scenario: 报告延后而不是丢弃

- **WHEN** 统计窗口已攒满而距上次输出不足间隔下限
- **THEN** 该报告 MUST 延后到间隔满足时给出，MUST NOT 直接丢弃

#### Scenario: 热路径取值只留 trace

- **WHEN** 某一诊断记录的是热路径上的单次取值
- **THEN** 该诊断 MUST 记为 trace，MUST NOT 以 debug 逐次输出

#### Scenario: 诊断行的字段不得互相冒充

- **WHEN** 同一行给出多个取值不同的字段（如是否部分帧与脏带数）
- **THEN** 各字段 MUST 以自身名称标识，MUST NOT 用易与相邻字段混淆的简称

#### Scenario: 窗口次数以独立字段给出

- **WHEN** 同一行既有渲染实例数，又有窗口内的事件计数
- **THEN** 两者 MUST 以各自名称的字段给出（窗口次数为 `slow_frames` 或 `slow_loops`），
      MUST NOT 复用同一个字段名
