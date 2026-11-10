# logcat-hygiene Specification

## MODIFIED Requirements

### Requirement: 应用自身的正常路径不得产生警告与错误

应用自有日志 MUST 按「是否代表异常」分级。正常退化路径——子进程已退出、信号目标
退化为直杀子进程、Surface 尚未就绪而延迟处理、平台收走原生窗口、共享内存等介质
在平台不支持时——MUST 记为 debug 或 info，MUST NOT 记为 warn 或 error；只有真正
表示异常的路径（信号投递失败、命令通道失效、载荷解码失败、宿主迟迟换不来新窗口）
才可记为 warn 或 error。正常路径产生 warn 等于取消「应用缺陷」与「环境噪声」的
分辨闸门。

同一情形在不同调用点 MUST 判成同一等级，MUST NOT 一处记 warn、另一处记 debug。

框架侧的条目同样在判据之内：应用 MUST NOT 触发框架的 W 级「Failed during query」
——缺失文档回 null 游标与抛 `FileNotFoundException` 在客户端看到的响应逐位相同。

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

#### Scenario: 标题查询失败

- **WHEN** 宿主查询会话标题而原生侧没有标题数据
- **THEN** 查询按缺省值（空标题）处理，MUST NOT 记为 error

#### Scenario: 文档缺失查询不得触发框架告警

- **WHEN** 客户端查询不存在的文档、目录或搜索根
- **THEN** 提供者回 null 游标，MUST NOT 抛 `FileNotFoundException`，
      框架 MUST NOT 记出 W 级「Failed during query」

#### Scenario: 应用不可见时不得逐帧停启帧回调链

- **WHEN** 应用不可见（切后台）而 vsync 帧回调链处于停链状态
- **THEN** 链 MUST NOT 被逐帧拉起，也 MUST NOT 逐帧记日志；停链判据 MUST 与
      拉起判据同口径

#### Scenario: 输入法连接失活时不得由应用侧触发锚点回报

- **WHEN** 输入法连接已失活（`InputMethodManager.isActive` 为假）
- **THEN** 应用 MUST NOT 主动请求光标锚点回报，MUST NOT 因此产生框架告警；
      平台在连接失活时本就丢弃锚点信息

#### Scenario: 不得以设备分配器不支持的格式申请缓冲

- **WHEN** 宿主创建交换链
- **THEN** 缓冲格式 MUST 与窗口缓冲格式一致（见 `render-stability` 的同款
      Requirement），设备分配器 MUST NOT 因格式不被支持而记 error

### Requirement: 外部与上游条目必须逐条归因并留档

日志审计 MUST 把每条 W/E/F 归到「应用自身」「上游终端」「平台」「外部程序」之一，
并为每一类写明结论：应用自身条目 MUST 有根因与落地位置；上游条目 MUST 记录
终端上游与绑定层两处事实；平台与外部条目 MUST 说明无应用栈、不做绕过。

审计结论 MUST 以聚合形式留档（逐类计数、归因、处置），MUST NOT 把原始日志行、
时间戳、PID、设备标识或下载链接写入仓库。

#### Scenario: 上游未实现的协议动作

- **WHEN** 日志出现上游 VT 解析的「未实现」告警
- **THEN** 审计 MUST 记录该能力在终端上游 master 与绑定层所钉提交中的存在情况，
       并写明本层代答的落地位置（VT 输入预处理）、判定口径（本块完整出现）与
       不自创错误语义的前提

#### Scenario: 平台不支持的协议介质

- **WHEN** 客户端使用的传输介质被上游按平台限制拒绝
- **THEN** 审计 MUST 区分「介质未启用」（应用侧可修）与「平台不支持」（上游限制），
       并写明应用侧已完成的可修项（能力探测由本层按上游错误语义代答）

#### Scenario: 框架契约触发的缺失文档告警

- **WHEN** 文档提供者对不存在的 docId 抛 `FileNotFoundException`，框架在其 catch 块
      打出 W 级告警
- **THEN** 归因 MUST 指出该告警由框架 catch 块打印而非应用自身输出，MUST 写明触发条件
       是客户端回查缺失文档，MUST NOT 记为应用侧缺陷；处置 MUST 走「客户端可见响应不变」
       的口径——框架捕获该异常后回的同样是 null 游标，故缺失文档回 null 游标即消掉该
       告警；MUST NOT 以过滤该日志作为处置

#### Scenario: 上游未实现的矩形区域属性动作

- **WHEN** 日志出现上游 VT 解析器对 VT420 矩形区域属性（DECCARA／DECSACE）的未实现告警
- **THEN** 归因 MUST 记录该序列的协议名与发出方（客户端的屏幕清理序列），MUST 说明应用
       在 VT 输入预处理中丢弃该序列，且不自创协议未定义的应答
