# logcat-hygiene Specification

## Purpose

运行期日志的卫生要求：应用自身的正常路径不产生警告与错误，高频诊断限频，
外部与上游条目逐条归因并留档。日志分级是排障时区分「应用缺陷」与「环境噪声」的
第一道闸门；闸门失效时，一次环境噪声就能淹没真正的缺陷。

## Requirements

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

#### Scenario: 输入法在失活连接上请求光标监视

- **WHEN** 日志出现输入法侧打出的 `requestCursorAnchorInfo on inactive
      InputConnection`
- **THEN** 归因 MUST 指出该条目由输入法侧的
      `RemoteInputConnectionImpl.requestCursorUpdatesInternal`（AOSP
      android14-release）打出，判据是 `ic == null || !isActive()`，而
      `isActive()` 要求 `InputMethodManager.isActive()` 为真且连接尚未
      `closeConnection`
- **AND** 归因 MUST 写明请求由输入法发起，应用侧无对应调用；应用侧的准入判据
      （`admitCursorAnchor`）已与平台同口径取 `InputMethodManager.isActive`，
      MUST NOT 记为应用侧缺陷

#### Scenario: 不得以设备分配器不支持的格式申请缓冲

- **WHEN** 宿主创建交换链
- **THEN** 缓冲格式 MUST 与窗口缓冲格式一致（见 `render-stability` 的同款
      Requirement），设备分配器 MUST NOT 因格式不被支持而记 error

### Requirement: 日志 tag 必须与事件语义一致

日志 tag 取自 `log` 宏的模块路径（`native/src/android/logging.rs` 用 `record.target()`
作 tag），故 tag 反映的是「这段代码落在哪个模块」。导出类日志的调用点 MUST 落在与
事件语义相符的模块：渲染开关不得记成字体事件、主题应用不得记成字体事件、选区变更
不得记成字体事件。

归位 MUST 通过把导出移进语义相符的模块实现，MUST NOT 靠过滤 tag、给日志改级别、
统一前缀或给行内文案加前缀来「对齐」——那类做法只改外观，不改归因。

#### Scenario: 渲染开关的 tag 与语义一致

- **WHEN** 宿主暂停或恢复渲染器
- **THEN** 该行的 tag MUST 落在渲染模块路径下，MUST NOT 落在字体等其他域的路径下

#### Scenario: 外观设置的 tag 与语义一致

- **WHEN** 宿主应用主题或设置光标样式
- **THEN** 该行的 tag MUST 落在外观模块路径下，MUST NOT 落在字体路径下

### Requirement: 日志等级必须可调且两侧同步

日志等级 MUST 可由宿主调整，MUST 覆盖「关闭」。等级 MUST 同时作用于 Kotlin 日志与
原生日志：两边各自过滤，只改一边会出现「原生日志照刷、Kotlin 日志已静默」的半截
状态，排障时反而更难判断。

原生日志的等级 MUST 经 `log::set_max_level` 生效，MUST NOT 只过滤 `logcat` 写入：
上游 VT 的日志经同一门面进同一个 sink，故等级改一次即对上游同样生效。

#### Scenario: 关闭日志

- **WHEN** 用户把日志等级设为「关闭」
- **THEN** Kotlin 与原生两侧 MUST 均不再向 logcat 写入任何级别的日志

#### Scenario: 等级只放行警告及以上

- **WHEN** 用户把日志等级设为「警告及以上」
- **THEN** 错误与警告级日志 MUST 照常写入，信息、调试与详细级 MUST NOT 写入

#### Scenario: 未知等级取值不得退化为最宽松的一级

- **WHEN** 宿主传来一个未知的等级取值
- **THEN** 原生侧 MUST 退回信息级并出声，MUST NOT 退化为「详细」级——
      那会让一次笔误直接变成满屏日志

### Requirement: Release 构建必须剥离调试级日志代码

Release 构建 MUST 剥离调试级日志代码（调用点连同其消息构造一起移除）。
MUST NOT 剥离信息、警告与错误级：它们是排障时真正需要的信息。
Debug 与 Release 的可选等级列表 MUST 不同：Release 里被剥离的档位 MUST NOT 出现在
设置页——列出它只会让用户选中一个不生效的档位。

剥离的适用范围 MUST 如实记录，不得把运行时门控说成编译期剥离。Kotlin 侧的调试级
调用点由 R8 的 `assumenosideeffects` 在 Release 构建中真正移除。原生侧（Rust）的
调试级调用点当前由运行时等级门控跳过——消息构造不执行，但调用点仍在二进制里：
两个 APK 变体共用同一份按 release profile 构建的原生库，编译期剥离需要为它们
分别构建原生库（改 `Cargo.toml` 的 `log` 特性并调整构建流水线）。

#### Scenario: Release 的可选等级不含被剥离的档位

- **WHEN** 用户打开设置页的日志等级分区
- **THEN** Release 构建 MUST 只列出未被剥离的档位（关闭、错误、警告、信息），
      Debug 构建 MUST 列出全部档位

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

#### Scenario: 宿主渲染失败的告警

- **WHEN** 宿主渲染循环持续遇到瞬时渲染错误或渲染异常（含失败与成功交替的形态）
- **THEN** 两类诊断 MUST 各按窗口输出，MUST NOT 逐帧输出；每一行 MUST 带上窗口内累计的次数

#### Scenario: 错误后恢复的告警

- **WHEN** 宿主渲染循环在失败与成功之间反复切换，恢复消息逐帧发生
- **THEN** 恢复消息 MUST 按窗口输出，MUST NOT 逐帧输出；该行 MUST 带上窗口内累计的恢复次数

#### Scenario: 输出泵失败的告警

- **WHEN** 宿主渲染循环持续遇到输出泵失败（pollAll 抛异常）
- **THEN** 告警 MUST 按窗口输出，MUST NOT 逐帧输出；该行 MUST 带上窗口内累计失败的次数

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

### Requirement: 未捕获 GPU 错误必须按来源分级

未捕获 GPU 错误 MUST 按来源分级：只有「平台收走原生窗口后 wgpu 重新配置 surface
必然失败」这一类可记为 debug，其余一律 MUST 记为 error。判别 MUST 依据错误种类
与上游固定文案，MUST NOT 依据「看起来像」或整段描述的模糊匹配。

#### Scenario: 窗口被收回后的配置失败

- **WHEN** 未捕获错误是校验错误，且描述同时含上游的 surface 配置调用点标识与
      「不支持该适配器的队列族」
- **THEN** 该错误记为 debug，MUST NOT 记为 error

#### Scenario: 自家配置错误仍是应用缺陷

- **WHEN** 未捕获错误出自 surface 配置，但失败原因是格式不支持、尺寸非法等自家问题
- **THEN** 该错误 MUST 记为 error

#### Scenario: 非校验错误不按平台事件降级

- **WHEN** 未捕获错误不是校验错误
- **THEN** 该错误 MUST 记为 error

### Requirement: 限频闸门必须以单调时钟判定间隔

诊断限频 MUST 以进程内单调时钟判定「距上次输出过了多久」，MUST NOT 以墙上时钟
判定。墙上时钟会被校准：向前跳会把两次输出的真实间隔压到远小于限频下限
（两次输出挤在一起），向后跳会把间隔算成零而让告警在时钟追上之前长期沉默。
两者都使「两次输出之间的间隔 MUST 不小于 500ms」这条要求失去保证。

同一类诊断 MUST 只有一处限频判定实现，MUST NOT 在不同层各写一份口径不同的闸门；
不同调用点若需要不同窗口，差异 MUST 体现为该闸门的配置，MUST NOT 体现为另一份
私有实现。

#### Scenario: 墙上时钟前跳不压缩输出间隔

- **WHEN** 诊断限频窗口尚未走完，而墙上时钟向前跳了一段
- **THEN** 窗口内的诊断 MUST 继续被压住，MUST NOT 因时钟跳变而提前输出

#### Scenario: 墙上时钟回跳不静默告警

- **WHEN** 墙上时钟回跳，而限频窗口尚未走完
- **THEN** 窗口内的诊断 MUST 继续被压住，且 MUST NOT 在时钟追上之前长期沉默

#### Scenario: 同一类诊断只有一处限频判定

- **WHEN** 原生侧存在多类高频诊断（逐帧分段诊断、帧丢弃告警）
- **THEN** 它们 MUST 共用同一个限频闸门实现，各调用点之间的差异 MUST 仅体现为
      窗口长度，MUST NOT 另起一份私有闸门

### Requirement: 慢循环判定不得计入主动等待

慢循环诊断 MUST 只按**实际工作时长**判定。空闲门控驻留与渲染失败退避都是设计上
主动等的等待，MUST NOT 计入判定；判定所用的时长 MUST 减去这两段等待。

慢循环诊断行 MUST 显式给出退避时长，且各段时长之和 MUST 等于整循环时长：把主动
等待并进「轮询」段既让定位信息失真（真机上 50ms 退避被报成 50ms 轮询，而实际
轮询不足 1ms），也让正常退化路径（平台收走原生窗口）产生 warn 级告警。

#### Scenario: 平台收走窗口不产生慢循环告警

- **WHEN** 平台收走原生窗口导致渲染失败，循环随后按设计退避
- **THEN** 该次循环 MUST NOT 记出慢循环告警

#### Scenario: 真实慢渲染仍告警

- **WHEN** 渲染本身耗时超过慢循环阈值，且该次循环没有主动等待
- **THEN** 该次循环 MUST 记出慢循环告警

#### Scenario: 诊断行各段自洽

- **WHEN** 慢循环告警被记出
- **THEN** 行内各段时长之和 MUST 等于整循环时长，且退避时长 MUST 有独立字段

### Requirement: 分段诊断的各段时长之和必须等于整段时长

渲染侧的分段慢帧诊断 MUST 把整段时长拆成若干**命名段**，各段之和 MUST 等于整段
时长；MUST NOT 有夹在两段之间却无处可归的耗时。

单元构建诊断的各段依次为：实例构建 → 字形图集上传 → 脏带编排 → GPU 提交。图集
上传夹在实例构建与脏带编排之间：新字形首次光栅化后要把图集像素传上 GPU，一次可达
数十毫秒。它 MUST 单列成段，否则这段时间在诊断行里既不属于「构建」也不属于「GPU」，
排障时只剩两个对不上的数字。

#### Scenario: 各段自洽

- **WHEN** 单元构建诊断被记出
- **THEN** 行内各段时长之和 MUST 等于整帧时长

#### Scenario: 图集上传单独成段

- **WHEN** 本帧有新光栅化的字形需要上传图集
- **THEN** 上传耗时 MUST 以独立字段给出，MUST NOT 被并入构建段或 GPU 段

### Requirement: 帧内慢渲染诊断必须分出锁外查询与重取锁

帧内慢渲染诊断（`SLOW_RENDER_INNER`）的渲染段 MUST 再把查询后的链路拆开：`keys`
（持锁期间算放置键与失效判据）→ `query`（锁外的 VT 往返查询）→ `lock2`（查询后
重取渲染状态锁）→ `prepare`（绘制前准备）→ `draw`（绘制与呈现）。首帧路径在
查询前还会先取一次锁，该段由 `lock` 单列；空闲路径既不查询也不重取锁，`keys`、
`query` 与 `lock2` 均为 0。首帧路径在没有锁外工作时（纯文本稳态）同样既不查询
也不重取锁，三段为同一时刻。查询 MUST 单列成段：它是一次 VT 线程往返（忙时可等到
查询超时），与「等锁」是两回事，混在一段里则慢帧既看不出是查询慢也看不出是锁忙。

#### Scenario: 帧内慢渲染诊断的各段自洽

- **WHEN** `SLOW_RENDER_INNER` 被记出
- **THEN** 行内 `keys`、`query`、`lock2`、`prepare`、`draw` 五段之和 MUST 等于
      `render`，`phase1`、`receive`、`lock`、`render` 四段之和 MUST 等于 `total`

### Requirement: 平台与驱动侧条目必须给出可复核的上游事实

日志审计 MUST 把平台与驱动侧条目归到具体的一层，并给出可复核的事实：库源码中的
判据位置，或 Vulkan 扩展名这类可查的能力判据。归因 MUST 说明该条目由哪一层决定，
以及应用侧有无开关；MUST NOT 以「当前路径不使用该能力」或笼统的「平台/驱动」代替
根因。

需要原始日志才能重算的总数 MUST NOT 在原始文件未入库时凭记忆补写；留档 MUST 只写
可复现的计数与结构性倍数关系。

由表面重建触发的条目（驱动格式探测、平台帧时间线同步组查询）MUST 记明条数与重建
次数的倍数关系，并说明应用侧减少重建的手段（平台在后台拆掉窗口 surface，重建不可避免，
只能靠开孔遮罩消除闪烁）——这类条目
的条数不是独立的缺陷，而是重建次数的函数。

厂商框架与厂商系统镜像带来的条目（厂商改写的默认字体、厂商特性开关探测、厂商
dex 元数据缺失、厂商私有属性读取被拒）MUST 指出条目由厂商层决定，并把「应用无对应
调用」写明；内核 AVC 审计行 MUST 指出应用是检查的主体而非成因。

#### Scenario: 渲染能力提示缺失

- **WHEN** 日志出现渲染后端的能力缺失提示（如表面视图格式能力位缺失）
- **THEN** 归因 MUST 给出决定该能力位的驱动扩展判据，MUST 说明应用侧无可用开关

#### Scenario: 设备分配器的格式探测告警

- **WHEN** 日志出现设备图形分配器对格式的探测告警（真机为 `mali_gralloc` 报告 R8
      格式不被支持）
- **THEN** 归因 MUST 给出上游判据位置：AOSP `vulkan/libvulkan/swapchain.cpp` 的
      `GetPhysicalDeviceSurfaceFormatsKHR` 在枚举表面格式时无条件对 R8 调
      `AHardwareBuffer_isSupported`，Mali 的 gralloc 在该探测里逐次打出
      「Requested R8 format…」与「Unrecognized…0x38/usage 0xb00」；探测由表面重建
      触发，每次重建固定 4 对（8 行）
- **THEN** MUST 指出被拒的是平台格式枚举的支持性探测，不是应用申请的呈现缓冲；应用
      实际请求 `Rgba8Unorm`（与窗口缓冲 `RGBA_8888` 同格式），交换链照常建立、帧
      照常呈现，MUST 说明应用侧无开关（除非放弃 Vulkan 表面），MUST NOT 记为应用
      选错格式；MUST 同时记明该组条数与表面重建次数的倍数关系（8 行/次），以及
      应用侧减少重建的手段

#### Scenario: 系统窗口事务的空 Binder 返回

- **WHEN** 日志出现系统窗口加入显示列表或重排布局时的 Binder 空返回告警
- **THEN** 归因 MUST 指出该事务由平台发起、应用代码无对应调用，MUST NOT 记为
      应用侧缺陷

#### Scenario: 平台帧时间线的同步组查询告警

- **WHEN** 日志出现平台帧时间线机制在同步组查询失败时打出的告警（真机为 Android 13
      的 `SurfaceSyncer` 报告 `Failed to find sync for id=N`）
- **THEN** 归因 MUST 给出判据：Android 13 `SurfaceView.handleSyncNoBuffer` 在
      `mSyncIds.add(syncId)` **之前**调用重绘回调，而移除只在完成回调里；完成回调
      同步执行时 syncId 在入账前就随同步组完成被移除，之后 `surfaceSyncStarted`
      遍历 `mSyncIds` 的每个陈旧 id 都经 `SurfaceSyncer.merge` →
      `getAndValidateSyncSet` 查组失败并逐条打 E
- **THEN** 应用侧 MUST 实现 `SurfaceHolder.Callback2`，把 `drawingFinished` 投递到
      下一轮 UI 循环执行，MUST NOT 在 `surfaceRedrawNeededAsync` 内同步完成；MUST NOT
      用反射清 `mSyncIds`、降低日志级别或过滤 tag 掩盖
- **THEN** 归因 MUST 说明 Android 14 重写了该路径并删除 `mSyncIds`，故告警只在 13
      出现；序号按表面重建逐个累加，是同一泄漏的重复暴露，不是独立缺陷

#### Scenario: 厂商改写的默认字体条目

- **WHEN** 日志出现 `Typeface` 更新默认字体的 error（真机为 `updateDefaultTypeface`）
- **THEN** 归因 MUST 指出 AOSP 的 `android.graphics.Typeface` 没有该日志与该更新路径
      （判据：上游源码内无此字样），条目由厂商改写的框架在 ROM 用自己的默认字体覆盖
      系统默认字体时打出；MUST 说明应用无对应调用、终端字体由原生渲染器另行装载，
      应用侧无开关

#### Scenario: 调试器未启动条目

- **WHEN** 进程启动时出现「无法加载 JDWP agent 故不启动调试器」的 error
- **THEN** 归因 MUST 说明进程可调试（安装的是 debug 构建）与 ROM 未提供 JDWP agent
      两个事实的组合，MUST 写明该信息经进程 stderr 转入 logcat（tag 为进程名）、
      应用代码无对应调用，MUST 写明改用 release 构建即不再出现

#### Scenario: 厂商特性开关探测条目

- **WHEN** 日志出现厂商框架的特性开关探测告警（真机为 `FeatureOpt` 的
      `ensureGetRight`）
- **THEN** 归因 MUST 指出被探测的键名属于厂商框架的可折叠屏特性开关、应用无对应
      调用，MUST 说明应用侧无开关

#### Scenario: 系统 dex 元数据缺失条目

- **WHEN** 日志出现系统镜像内 dex 装载器读不到厂商框架 dex 元数据的告警（真机为
      `ziparchive` 的 `Unable to open`）
- **THEN** 归因 MUST 说明缺失的文件属于系统镜像（厂商框架 jar 未配套 `.dm`）、
      dex 装载器读不到即告警并继续，应用无对应调用，应用侧无开关

#### Scenario: 属性读取被拒条目

- **WHEN** 日志出现 `libc` 打出的属性读取被拒告警（`Access denied finding property`）
- **THEN** 归因 MUST 给出 bionic `SystemProperties::Find` 的判据位置（属性区对应用
      的 SELinux 域不可见时打这条 W），MUST 说明被读的是厂商库在应用进程里读的私有
      属性，MUST 说明应用侧无开关

#### Scenario: 内核 AVC 审计行

- **WHEN** 日志出现 `type=1400 audit(...)` 形式的内核审计行
- **THEN** 归因 MUST 指出该行由内核 `audit` 子系统产生、经 logcat 送达，应用是检查
      的**主体**而非成因，MUST 说明其中 `granted` 记录是放行决定，MUST NOT 记为
      应用侧缺陷

#### Scenario: 平台读到的空 Binder 条目

- **WHEN** 日志出现 `Parcel` 打出的「读到 null binder」告警
- **THEN** 归因 MUST 指出 framework/native 的 `Parcel::readStrongBinder` 在读到 null
      binder 时打这条 W 并返回 `UNEXPECTED_NULL`，调用方在平台一侧，应用无对应
      调用，MUST NOT 记为应用侧缺陷

#### Scenario: 运行时 GC 引用的 finalizer 可达性告警

- **WHEN** 日志出现运行时引用处理打出的「清理的引用仅能从 finalizer 到达」告警
      （真机 tag 为进程名，行内自带 `(only reported once)`）
- **THEN** 归因 MUST 给出 AOSP ART 的判据位置（`runtime/gc/reference_queue.cc` 的
      `ReferenceQueue::ClearWhiteReferences` 形参 `report_cleared`，由
      `reference_processor.cc` 在软引用与弱引用的引用处理路径打开），MUST 说明该告警针对
      「引用所指对象只被 pending finalizer 的 `this` 引用持有」这一语言规范未覆盖的
      角落、每个虚拟机只打一次，MUST 写明应用侧不创建软引用/弱引用/幻引用（仓库内无
      对应 API 调用）、引用由平台与 libcore 内部创建，应用侧无开关

### Requirement: 命令入队失败的告警必须限频并保留次数

命令通道有界，VT 线程繁忙时命令入队会失败。该状况属**一种**异常（通道满），
MUST 只按窗口输出一次，MUST NOT 按失败次数逐条输出：一次输出高峰即可刷出两万余行
同文案告警，把其余诊断淹没。

告警行 MUST 带上窗口内累计的丢弃次数：限频吞掉的是条数，不是次数，次数丢失后
排障时看不出「多久一次、一次多少」。

同一类告警 MUST 只有一处限频判定实现：全部命令入队调用点 MUST 共用同一把闸门，
MUST NOT 各自另写一份口径不同的判定。闸门 MUST 可被多个线程共享（命令入队来自
渲染线程与输入线程）。

命令入队失败本身 MUST 仍记为 warn：被丢弃的命令不会重放，这是真实异常。

实现：`GhosttyTerminal::cmd_drop_log`（`native/src/slow_log.rs` 的
`CountedLogThrottle`）为唯一闸门，`public_api.rs` 的
`warn_command_dropped` 为唯一告警出口，12 处命令入队调用点全部经此。

#### Scenario: 输出高峰不再刷屏

- **WHEN** 命令通道连续多次满，命令入队反复失败
- **THEN** 告警 MUST 按 500ms 窗口至多输出一行，MUST NOT 每失败一次输出一行

#### Scenario: 限频不得吞掉丢弃次数

- **WHEN** 一个限频窗口内发生了多次命令入队失败
- **THEN** 放行时输出的那一行 MUST 带上该窗口内累计的失败次数

#### Scenario: 全部入队调用点共用一把闸门

- **WHEN** 不同的命令入队调用点（写输出、改尺寸、滚动、选区等）先后失败
- **THEN** 它们 MUST 记入同一把闸门，MUST NOT 各自另起一份限频判定

#### Scenario: 入队失败仍是告警而非静默

- **WHEN** 命令入队失败
- **THEN** 该次失败 MUST 记为 warn，MUST NOT 因限频而完全静默

#### Scenario: 共享闸门不得丢计数

- **WHEN** 多个线程同时向同一把闸门记事件
- **THEN** 已交出的计数与窗口内待交出的计数之和 MUST 等于事件总数

#### Scenario: PTY 输出拥塞

- **WHEN** VT 命令通道在输出高峰期间持续满载
- **THEN** 应用侧 MUST NOT 出现逐块「命令通道已满」告警：输出收割走背压而非丢弃
      （见 `pty-output-backpressure`）；无重试槽的调用方也 MUST 按窗口累计成一行

### Requirement: 图像传输介质的路径解析基准由上游决定

Kitty 图像协议的「文件」与「临时文件」介质 MUST 由上游按**终端进程自己的工作目录**
解析客户端给出的路径：上游以终端进程的工作目录打开该路径，客户端给出的路径来自它
自己的命令行上下文，两者不在同一目录时文件介质必然打不开。

应用 MUST NOT 为客户端改写路径、MUST NOT 伪造文件存在：客户端在介质失败后按协议
自行退到可用介质，应用侧的介入只会制造第二条假路径。

共享内存介质在不支持 POSIX 共享内存的平台 MUST 由上游拒绝（上游按平台限制回
`UnsupportedMedium`），应用 MUST NOT 以「介质未启用」为由把这类上游限制误记为应用
侧缺陷。

#### Scenario: 相对路径的文件介质在终端进程目录下打不开

- **WHEN** 客户端把图像按文件介质传来，路径是相对于客户端自身工作目录的相对路径
- **THEN** 上游打不开该路径并回「无效数据」，应用 MUST NOT 改写路径或伪造文件存在

#### Scenario: 不支持 POSIX 共享内存的平台拒绝共享内存介质

- **WHEN** 客户端把图像按共享内存介质传来
- **THEN** 上游按平台限制拒绝该介质，应用 MUST NOT 把该拒绝记为应用侧未启用介质

### Requirement: 启动期不得在主线程产生磁盘写告警

应用 MUST NOT 在主线程上创建或改写持久化文件：目录创建 MUST 与首次读写一并
推迟到承担该读写的 IO 线程，MUST NOT 以放宽 StrictMode 线程策略代替推迟写入
（允许线程磁盘读只放宽「读」，不覆盖「写」）。

冷启动 logcat MUST NOT 出现 StrictMode 的 DiskWriteViolation：该告警由平台
打出却指向应用代码，与真实缺陷无法区分，会掩盖真正的应用侧告警。

#### Scenario: 设置存储在主线程构造

- **WHEN** 应用在主线程上构造设置存储单例
- **THEN** MUST NOT 创建持久化目录；目录 MUST 在首次读写时由 IO 线程创建

#### Scenario: 冷启动日志无磁盘写告警

- **WHEN** 应用冷启动并完成首次设置读写
- **THEN** 应用进程的 logcat MUST NOT 出现 StrictMode 的 DiskWriteViolation

### Requirement: 剪贴板应答回写失败的告警必须限频并保留次数

剪贴板应答回写失败是真实异常，MUST 记为 warn；但洪泛削减时每帧最多回写 16 次应答，
逐次记 warn 会把 logcat 刷成洪水。该告警 MUST 按窗口汇总输出：每个窗口至多一行，
该行 MUST 带上窗口内累计的失败次数，窗口内的计数 MUST 延后交出而非丢弃。

会话已结束导致的回写失败是正常退化路径，MUST 记为 debug，MUST NOT 计入失败窗口。

同一类告警 MUST 只有一处限频判定实现：两条回写调用点 MUST 共用同一把闸门。

实现：`native/src/android/ffi/render/clipboard.rs` 的
`CLIPBOARD_WRITE_BACK_FAILURE_LOG`（`native/src/slow_log.rs` 的
`CountedLogThrottle`）为唯一闸门。

#### Scenario: 回写失败不再逐次刷屏

- **WHEN** 剪贴板应答回写在同一窗口内连续失败多次
- **THEN** 告警 MUST 按 500ms 窗口至多输出一行，MUST NOT 每失败一次输出一行

#### Scenario: 限频不得吞掉失败次数

- **WHEN** 一个限频窗口内发生了多次回写失败
- **THEN** 放行时输出的那一行 MUST 带上该窗口内累计的失败次数

#### Scenario: 回写失败仍是告警而非静默

- **WHEN** 剪贴板应答回写失败
- **THEN** 该次失败 MUST 记为 warn，MUST NOT 因限频而完全静默

#### Scenario: 会话已结束不计入失败窗口

- **WHEN** 会话已结束导致应答回写失败
- **THEN** 该次失败记为 debug，MUST NOT 计入限频窗口的失败次数

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

### Requirement: 无状态变化的重复下发不得逐次记日志

同一设置被重复下发且结果与上次相同时，MUST NOT 逐次记日志；MUST 只在结果真正变化时
记录。重复下发是常态（设置应用、字体应用与会话状态发射三条路径都会下发同一值），
逐次记录「未变化」既不带新信息，也会把真正的告警挤出可读范围。

去重 MUST NOT 吞掉异常信号：结果变化、失败与降级 MUST 照常记录。

#### Scenario: 同一字族重复下发只记一次

- **WHEN** 同一字族被连续下发多次，且每次解析出的字面与上次相同
- **THEN** 终端 MUST 只在首次解析时记录，MUST NOT 逐次记录「未变化」

#### Scenario: 结果变化时照常记录

- **WHEN** 下发的字族解析出与上次不同的字面
- **THEN** 终端 MUST 记录该次变化

#### Scenario: 字族找不到仍出声

- **WHEN** 下发的字族不在渲染字体库中
- **THEN** 终端 MUST 记录告警，MUST NOT 因去重而沉默

### Requirement: 系统等宽字族的重复下发不得重复解析

系统等宽字族被重复下发且当前字面已是系统等宽面时，MUST NOT 重新解析系统等宽面，
MUST NOT 重复记录该选择：重扫字体库得到的是同一个面，重复的记录不带新信息。

切到用户字族后 MUST 清除该状态：切回系统等宽字族时重新解析并记录——此时主字体
确实变了，记录是必要的。

#### Scenario: 重复下发系统等宽字族不重复解析

- **WHEN** 当前字面已是系统等宽面，系统等宽字族又被下发
- **THEN** 终端 MUST NOT 重新解析系统等宽面，MUST NOT 重复记录该选择

#### Scenario: 切回系统等宽面时重新解析

- **WHEN** 字族换成用户字族后又换回系统等宽字族
- **THEN** 终端 MUST 重新解析并选中系统等宽面，MUST 记录该次切换

### Requirement: 渲染线程的中断是关闭信号而非渲染错误

关闭路径（会话切换、运行期停止）中断渲染线程时，协程运行时以取消异常收尾，其 cause 是
`InterruptedException`。这类异常 MUST NOT 被记为渲染错误：MUST 按 debug 记，MUST NOT
输出完整栈，MUST NOT 计入渲染失败的连续计数与退避。

判据 MUST 取异常的 cause 而非异常类型：取消异常也可能来自协程作用域被取消，只有 cause
是 `InterruptedException` 时才说明是线程被中断。判据 MUST 抽成可单元测试的纯函数。

实现：`TerminalRuntime.kt` 的 `isRenderThreadInterruption` 为唯一判据，渲染循环的通用异常
分支先过该判据再决定记 error。

#### Scenario: 关闭会话时中断渲染线程

- **WHEN** 关闭路径中断渲染线程，渲染循环收到取消异常
- **THEN** 该次中断 MUST NOT 记为 error，MUST NOT 输出异常栈
- **AND** 渲染循环 MUST 干净退出

#### Scenario: 真正的渲染异常仍记 error

- **WHEN** 渲染循环抛出的异常不是中断引起
- **THEN** 该次异常 MUST 仍按 error 记，并计入渲染失败的连续计数

### Requirement: 宿主未就绪的路径必须按正常退化记

宿主未就绪的路径——平台收走原生窗口、宿主换新窗口、视图尚未附着——都是可自愈的正常退化，
MUST 按 info 或 debug 记，MUST NOT 记为 warn 或 error。连续出现说明恢复迟迟不来，那一档
MUST 留在 warn，MUST NOT 由应用侧自行吞掉。

同一情形在不同调用点 MUST 判成同一等级：原生侧的窗口判死与重挂载、宿主侧的换窗请求与
视图未附着，各有唯一判据且有单元测试钉住。

实现：原生侧判死记 debug（`SURFACE_INVALIDATION_LOG_LEVEL`）、重挂载记 info；宿主侧换窗
请求首次记 info、连续记 warn（`surfaceRecreateIsWarning`），重建信号记 debug；视图未附着
时跳过选择菜单记 debug。

#### Scenario: 平台收走原生窗口

- **WHEN** 平台收走原生窗口，渲染帧连续取不到纹理
- **THEN** 原生侧的窗口判死 MUST 记为 debug，MUST NOT 记为 error
- **AND** 宿主换新窗口的首次请求 MUST 记为 info，MUST NOT 记为 warn

#### Scenario: 新窗口迟迟不来

- **WHEN** 宿主连续请求换新窗口，新窗口仍未交付
- **THEN** 连续的那次请求 MUST 记为 warn
- **AND** 快速重试耗尽 MUST 仍记为 error

#### Scenario: 视图尚未附着

- **WHEN** 选择菜单因视图未附着而跳过
- **THEN** 该次跳过 MUST 记为 debug，MUST NOT 记为 warn

### Requirement: 应用内瞬时提示不得借用平台提示窗口

应用内瞬时提示 MUST 由应用自己的呈现通道承载（Snackbar），MUST NOT 借用平台提示窗口。
平台提示窗口受两条约束：同一 UID 同时只允许一个窗口（连发时框架打出窗口冲突告警），
以及按包的提示频率限额（超额的提示被直接丢弃，用户看不到反馈）。两条约束的告警都由
平台打出，根因却在应用的调用方式。

前台桌面通知同属应用内瞬时提示，MUST 经同一条通道投递；后台桌面通知另走系统通知。

提示 MUST 由单一通道投递，MUST NOT 由各调用点各自构造；MUST NOT 借提示顺带做别的事。

实现：`TransientMessageChannel` 为唯一投递通道（进程内单例，由依赖注入提供），终端页与设置页
各自订阅并以 Snackbar 呈现，桌面通知经同一通道投递。

#### Scenario: 连续切换字族

- **WHEN** 用户在短时间内连续切换多个字族
- **THEN** 每条提示 MUST 由应用内通道承载，平台 MUST NOT 报出提示窗口冲突、提示令牌
      生命周期或提示配额告警

#### Scenario: 没有界面时投递提示

- **WHEN** 应用没有前台界面而投递提示
- **THEN** 提示 MUST 直接丢弃，MUST NOT 在平台侧排队，MUST NOT 触发平台限额告警

#### Scenario: 前台桌面通知经同一条通道

- **WHEN** 应用位于前台且收到终端程序的桌面通知
- **THEN** 通知 MUST 经应用内通道投递，平台 MUST NOT 出现提示窗口条目
