## ADDED Requirements

### Requirement: 剪贴板应答缺失的诊断必须按窗口汇总并分级

宿主未在期限内应答剪贴板读请求时 MUST 记诊断，且 MUST 按窗口汇总输出：每个窗口
至多一行，该行 MUST 带上窗口内累计的次数，窗口内的计数 MUST 延后交出而非丢弃。

分级 MUST 依据宿主是否仍在渲染：仍在渲染却没应答时 MUST 记为 warn（宿主漏了
这条读请求，是真实异常）；宿主已暂停渲染时 MUST 记为 debug（正常退化，见「应用
自身的正常路径不得产生警告与错误」）。

两种情形 MUST 各用一把闸门，计数不得混在一起：否则恢复渲染后的那次输出会把
正常退化的次数说成异常。

实现：`native/src/android/ffi.rs` 的 `CLIPBOARD_ANSWER_MISSING_LOG` 与
`CLIPBOARD_ANSWER_MISSING_WHILE_PAUSED_LOG`（`native/src/slow_log.rs` 的
`CountedLogThrottle`）为两把闸门；`render_paused_flag` 是渲染暂停标志的原子镜像
（渲染状态锁被整帧 GPU 工作持有，旁路线程不能去抢它）。

#### Scenario: 应答缺失不再逐次刷屏

- **WHEN** 多条剪贴板读请求接连超时，宿主都没在期限内应答
- **THEN** 诊断 MUST 按 500ms 窗口至多输出一行，MUST NOT 每超时一次输出一行

#### Scenario: 限频不得吞掉次数

- **WHEN** 一个限频窗口内发生了多次应答缺失
- **THEN** 放行时输出的那一行 MUST 带上该窗口内累计的次数

#### Scenario: 宿主仍在渲染时的应答缺失

- **WHEN** 宿主仍在渲染，剪贴板读请求却没在期限内得到应答
- **THEN** 该诊断 MUST 记为 warn

#### Scenario: 宿主暂停渲染时的应答缺失

- **WHEN** 宿主已暂停渲染，剪贴板读请求必然无人应答
- **THEN** 该诊断 MUST 记为 debug，MUST NOT 记为 warn 或 error

## MODIFIED Requirements

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
