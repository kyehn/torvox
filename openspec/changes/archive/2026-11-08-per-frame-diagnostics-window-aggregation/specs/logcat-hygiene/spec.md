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

### Requirement: 应用自身的正常路径不得产生警告与错误

应用自有日志 MUST 按「是否代表异常」分级。正常退化路径——子进程已退出、信号目标
退化为直杀子进程、Surface 尚未就绪而延迟处理、平台收走原生窗口、共享内存等介质
在平台不支持时——MUST 记为 debug 或 info，MUST NOT 记为 warn 或 error；只有真正
表示异常的路径（信号投递失败、命令通道失效、载荷解码失败、宿主迟迟换不来新窗口）
才可记为 warn 或 error。

正常路径产生 warn 的代价是真实的：日志分级是排障时区分「应用缺陷」与「环境噪声」
的第一道闸门，把它淹没在正常噪声里，等于取消这道闸门。

正常退化的判据 MUST 覆盖「同一情形在不同调用点」的情形：同一状况在不同路径上被
判成同一等级，MUST NOT 一处记 warn、另一处记 debug。

#### Scenario: 会话正常收尾

- **WHEN** 子进程已退出，或信号目标退化为直杀子进程
- **THEN** 收尾日志记为 debug，MUST NOT 出现进程组相关 warn

#### Scenario: execve 失败的判定依据

- **WHEN** 子进程以退出码结束，且该退出码落在「100 + errno」编码区间内
- **THEN** MUST NOT 据此判定 execve 失败；判定 MUST 只依据子进程回报的确切 errno。
      正常收尾（组杀 SIGHUP 后 shell 按 128+信号码惯例退出）MUST NOT 产生
      error 级日志

#### Scenario: Surface 尚未就绪

- **WHEN** 布局阶段先于 `surfaceCreated` 到达，尺寸变化被延迟处理
- **THEN** 该延迟日志记为 debug，MUST NOT 记为 warn

#### Scenario: 平台收走原生窗口后的瞬时渲染错误

- **WHEN** 平台收走原生窗口导致渲染帧取不到纹理，宿主换新窗口重挂载后即恢复
- **THEN** 该次渲染失败记为 debug，宿主侧的瞬时渲染错误汇总首次记为 info；
      仅当连续出现（新窗口迟迟不来）才记为 warn

#### Scenario: 安装后脚本的执行轨迹

- **WHEN** 安装器执行某个包的安装后脚本，并打印实际执行的命令行
- **THEN** 该轨迹行记为 info，MUST NOT 记为 warn；执行失败与重试 MUST 仍记 warn

#### Scenario: 会话已结束后的剪贴板应答回写

- **WHEN** 远端请求读取剪贴板，而会话已退出，或 PTY 已随子进程关闭
- **THEN** 应答回写失败记为 debug，MUST NOT 记为 warn；该判定 MUST 与输入路径
      对同一情形的判定同口径

#### Scenario: 宿主暂停渲染时的剪贴板应答超时

- **WHEN** 宿主暂停渲染（切后台、打开设置页、Surface 销毁），远端请求读取剪贴板
- **THEN** 应答超时记为 debug，MUST NOT 记为 warn 或 error

#### Scenario: 子进程已退出后的 VT 应答写回

- **WHEN** 子进程已退出，待写的 VT 应答再也无人接收
- **THEN** 应答被丢弃并记为 debug，MUST NOT 记为 warn 或 error
