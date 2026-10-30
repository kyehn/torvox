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

## ADDED Requirements

### Requirement: 平台与驱动侧条目必须给出可复核的上游事实

日志审计 MUST 把平台与驱动侧条目归到具体的一层，并给出可复核的事实：库源码中的
判据位置，或 Vulkan 扩展名这类可查的能力判据。归因 MUST 说明该条目由哪一层决定，
以及应用侧有无开关；MUST NOT 以「当前路径不使用该能力」或笼统的「平台/驱动」代替
根因。

需要原始日志才能重算的总数 MUST NOT 在原始文件未入库时凭记忆补写；留档 MUST 只写
可复现的计数与结构性倍数关系。

#### Scenario: 渲染能力提示缺失

- **WHEN** 日志出现渲染后端的能力缺失提示（如表面视图格式能力位缺失）
- **THEN** 归因 MUST 给出决定该能力位的驱动扩展判据，MUST 说明应用侧无可用开关

#### Scenario: 设备分配器的格式探测告警

- **WHEN** 日志出现设备图形分配器对格式的探测告警
- **THEN** 归因 MUST 说明探测由表面重建触发、由驱动枚举格式引起，应用侧无开关，
      MUST NOT 记为应用选错格式

#### Scenario: 系统窗口事务的空 Binder 返回

- **WHEN** 日志出现系统窗口加入显示列表或重排布局时的 Binder 空返回告警
- **THEN** 归因 MUST 指出该事务由平台发起、应用代码无对应调用，MUST NOT 记为
      应用侧缺陷
