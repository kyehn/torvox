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

#### Scenario: 死键与无预组合形式的字母相邻

- **WHEN** 物理键盘布局产生死键（如 `´`）后用户按下没有预组合形式的字母
- **THEN** 丢弃重音是设计行为，MUST 记为 debug，MUST NOT 记为 warn；
       `getUnicodeChar` 对无组合结果本就返回裸字母，逐键记为 warn 等于把正常输入当异常

#### Scenario: 快捷方式请求应急会话

- **WHEN** 应用收到 `com.termux.app.failsafe_session=true` 的启动 Intent（见
      `app-shortcuts`）
- **THEN** 该请求 MUST 记为 info，MUST NOT 记为 warn：应急会话是规范明列的入口，
      不是引导损坏的证据

#### Scenario: Activity 重建时旧窗口的销毁回调

- **WHEN** Activity 重建后旧窗口的 `surfaceDestroyed` 晚于新视图的 `surfaceCreated` 到达
- **THEN** 陈旧视图的回调 MUST 被忽略并记为 debug，MUST NOT 记为 warn：
      该时序是重建期的正常现象，且忽略它正是防止全局渲染暂停无人解除的手段

#### Scenario: 会话关闭竞态下到达的输入

- **WHEN** 用户刚关闭会话，输入法仍提交了一次文本，写向已移除或正在关闭的会话
- **THEN** 未写入 MUST 记为 debug，MUST NOT 记为 warn（同「会话已结束后的剪贴板
      应答回写」）；存活会话却丢失 bridge 的另一条分支仍记 error

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
- **AND** 「应用侧无对应调用」MUST 给出源码判据：应用能用的锚点 API 只有
      `InputMethodManager.updateCursorAnchorInfo(View, CursorAnchorInfo)`，其实现
      （AOSP `InputMethodManager.java`）在 `hasServedByInputMethodLocked` 通过后
      直接走 `mCurBindState.mImeSession.updateCursorAnchorInfo(...)` 发给输入法，
      全函数不触碰 `requestCursorUpdatesInternal`；后者只由
      `IRequestCursorUpdates*` 的 Binder 入口进入，即只能由输入法进程发起
- **AND** 触发条件 MUST 写明是**输入焦点离开终端视图**：2026-11-15 日志中每次该告警
      都紧邻 `InputMethodManager: showSoftInput() view=androidx.compose.ui.platform.
      AndroidComposeView`（搜索栏/设置页的 Compose 文本框）或紧随其后的
      `InsetsController: show(ime(), fromIme=true)`，即输入法正从终端的连接切到另一条
      文本框连接；同窗口内偶有 `getSurroundingText on inactive InputConnection`，
      同属「取文类请求打到已被取代的连接」
- **AND** 应用侧 MUST NOT 靠延迟创建连接、跳过 `finishComposingText` 或预先
      `closeConnection` 来消除该行——那会让组字状态失步；该告警是平台在焦点切换窗口内
      的固有行为，随切换结束自行停止
- **AND** 判据 MUST 包含**跨平台对照**：AOSP x86_64 模拟器上点开 Compose 文本框时
      同样出现 `getSurroundingText`／`requestCursorUpdates on inactive
      InputConnection`（主线 AOSP 已把后者的文案从 `requestCursorAnchorInfo` 改成
      `requestCursorUpdates`，同一 `requestCursorUpdatesInternal` 输出），
      故该告警与厂商定制无关

#### Scenario: 不得以设备分配器不支持的格式申请缓冲

- **WHEN** 宿主创建交换链
- **THEN** 缓冲格式 MUST 与窗口缓冲格式一致（见 `render-stability` 的同款
      Requirement），应用 MUST NOT 主动申请单通道或双通道格式
- **AND** 归因 MUST 指明 `Requested R8 format ...` 与
      `Unrecognized ... format 0x38 and usage 0xb00` 由平台的 `libvulkan`
      （`GetPhysicalDeviceSurfaceFormatsKHR` 合成候选列表时对 R8 的支持性探测）打出，
      MUST NOT 记为应用侧格式选择错误，MUST NOT 记为驱动交换链缓冲分配

判据（2026-11-15 07:15 日志，Unisoc/Spreadtrum 设备；`SPRD:` 前缀与
`persist.vendor.gpu.fbc` 读取即该厂商分配的指纹）：

1. **数值指纹**：`0x38` 是 `AHARDWAREBUFFER_FORMAT_R8_UNORM`
   （AOSP `libs/nativewindow/include/android/hardware_buffer.h`）；`0xb00` 是
   `COMPOSER_OVERLAY (0x800) | GPU_SAMPLED_IMAGE (0x100) | GPU_FRAMEBUFFER (0x200)`，
   恰为 `swapchain.cpp` 的 `GetSurfaceFormats` 构造探测描述符时的
   `consumer_usage | GPU_SAMPLED_IMAGE | GPU_FRAMEBUFFER`（`consumer_usage` 取自
   `native_window_get_consumer_usage`，即 SurfaceFlinger 的 `COMPOSER_OVERLAY`）。
   尺寸 `1x1x1` 与这两项合起来唯一对应 `AHardwareBuffer_isSupported` 的这个调用点。
2. **请求方**：调用方是平台的 `libvulkan`，不是驱动 WSI、也不是应用：
   `GetPhysicalDeviceSurfaceFormatsKHR` 合成候选列表时无条件探测 R8（同一函数还探测
   `R5G6B5`、`RGBA16F`、`RGB10A2`，Android 14 起另有 `RGBA10x6`；本机 gralloc 对这些
   格式探测通过故无日志——只有 R8 这一项被拒）。
   应用请求 `Rgba8Unorm`（`VK_FORMAT_R8G8B8A8_UNORM`），窗口几何
   `WINDOW_FORMAT_RGBA_8888`（= 1），生产代码没有任何 `R8Unorm` 请求。
3. **时序**：4 组全部落在 `attachWindow`（07:12:27.469）之后、
   `configured ... format=Rgba8Unorm`（07:12:27.481）之前，且夹在 4 条
   `unregister`（旧交换链拆除）与 4 条 `register`（新交换链图像）之间——正是
   `attach_surface` 里 `self.surface = None` → `create_surface` → `get_capabilities`
   → `configure` 那一段，与 `vkGetPhysicalDeviceSurfaceFormatsKHR` 的调用点吻合。
4. **倍数**：1 组 = 1 次 R8 探测 = 1 次 `AHardwareBuffer_isSupported` 调用，
   每次探测打 2 条 E（`Requested R8 …` 与 `ERROR: Unrecognized …`）。
   一次挂载固定 4 组，与交换链图像数无关；4 = 2 × 2，两层各翻一倍：
   应用侧每次挂载查 2 次表面能力（显式 `get_capabilities` + `configure` 内部的
   能力校验），而每次能力查询在 ash 的 Vulkan 枚举两段式下变成 **2 次**
   `vkGetPhysicalDeviceSurfaceFormatsKHR`（先问个数、再取数据，见 ash `prelude.rs`
   的 `read_into_uninitialized_vector`）；AOSP 的实现两次都重建候选列表、两次都探测
   R8。日志可复算：`Requested R8` 24 条 ÷ `attach_surface: configured` 6 条 = 4 组/次。
5. **非致命**：探测失败只表示「该候选格式不进入可选列表」，交换链按 `Rgba8Unorm`
   建立、颜色正确；紧随其后的 `register:` 4 条是图像分配成功。
6. **不可抑制**：`AHardwareBuffer_isSupported` 的探测列表由平台源码写死；wgpu 不
   暴露该查询，`SurfaceHolder.setFormat` 已固定 `RGBA_8888`，
   `persist.vendor.gpu.fbc` 是厂商帧缓冲压缩开关、应用无权限。故 MUST NOT 通过
   过滤 tag、降级日志或改窗口格式来掩盖。

条目数是**新建原生窗口次数**的函数：同一 `ANativeWindow` 上切会话走
`attach_surface` 的 fast path（日志为 `RECONFIGURE_SWAPCHAIN (fast path, existing
surface)`），一次表面格式查询都不做，一行都不产生；换新窗口才重走完整挂载
（`get_capabilities` + `configure`），每次固定 8 行（4 次表面格式查询）。2026-11-15
日志中 6 次新窗口对应 48 行（`attach_surface: configured` 恰 6 条、`Requested R8`
24 条、`Unrecognized ... format 0x38` 24 条），3 次同窗重挂（`fast path, existing
surface`，`attachWindow ptr` 与上次相同）对应 0 行，与该口径逐条相符。故该条目的治理
目标是**减少不必要的换窗**（见 `render-stability`「终端 Surface 的表面策略跟随平台
默认」），不是压低单次重建的行数。

#### Scenario: 返回回调未启用导致的框架告警

- **WHEN** Android 13+ 的 `android.window.WindowOnBackInvokedDispatcher` 在注册返回回调时
      以 tag `OnBackInvokedCallback` 打出 `OnBackInvokedCallback is not enabled for the
      application`
- **THEN** 应用 MUST 在 `<application>` 声明 `android:enableOnBackInvokedCallback="true"`；
      该属性由平台按 `ApplicationInfo.isOnBackInvokedCallbackEnabled()` 读取，判断路径不含
      targetSdk 条件；属性自 API 33 引入，API 31/32 设备忽略（本应用 minSdk 31），
      lint 的 UnusedAttribute 属误报，MUST NOT 因此移除该属性
- **AND** MUST NOT 通过过滤 `OnBackInvokedCallback` tag、降级日志或移除返回回调来消除；
      AndroidX 的 `OnBackPressedDispatcher` 注册回调是本告警的触发点

#### Scenario: 选区窗口在投递中途拆装导致的输入接收器告警

- **WHEN** 真机（Mali/Android 13，2026-11-14 14:52）日志出现
      `InputEventReceiver: Attempted to finish an input event but the input event receiver
      has already been disposed.`
- **THEN** 该告警 MUST 归因于「在覆盖层自己的 `dispatchTouchEvent(ACTION_UP)` 里
      dismiss 承载本次按压的 `PopupWindow`」：平台在同一事件仍在投递时拆掉该窗口的
      输入接收器，随后的 `finishInputEvent` 便无处可去
- **AND** 重锚与重显 MUST 推到下一帧（`View.post`），MUST NOT 在投递内同步拆装窗口；
      手柄吸附差一帧（约 16ms）不可感知
- **AND** 承载本次按压的覆盖层弹窗自身的延迟 reap（`onStreamEnded`）同样 MUST
      推到下一轮 UI 循环再 `dismiss()`：转发的触摸流结束时若同步拆掉自己的窗口，
      本事件仍在派发，告警与上一条同源。窗口拆除与事件投递 MUST NOT 重叠
- **AND** 同窗口期 `WindowManagerGlobal` 的 `Add to mViews ... PopupDecorView` 多次出现
      属选区会话的既有行为（覆盖层每次选区会话重建一次、菜单窗口复用），
      MUST NOT 记为泄漏；判据是 `mViews.size()` 峰值与回落，不是单次添加

#### Scenario: 平台编组路径上的 `Expecting binder but got null!`

- **WHEN** 日志以 tag `Parcel` 打出 `Expecting binder but got null!`
- **THEN** 该条目 MUST 按**出现位置**分成两类归因，MUST NOT 用一条概括覆盖两者：
      其一在**缓冲队列线程**上，紧邻 `mali_gralloc` 的 `register: ... importpid=-1`
      与 `unregister`（2026-11-14 13:58/14:52 两份日志均如此，2026-11-15 日志的
      07:12:27.648 亦然）
- **AND** 其二在**主线程**上，紧跟 `WindowClient: Add to mViews ...
      PopupWindow$PopupDecorView` 与 `VRI[PopupWindow] performTraversals
      relayoutWindow` 之后（2026-11-15 日志 07:14:15.395 与 07:14:15.429，分别对应
      选区手柄覆盖层与选区菜单两个窗口的 `addView`）
- **AND** 两类都发生在平台把窗口参数/缓冲句柄编组到 Binder 的路径上；应用侧只是
      以已附着的视图为父调用 `PopupWindow.showAtLocation`（窗口令牌非空），
      MUST NOT 记为应用侧缺陷，MUST NOT 因此改动选区窗口的创建方式
- **AND** MUST NOT 通过过滤 `Parcel` tag、降级日志或改动窗口/交换链格式来消除；
      应用侧无对应开关，与 `render-stability` 的 FBC 噪声同类
- **AND** 判据 MUST 包含**跨平台对照**：同一应用与同一选区窗口在 AOSP x86_64
      模拟器（`sdk_gphone64_x86_64`，仪器化用例实跑）上该行出现 0 次，故它是
      OEM 框架在编组路径上的差异，MUST NOT 记为应用或 AOSP 行为

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

#### Scenario: 搜索 needle 的逐键取值只留 trace

- **WHEN** 用户在搜索框逐键输入，触发 `searchSetNeedle`
- **THEN** 该诊断 MUST 记为 trace，MUST NOT 在 info 级按每次击键输出
- **AND** MUST NOT 记录 needle 原文（用户输入不落日志），只记长度与命中数

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
时间戳、PID、设备标识或下载链接写入仓库。下文各审计场景引用的
`logcat_*.txt` 是设备侧采样产物，按该条不入库；结论以场景内的聚合计数与归因为准。

#### Scenario: 上游未实现的协议动作

- **WHEN** 日志出现上游 VT 解析的「未实现」告警
- **THEN** 审计 MUST 记录该能力在终端上游 master 与绑定层所钉提交中的存在情况，
       并写明本层代答的落地位置（VT 输入预处理）、判定口径（CSI 按本块完整出现判定；
       Kitty APC 扣留块尾未结束的前缀，拼到下一块后判定）与不自创错误语义的前提

#### Scenario: Kitty 探测被读边界拆开

- **WHEN** 客户端的能力探测被 PTY 读边界拆成多块，上游为半条探测记
       `failed to open image file` 与 `erroneous kitty graphics response`
- **THEN** 处置 MUST 是扣留块尾未结束的 APC 前缀、与下一块拼接后重新判定
       （上限之内扣留，超限的 direct 大图不外答故直接透传），MUST 有「同一探测分两次
       写入仍只产生本层代答」的端到端用例，MUST NOT 以过滤 tag 或调低日志级别处置

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

#### Scenario: 2026-11-14 两份运行日志的聚合归因

- **WHEN** 审计 `logcat_2026-11-14_14-00-06.txt`（主进程 18495／渲染 18735）与
      `logcat_2026-11-14_14-52-46.txt`（主进程 28278）
- **THEN** 应用进程 W/E 的计数与处置 MUST 为（前一份 106 条，后一份 61 条）：
      - `mali_gralloc` 72 + 32 条：R8 格式探测与格式拒绝各半，按表面重建成组
        （8 条/次），见同文件的分配器场景；
      - `Parcel` 2 + 23 条：系统窗口事务的空 Binder 返回，见同文件场景；
      - `InputEventReceiver` 5 条（仅后一份）：覆盖层弹窗在事件投递途中被拆除，
        已修复，见同文件场景；
      - `ziparchive` 8、`libc` 8、`FeatureOpt` 4、`Typeface` 2、
        `wgpu_core::instance` 4、`com.termux` 2（debug 构建 jdwp 未启动）、
        `com.termux` 1（ART GC 直方图跳窗）：厂商框架与平台路径，均无应用栈；
      - 慢帧 4 条（仅前一份：`SLOW_CELL_BUILD`／`SLOW_RENDER_INNER`／
        `SLOW_FRAME`／`SLOW_LOOP` 各一条）：同一帧的 CJK 冷字形逐字符重映射，
        已由系统字体面共享映射修掉（见 `cjk-rendering`）；两份日志都没有
        `FONT_LOAD: shared` 行，即运行的是修复前的库
- **AND** 同 UID 子进程的 72 + 33 条 MUST 全部归为 `avc: granted { execute }`
      审计行（见同文件的内核 AVC 场景）；唯一的 `F` 是 2026-11-12 另一进程
      （PID 12237）的历史 `Device::poll` 崩溃，已被 `poll_device_wait` 接住
- **AND** 平台/框架条目 MUST NOT 以过滤 tag 或降级日志处置

#### Scenario: 2026-11-15 运行日志的聚合归因

- **WHEN** 审计 `logcat_2026-11-15_07-15-08.txt`（主进程 29154，运行约 2.5 分钟）
- **THEN** 应用进程 W/E 的计数与处置 MUST 为（合计 75 条）：
      - `mali_gralloc` 48 条：6 次新窗口 × 8 条（R8 探测与格式拒绝各半），
        3 次同窗重挂（fast path）零条，见同文件的分配器场景；
      - `RemoteInputConnectionImpl` 10 条（`requestCursorAnchorInfo` 9、
        `getSurroundingText` 1）：焦点从终端切到 Compose 文本框时输入法仍在轮询
        已被取代的连接，见同文件的光标监视场景；
      - `ziparchive` 4、`libc` 4、`FeatureOpt` 2：厂商框架的镜像 dex 元数据探测、
        属性读取被拒与特性开关探测，均为各自场景所述的无应用栈路径；其中
        `ziparchive` 的 `base.dm` 探测在 AOSP 模拟器上对本应用的调试包同样出现，
        即平台 dexopt 元数据查找，MUST NOT 记为应用打包缺失元数据；
      - `Parcel` 3：1 条在缓冲队列线程、2 条在主线程的选区窗口 `addView` 之后，
        见同文件的编组路径场景；
      - `wgpu_core::instance` 2：适配器缺 `SURFACE_VIEW_FORMATS` 时上游
        `Adapter::request_device` 无条件打出的 downlevel 提示（`instance.rs` 中
        判据为 `!caps.downlevel.is_webgpu_compliant()`），应用侧无 API 可关；
      - `Typeface` 1、`com.termux` 1（debug 构建 jdwp 未启动）：见同文件的
        厂商标字体与调试器场景
- **AND** 同 UID 子进程的 75 条 MUST 全部归为内核审计与历史崩溃：74 条是
      `type=1400 audit(...)` 行（`granted` 66、`denied` 8，全部为用户 shell 里的
      `fish`／`coreutils`／`file`／`comma`／`kitten`／`which` 读取 `/proc` 的结果；此处 `fish` 是审计观察到的进程名，不是应用代码使用的 shell），<!-- nosemgrep: semgrep.no-prohibited-shells -->
      唯一的 `F` 是 2026-11-12 另一进程（PID 12237）的历史 `Device::poll` 崩溃
      ——该日志早于本次运行三天，已被 `poll_device_wait` 的 panic 捕获接住
- **AND** 该审计当时唯一的新增可修项是渲染循环的待编译方法体量：ART 把整条循环
      编成一个方法（`RenderSupervisor$startRenderThread$renderThread$1$1.invokeSuspend`），
      其编译期 arena 工作内存达 4.9MB／5.8MB（`I/com.termux: Compiler allocated ...`，
      2.5 分钟内 4 次）。该数字是**编译期 arena 工作内存**而非编译产物的字节数
      （判据为 AOSP `runtime/jit/jit.cc` 的 `Jit::AddMemoryUsage` 与
      `compiler/optimizing/optimizing_compiler.cc` 的 `ArenaAllocator::BytesUsed`，
      阈值 4MiB）。当时的处置只把循环体内的退出回收、慢循环诊断与计时窗口三块抽成
      独立方法（`RenderSupervisor.reapExitedSessions`／`logSlowLoop`／`logLoopTiming`），
      方法体量并未降到阈值以下——2026-11-20 的日志里同一方法仍以 4232KB／4972KB
      出现 6 次，故当时的处置并未完成，现由同文件的「渲染循环的待编译方法必须保持
      小体量」要求接管。判据是编译 arena 的体量而非行数阈值（`code-health` 明禁按
      行数拆分）；该行是 info 级、不属应用侧错误，MUST NOT 以过滤它作为处置
- **AND** 平台/框架条目 MUST NOT 以过滤 tag 或降级日志处置

#### Scenario: 2026-11-15 第二份运行日志（13:25-24）的聚合归因

- **WHEN** 审计 `logcat_2026-11-15_13-25-24.txt`（主进程 25222，13:23:41 冷启动到
      13:25:01 会话退出，含两次真实 HOME 往返）
- **THEN** 应用进程 W/E 合计 MUST 为 104 条，全部无应用栈，逐条处置 MUST 为：
       - `mali_gralloc` 80 条（`Requested R8 format is not supported with this allocator`
         40 + `ERROR: Unrecognized and/or unsupported format 0x38 and usage 0xb00` 40）：
         即 10 次新窗口 × 8 条，源头是 AOSP `vulkan/libvulkan/swapchain.cpp` 对
         `VK_FORMAT_R8_UNORM`（`AHARDWAREBUFFER_FORMAT_R8_UNORM` = 0x38）的
         `AHardwareBuffer_isSupported` 探测（该文件第 849-851 行），厂商 HIDL 分配器
         不支持 R8 故回这条错；应用侧无 API 可改，MUST NOT 以换 surface 格式或过滤
         该 tag 处置
       - `ziparchive` 4（`unisoc-framework.dm` 2、`unipnp-framework.dm` 2）、
         `FeatureOpt` 2（`zte_foldable`）、`libc` 4（`vendor.MB.running`、
         `persist.vendor.gpu.fbc`、`persist.unipnp.debug` 2）：厂商框架镜像 dex
         元数据与属性读取探测，无应用栈
       - `Typeface` 1（`updateDefaultTypeface(fontPath: /data/resource-cache/font/...)`）：
         AOSP `graphics/java/android/graphics/Typeface.java` 既无该函数也无任何
         `Log.e`/`Log.w`（全文件只有 TAG 常量），故 MUST 归为厂商标字体框架，
         MUST NOT 记为应用字体路径错误
       - `wgpu_core::instance` 2（`Missing downlevel flags: DownlevelFlags(SURFACE_VIEW_FORMATS)`
         + `DownlevelCapabilities {`）：上游 `wgpu-core 30.0.2` 的 `instance.rs` 第
         924-925 行，判据 `Backends::PRIMARY.contains(...) && !caps.downlevel
         .is_webgpu_compliant()`，对适配器无 `SURFACE_VIEW_FORMATS` 时无条件打出，
         应用侧无 API 可关
       - `Parcel` 1（`Expecting binder but got null!`）：`frameworks/native/libs/binder/
         Parcel.cpp` 第 2330 行的 `ALOGW`，读 null binder 时打出，调用方在平台一侧
       - `com.termux` 1（`Not starting debugger since process cannot load the jdwp agent.`）：
         debug 构建未启用 jdwp，无应用栈
       - `AndroidRuntime` 1（`FATAL EXCEPTION: Render-1`，正文为 2026-11-12 另一进程
         PID 12237 的 `Rust panic: Error in Device::poll: Validation Error`）：该行早于
         本次运行三天，属历史崩溃；现由 `poll.rs` 的 `poll_outcome`/`classify` 用
         `catch_unwind` 归一成 `PollOutcome::Failed`（`native/src/render/context/poll.rs`
         含回归用例），MUST NOT 记为本次运行的缺陷
       - 同 UID 子进程 8 条 MUST 全部归为 `avc: granted { execute }` 内核审计行：
         路径全在 `files/usr/nix/store/.../{glibc,jemalloc,oniguruma,gcc,ncurses}`，<!-- nosemgrep: semgrep.no-nix-store-paths -->
         由用户 shell 起的 `bash`/`yazi`/`clear` 装载动态库所致，`granted` 是放行决定，
         MUST NOT 记为应用侧缺陷，也 MUST NOT 以过滤这些 tag 处置
- **AND** 本份日志 MUST NOT 出现任何应用侧 W/E：应用自身 tag（`Runtime`、
      `native::render::*`、`native::android::ffi::*`）全为 info 级；慢帧告警
      （`SLOW_*`）一条也没有，即本次运行未触发渲染诊断
- **AND** 本份日志的两次往返 MUST 与 `render-stability` 的开孔序列对得上：
      `setRenderPaused: paused=true` → `read_cover_frame`（42ms 后）→（约 26s 后台）
      → `attachWindow`（新指针）→ `attach_surface: ANativeWindow changed; rebuilding`
      → `configured ... latency=2` → `warmup: presenting the retained last frame`
      → `paused=false`。即后台期间窗口 surface 确实被平台拆掉、回前台确实重建，
      与「`SURFACE_LIFECYCLE_FOLLOWS_ATTACHMENT` 已证伪」的结论一致
- **AND** 本份日志中的 `read_cover_frame` 只证明「回读出了遮罩帧」，不证明遮罩画进了
      窗口；后者由 `render-stability` 的「已涂满」判据单独取证（见该文件的开孔场景）
- **AND** 平台/框架条目 MUST NOT 以过滤 tag 或降级日志处置

#### Scenario: 2026-11-16 第三份运行日志（09:03-46）的聚合归因

- **WHEN** 审计 `logcat_2026-11-16_09-03-46.txt`（`logcat_2026-11-16_09-03-46.txt`，
      真实设备，主进程 28303，含三次新窗口与两次真实 HOME 往返，
      应用侧 W/E 合计 178 条、零应用栈）
- **THEN** 应用进程（28303）W/E 合计 MUST 为 87 条，逐条处置 MUST 为：
       - `mali_gralloc` 72 条（`Requested R8 format is not supported with this allocator`
         36 + `ERROR: Unrecognized and/or unsupported format 0x38 and usage 0xb00` 36）：
         即 3 次新窗口 × 24 条（每次新窗口 12 次格式探测 × 2 行），源头与
         13:25-24 相同（AOSP `vulkan/libvulkan/swapchain.cpp` 对
         `VK_FORMAT_R8_UNORM` 的 `AHardwareBuffer_isSupported` 探测，厂商 HIDL 分配器
         不支持 R8 故回这条错），应用侧无 API 可改，MUST NOT 以换 surface 格式或过滤
         该 tag 处置
       - `ziparchive` 4（`unisoc-framework.dm`、`unipnp-framework.dm`）、`FeatureOpt` 2
         （`zte_foldable`）、`libc` 4（`vendor.MB.running`、`persist.vendor.gpu.fbc`、
         `persist.unipnp.debug`）：厂商框架镜像与属性读取探测，无应用栈，
         MUST NOT 以过滤 tag 处置
       - `Typeface` 1（`updateDefaultTypeface(fontPath: /data/resource-cache/font/...)`）：
         归为厂商标字体框架（同 13:25-24 的归因），MUST NOT 记为应用字体路径错误
       - `wgpu_core::instance` 2（`Missing downlevel flags: SURFACE_VIEW_FORMATS`
         及其 `DownlevelCapabilities` 清单）：上游对适配器缺该旗标的无条件提示，
         无应用 API 可关，MUST NOT 记为应用缺陷
       - `Parcel` 1（`Expecting binder but got null!`）：`frameworks/native/libs/binder/
         Parcel.cpp` 第 2330 行的 `ALOGW`，调用方在平台一侧
       - `com.termux` 1（`Not starting debugger since process cannot load the jdwp agent.`）：
         debug 构建未启用 jdwp，无应用栈
      另有一个历史进程（PID 12237，与 28303 同 UID 但非本次运行）：`AndroidRuntime` 1
      （`FATAL EXCEPTION: Render-1`，正文为 2026-11-12 的
      `Rust panic: Error in Device::poll: Validation Error`），早于本次运行四天，
      属历史崩溃；现由 `poll.rs` 的 `poll_outcome`/`classify` 归一成
      `PollOutcome::Failed`，MUST NOT 记为本次运行的缺陷
- **AND** 同 UID 子进程（28368/28628/28954/29707）MUST 为 90 条：`DefaultDispatch` 2、
      `fish` 65、`login` 3、`bash` 2、`yazi` 4、`git` 14——其中 `avc: granted`<!-- nosemgrep: semgrep.no-prohibited-shells -->
      83 条（内核审计的放行决定）、`avc: denied` 7 条：`login` 读 `/proc/modules`与
      `/proc/bus/pci/devices` 2 次、`git` 读 `/proc/sys/vm/overcommit_memory` 2 次及
      建立 3 个 pack 临时硬链接。路径全在 `files/usr/nix/store/...`，由用户 nix 用户态<!-- nosemgrep: semgrep.no-nix-store-paths -->
      自行执行（与应用代码无关）；`granted` 不得记为缺陷，`denied` 是 SELinux
      enforcing 的正常裁定且 nix 用户态已照常完成拉取（git 后续对象已入库），
      MUST NOT 以过滤这些 tag 处置
- **AND** 本份日志 MUST NOT 出现任何应用侧 W/E：应用自身 tag（`Runtime`、
      `native::render::*`、`native::android::ffi::*`、`TerminalSurface`）全为 info 级
- **AND** 本份日志的开孔路径 MUST 与 `render-stability` 对得上：两次往返均进入
      「无缓冲 + 有遮罩」并 `warmup: presenting the retained last frame` 恢复
- **AND** `read_cover_frame` 只证明回读出了遮罩帧，不证明遮罩画进了窗口；后者由
      `render-stability` 的「已涂满」判据单独取证（见该文件的开孔场景）
- **AND** 该日志 MUST NOT 被当作闪烁已修的证据：它不携带开孔像素信息

#### Scenario: 2026-11-17 运行日志（08-52-39 至 08-55-46）的聚合归因

- **WHEN** 审计 `logcat_2026-11-17_08-55-53.txt`（真实设备，主进程 17091，运行约
      3 分钟，含十次切后台往返）
- **THEN** 应用进程 W/E 合计 MUST 为 120 条，逐条处置 MUST 为：
      - `mali_gralloc` 88 条（`Requested R8 format is not supported with this
        allocator` 44 + `ERROR: Unrecognized and/or unsupported format 0x38 and
        usage 0xb00` 44）：十一次挂载 × 每次 4 次格式枚举 × 每枚举 2 行。枚举出自
        AOSP `frameworks/native/vulkan/libvulkan/swapchain.cpp` 的
        `GetPhysicalDeviceSurfaceFormatsKHR`：它以 `desc.format =
        AHARDWAREBUFFER_FORMAT_R8_UNORM`（0x38）、`desc.usage = consumer_usage |
        GPU_SAMPLED_IMAGE | GPU_FRAMEBUFFER` 调 `AHardwareBuffer_isSupported`；
        `consumer_usage` 取自 `native_window_get_consumer_usage()`，SurfaceView 上为
        `COMPOSER_OVERLAY`，故 usage 恒为 0xb00。探测对象是 1×1 能力位而非应用的
        呈现缓冲，被拒后交换链仍以 `Rgba8Unorm` 建立，应用侧无 API 可关
      - 慢帧 7 条（`SLOW_CELL_BUILD` 1、`SLOW_RENDER_INNER` 2、`SLOW_FRAME` 2、
        `SLOW_LOOP` 2）：两次冷启动首帧（49.348ms 与 33.007ms）灌满一屏汉字，首屏
        字形在 `warm_frame_glyphs` 里逐字光栅化（`SLOW_CELL_BUILD` 实测
        build=25.257ms，1097 个单元 / 648 个实例），是串行冷字形成本；处置 MUST NOT
        提高阈值或降级日志
      - `wgpu_core::instance` 4 条：两次设备创建各一对 downlevel 提示（`Missing
        downlevel flags: SURFACE_VIEW_FORMATS` 及其能力清单）。该告警在
        `wgpu-core-30.0.2/src/instance.rs` 的 `request_adapter` 里对 PRIMARY 后端的
        非 WebGPU 合规适配器无条件输出；该位在 `wgpu-hal-30.0.2/src/vulkan/adapter.rs`
        由 `VK_KHR_swapchain_mutable_format` 是否可用决定，驱动未暴露该扩展，
        应用侧无 API 可关
      - `ziparchive` 4、`libc` 4、`FeatureOpt` 2、`Typeface` 1、`com.termux` 1
        （debug 构建 jdwp 未启动）、`Parcel` 1：厂商框架与平台路径，均无应用栈
      - `RemoteInputConnectionImpl` 3 条：输入法侧在失活连接上请求光标锚点，
        见同文件的光标监视场景
      - `kitty_gfx` 2 条：用户 shell 里的 kitty 图形辅助程序输出，与应用无关
      - `native::render::context::gpu` 2 条：切后台时 Vulkan 设备丢失记一条 error
        （`Unknown: Device is lost`）、回前台重建后记一条 warning，是本份日志里
        「切后台返回闪烁」的直接成因（见 `render-stability` 的设备丢失场景）
      - `native::android::logging` 1 条（`panic: ... Device::poll: Validation Error`）：
        与上一条同一次设备丢失事件的第二份副本（panic 钩子）。根因已修：丢失回调
        置位后不再轮询死设备（`poll.rs` 的 `lost_device_outcome`），渲染线程的退出
        路径也不再回读（遮罩改为运行期缓存），该 panic 在新实现下不再发生；panic
        钩子仍照常记录未被接住的 panic，MUST NOT 以过滤该 tag 处置
- **AND** 本份日志 MUST 记下「切后台即设备丢失」这条因果链：`setRenderPaused:
      paused=true` 后 7ms 出现 `GPU device lost`，再 53ms 后出现 `Device::poll`
      的 panic；十次 `read_cover_frame: 已回读` 即十次切后台各现场回读一次。
      遮罩若只靠切后台现场回读，这道 panic 之后遮罩为空，回前台首个窗口帧就是
      空开孔——即「切后台返回闪烁」。故遮罩 MUST 在运行期缓存并**先**发布缓存；
      切后台那一刻的强制回读只把已发布的遮罩换成更新的帧，失败不改变已发布内容
      （见 `render-stability` 的运行期缓存场景）
- **AND** 同 UID 子进程的 93 条 MUST 全部归为内核审计与历史崩溃：92 条是
      `type=1400 audit(...)` 行（`fish` 62、`git` 25、`login` 3、`DefaultDispatch` 2，<!-- nosemgrep: semgrep.no-prohibited-shells -->
      由用户 shell 读取 `/proc` 与装载 nix store 动态库所致），唯一的 `F` 是
      2026-11-12 另一进程（PID 12237）的历史 `Device::poll` 崩溃，早于本次运行五天
- **AND** 本份日志 MUST NOT 被当作闪烁已修的证据：它不携带开孔像素信息；修复的
      判据取 `render-stability` 的开孔状态序列与模拟器仪器化用例
- **AND** 平台/框架条目 MUST NOT 以过滤 tag 或降级日志处置

#### Scenario: 2026-11-17 运行日志（20-23-59 至 20-32-32）的聚合归因

- **WHEN** 审计 `logcat_2026-11-17_20-32-40.txt`（真实设备，主进程 27702，运行约
      8.5 分钟，含多次切后台往返与用户 shell 里的 kitty 图形输出）
- **THEN** 应用进程 W/E 合计 MUST 为 150 条，逐条处置 MUST 为：
      - `mali_gralloc` 120 条（`Requested R8 format is not supported with this
        allocator` 60 + `ERROR: Unrecognized and/or unsupported format 0x38 and
        usage 0xb00` 60）：15 次新窗口挂载（日志中 `attach_surface: configured`
        恰 15 条）× 每次 4 次格式枚举 × 每枚举 2 行；同窗口重挂走
        `RECONFIGURE_SWAPCHAIN`（2 条）一次枚举都不做。归因与 08-52-39 一致：
        平台 `GetPhysicalDeviceSurfaceFormatsKHR` 的 R8 能力探测，应用侧无 API 可关
      - `RemoteInputConnectionImpl` 10 条：输入法侧在失活连接上请求光标锚点；每条
        都紧邻 `show(ime(), fromIme=false)` 与 `showSoftInput() view=AndroidComposeView`
        （本份日志 `fromIme=true` 53 条、`fromIme=false` 19 条），即焦点在终端与
        Compose 文本框之间切换，见同文件的光标监视场景
      - `ziparchive` 4、`libc` 4、`FeatureOpt` 2、`Typeface` 1、`com.termux` 1：
        厂商框架与平台路径（Unisoc 镜像 dex 元数据、ZTE 折叠态开关、厂商属性读取、
        debug 构建 jdwp 未启动），均无应用栈
      - `wgpu_core::instance` 2 条：设备创建时上游对缺少 `SURFACE_VIEW_FORMATS`
        的无条件 downlevel 提示（与 08-52-39 同一条上游事实）
      - `kitty_gfx` 2 条（`failed to open image file` + `erroneous kitty graphics
        response`）：这是**同一次真实文件介质传输**的两份日志，不是读边界劈开的
        能力探测。20:24:16.182 同一 VT 线程先由上游 `src/terminal/kitty/
        graphics_image.zig` 的 `cwd().openFile` 打开失败（`FileNotFound`）记第一条，
        再由 `graphics_exec.zig` 把同一失败编成 `EINVAL: invalid data` 应答并记
        第二条；两条是「一条请求 → 一份应答」的因果对。路径按**终端进程工作目录**
        解析，客户端给出的路径来自它自己的工作目录（`comma` 进程在 110ms 前刚被
        执行），两者不同或文件已不在时必然打不开。上游语义即「打开失败回
        `EINVAL: invalid data`、客户端自行退到其它介质」，应用 MUST NOT 改写路径或
        伪造文件存在（见 `kitty-graphics` 的媒体路径场景）；本层只代答 `a=q` 能力
        探测，真实传输仍交上游处理。故这 2 条是上游如实报告客户端请求失败，
        MUST NOT 记为应用侧缺陷，MUST NOT 用 APC 扣留或过滤 tag 处置
      - 慢帧 3 条（`SLOW_RENDER_INNER`／`SLOW_FRAME`／`SLOW_LOOP`）：同一次首帧
        kitty 放置（20:24:16），分段为 `query=10.744ms`（锁外 VT 放置查询）+
        `draw=23.741ms`（首次图集上传与软件光栅），`SLOW_LOOP` 的 `total=47.887ms`
        含 `gate=10.971ms`（vsync 驻留，MUST 从慢循环判定中扣除），实际工作量
        36.916ms 超过 30ms 阈值。属一次性冷路径；处置 MUST NOT 提高阈值或过滤日志，
        30–60ms 瞬时慢帧 MAY 记为 info，持续超预算或 ≥60ms MUST 记为 warn
      - `Parcel` 1 条（`Expecting binder but got null!`）：平台 binder 事务的空返回，
        调用方在平台一侧（与 08-52-39 同一条上游事实）
- **AND** 同 UID 子进程的 136 条 MUST 全部归为内核审计与历史崩溃：135 条是
      `type=1400 audit(...)` 行（`fish` 74、`git` 19、`login` 10、`yazi` 8、<!-- nosemgrep: semgrep.no-prohibited-shells -->
      `DefaultDispatch` 6、`bash` 4、`file` 4、`tokio-rt-worker` 4、`comma` 2、
      `clear` 2、`chromium.thorium` 2，由用户 shell 读取 `/proc` 与装载 nix store
      动态库所致），唯一的 `F` 是 2026-11-12 另一进程（PID 12237）的历史
      `Device::poll` 崩溃，早于本次运行五天
- **AND** `setRenderPaused` 的 19 次 `paused=true` 与 18 次 `paused=false` MUST 按
      配对解释为「日志在末次暂停后结束」而非令牌泄漏：前 18 对逐对吻合，第 19 次
      发生在 `destroySession id=2` 之后 53ms，此后日志仅剩 200ms；暂停由
      `ON_PAUSE` 领用、`ON_RESUME` 与 `onDispose` 归还，三条路径在
      `TerminalScreen` 中都有对应实现
- **AND** 17 次 `attachWindow` MUST 拆成 15 次换窗口（新指针，各触发 4 次格式枚举，
      与 `warmup` 15 条 = 13 条保留帧 + 2 条无保留帧吻合）与 2 次同窗口
      `RECONFIGURE_SWAPCHAIN`（快路径，零枚举）；20:27:44 的
      `warmup: no retained frame to present` 是**会话切换**（挂载的 session 1 与
      表面原属的 session 2 不同，按设计不得把上一会话的画面闪给新会话），不是保留帧
      丢失，与冷启动的首条同因
- **AND** 本份日志 MUST NOT 被当作闪烁已修的证据：它不携带开孔像素信息；修复的
      判据取 `render-stability` 的开孔状态序列与模拟器仪器化用例
- **AND** 平台/框架条目 MUST NOT 以过滤 tag 或降级日志处置

#### Scenario: 2026-11-19 运行日志（15-39-42 至 15-45-29）的聚合归因

- **WHEN** 审计 `logcat_2026-11-19_15-46-03.txt`（真实设备，主进程 16905，运行约
      5 分 47 秒，含一次新建会话、四次快速换窗与多次切后台）
- **THEN** 应用进程 W/E 合计 MUST 为 81 条，逐条处置 MUST 为：
      - `mali_gralloc` 56 条（`Requested R8 format is not supported with this
        allocator` 28 + `ERROR: Unrecognized and/or unsupported format 0x38 and
        usage 0xb00` 28）：七次新窗口配置（1 次冷启动 + 6 次 `ANativeWindow
        changed`）× 每次 4 次格式枚举 × 每枚举 2 行；两次
        `RECONFIGURE_SWAPCHAIN` 同窗口重挂一次枚举都不做。归因与 08-52-39 相同：
        平台 `GetPhysicalDeviceSurfaceFormatsKHR` 的 R8 能力探测，应用实际请求
        `Rgba8Unorm`，应用侧无 API 可关
      - `RemoteInputConnectionImpl` 9 条：输入法侧在失活连接上请求光标锚点，成
        1–2 条一组分布在搜索操作期间的六个时间点（15:39:55、15:39:59、
        15:40:03、15:40:10、15:40:54、15:40:58），判据见同文件的光标监视场景
      - `ziparchive` 4、`libc` 4、`FeatureOpt` 2、`Typeface` 1、`com.termux` 1：
        厂商框架与平台路径（Unisoc dex 元数据、ZTE 折叠态开关、厂商属性读取、
        debug 构建 jdwp 未启动），均无应用栈
      - `wgpu_core::instance` 2 条：设备创建时上游对缺少 `SURFACE_VIEW_FORMATS`
        的无条件 downlevel 提示（与 08-52-39 同一条上游事实）
      - `Parcel` 2 条（15:39:43.107、15:40:45.788）：平台窗口事务的空 Binder 返回
- **AND** 本份日志发现并修复两处应用侧重复工作，二者都属「无状态变化」：
      - `loadFontFile` 2 条（冷启动与新建会话各一次）装载同一
        `~/.termux/font.ttf`：进程级字体库只有一份，第二次装载会追加不可回收的面。
        修复后进程内按路径 + 大小 + mtime 缓存，见同文件的用户字体覆盖需求
      - `log level set to INFO` 5 条 = 启动 1 条 + 无关设置写入触发的 4 条重复下发。
        修复取 `logLevel` 视图的 `distinctUntilChanged`，见同文件的无关设置场景
- **AND** `searchSetNeedle` 3 条（needle 为 `"h"`、`"ho"` 与空串）MUST 归为逐键取值
      诊断：修复前在 info 级记录查询原文，修复后只留 trace 且只记长度与命中数
- **AND** `attachWindow surface attached` 9 条 MUST 拆成 7 次新窗口（1 次冷启动 + 6 次换窗）与
      2 次同窗口 `RECONFIGURE_SWAPCHAIN`；`warmup` 7 条 = 5 条保留帧 + 2 条无保留帧，
      两次无保留帧分别是冷启动与会话切换（15:42:10 挂载 session 2 时上一帧属于
      已销毁的 session 1）
- **AND** `setRenderPaused` 的 14 次 `paused=true` 与 13 次 `paused=false` MUST 按
      配对解释为「日志在末次暂停后结束」而非令牌泄漏：前 13 对逐对吻合，第 14 次
      发生在 `destroySession id=2` 之后 72ms，此后日志仅剩 192ms
- **AND** 同 UID 子进程的 63 条 MUST 全部归为内核审计：58 条 `granted` 是放行记录
      （`fish` 18、`git` 12、`eza` 8、`yazi` 7、`bash` 4、`file` 4、<!-- nosemgrep: semgrep.no-prohibited-shells -->
      `DefaultDispatch` 2、`clear` 2、`login` 1，由用户 shell 读取 `/proc`
      与装载 nix store 动态库所致），5 条 `denied` 是子进程探测 `/proc/modules`、
      `/proc/bus/pci/devices`（`login` 2）与 `/proc/partitions`、已删除的
      `/proc/<pid>/mounts`（`tokio-rt-worker` 3）；应用是检查的主体而非成因
- **AND** 本份日志 MUST NOT 被当作性能缺陷的证据：151 条 `loop timing window` 与
      151 条 `frame timing window` 均为 info 级窗口汇总，无一条 `SLOW_*` 告警；
      最低 fps（≈1fps、≈7fps、≈23fps）出现在 `setRenderPaused` 的主动等待窗口内，
      按慢循环口径 MUST NOT 计为渲染慢
- **AND** 平台/框架条目 MUST NOT 以过滤 tag 或降级日志处置

#### Scenario: 2026-11-19 运行日志（17-44-20 至 17-51-20）的聚合归因

- **WHEN** 审计 `logcat_2026-11-19_17-51-20.txt`（真实设备，主进程 21412，运行约 7 分钟，
      冷启动即遭遇两次 Vulkan 设备丢失风暴，第二次发生在 17:46:03 的滚动重输出期间）
- **THEN** 应用进程 W/E 合计 MUST 为 303 条（修复前），逐条处置 MUST 为：
      - `mali_gralloc` 176 条（`Requested R8 format is not supported...` 88 + `ERROR:
        Unrecognized and/or unsupported format 0x38 and usage 0xb00` 88）：22 次慢路径挂载
        （`attach_surface: configured` 恰 22 条）× 每次 4 次格式枚举 × 每枚举 2 行。其中
        16 次伴随设备重建（`GPU device created` 恰 16 条 = 冷启动 1 次 + 设备丢失后重建
        15 次），另 6 次是同会话换新窗口的重挂（`ANativeWindow changed` 与
        `same-session reattach` 各 6 条），只重建 surface、不重建设备；本份日志无
        `RECONFIGURE_SWAPCHAIN` 快路径记录。归因与 15-46-03 相同，应用侧无 API 可关
      - `wgpu_core::instance` 32 条：16 次设备创建各一对上游 downlevel 提示（与 08-52-39
        同一条上游事实）
      - `native::render::context::gpu` 15 条 `GPU device lost` error（每次设备丢失记一条，
        丢失原因 `Unknown: Device is lost`）+ 15 条 `GPU rebuilt after device loss` warn
        （`attach_surface` 重建设备时提示调用方换新窗口）
      - `native::android::ffi::state` 15 条 `cell metrics query skipped`：修复前为 error，
        修复后记 **debug**
      - `native::android::logging` 15 条（`panic: panicked at .../wgpu_core.rs:1924:30:
        Error in Device::poll: Validation Error`）：每次设备丢失时 wgpu 内部 panic 的副本，
        被 `poll_outcome` 接住；修复后 panic 钩子对预期内接住的 panic 记 **debug**
      - `native::android::ffi::render::frame` 6 条 `render skipped`（改 warn 限频，修复前
        为 error）+ 1 条 `SLOW_RENDER_INNER`（一次首屏重输出 draw=31.8ms）
      - `native::render::pass::frame` 1 条（`SLOW_CELL_BUILD` 22.097ms，997 单元 / 575
        实例，17:51:06 滚动输出路径上的一次首屏冷字形批量）
      - `Runtime` 2 条（`SLOW_FRAME` render=32.177ms、`SLOW_LOOP` total=36.2ms，同批输出
        帧，阈值 30ms 保持不变）
      - `Parcel` 13 条、`libc` 4 条、`ziparchive` 4 条、`FeatureOpt` 2 条、`Typeface` 1、
        `com.termux` 1（jdwp 未加载，debug 构建）、`native::render::context::gpu` 15 warn：
        平台/厂商/debug 路径，无应用栈
- **AND** 本份日志暴露并修复三处应用侧问题：
      - 换视图预算在每次 `surfaceCreated`/`resumeRendering` 都被清零，`attempts` 与
        `lastRequestNanos` 一并归零，使快试 5 次上限与 500ms 最小请求间隔同时失效：
        15 次设备丢失连成 15 次换窗（请求全部记为 `attempt 1/5`），17:44 簇 1.8s 内
        10 次、17:46:03 簇 0.8s 内 5 次，均约 190ms 重建一次 VkInstance。修复后预算只在
        回到前台（`ON_RESUME` / `resumeRenderingFromBackground`）清零，同一前台设备内不再
        复位；快试 5 次后转慢重试（5s），渲染成功由 RenderSupervisor 复位
      - 设备丢失的 panic 副本以 error 重复记 15 次、`cell metrics query skipped` 以
        error 15 次：修复后分别降为 debug / warn（`render skipped`）
      - `docs/STATUS.md` 所记「软件启动从出现背景到 shell 需要较久时间，概率性发生」
        的成因正是本次设备丢失风暴：首帧 17:44:20.989 已呈现，shell 首个输出
        17:44:21.245 已到，但冷启动簇的 10 次设备丢失重建风暴把稳定帧拖到
        17:44:23.084（距首帧约 2.1s，距 shell 输出约 1.8s）
- **AND** 同 UID 子进程的 87 条 MUST 全部归为内核审计（`fish` 57、`git` 21、<!-- nosemgrep: semgrep.no-prohibited-shells -->
      `patch2pr` 5、`coreutils` 4，均为 `type=1400 avc` 的 granted 或 open denied 探测
      记录，应用是检查的主体而非成因）
- **AND** 本份日志 MUST NOT 被当作性能缺陷的证据：134 条 `loop timing window` 与
      134 条 `frame timing window` 均为 info；唯一一次 `SLOW_LOOP` 36ms 出现在首屏
      重输出批量，不构成慢渲染
- **AND** 平台/框架条目 MUST NOT 以过滤 tag 或降级日志处置（`render skipped` 与
      `cell metrics` 两处的降级针对的是「同一丢失事件的余波」，不是平台条目，且都按
      warn/debug 保留全文）

#### Scenario: 2026-11-19 17-51-20 日志中未被应用侧消除项的逐条结论

- **WHEN** 深入核查 `logcat_2026-11-19_17-51-20.txt` 中应用侧无法消除的条目
- **THEN** 每一项 MUST 满足下述判定之一：
      - `Mali-G57` 设备丢失（两簇：冷启动首帧后约 6ms，10 次；17:46:03 滚动重输出前后，
        5 次）：根因未能定位——无内核/驱动日志；Android Studio 设备清单仅 `test_avd` 模拟器
        （SwiftShader，不能复现 Mali `Device lost`），两簇后应用照常出帧（之后数分钟
        `≈60fps` 正常），`PowerPreference=HighPerformance`、`Mailbox`、
        `ANativeWindow_setBuffersGeometry` 在本日志与 15-46-03 日志下表现一致，无证据
        表明具体应用配置为成因。应用侧只能有界恢复（快试 5 次 ≥500ms、慢重试 5s、
        新窗口连续稳定出帧 60 帧约 1 秒后由 RenderSupervisor 复位预算），
        MUST NOT 以降低重试频率或隐藏日志处置
      - `SLOW_CELL_BUILD` build=22.097ms（997 单元 / 575 实例，`partial=true`）：出现在
        滚动重输出后的首屏重输出帧；本机 `cargo bench --bench cell_builder -- --quick`
        热路径为 0.358ms，`bench_end_to_end_cpu_pipeline_latency` 预热后为 3.0ms/帧，
        无算法回归证据；该条目只能是新建首批字形光栅化成本，不能盲改缓存（见
        `partial_frame_bands` 代际门控），需真机 profiler 区分新建字形数与实例发射成本后再定
      - `Parcel Expecting binder but got null!`（13 条，偶发成对）：平台窗口/输入法事务的 binder
        日志，无应用栈、不携带应用堆；15-46-03 日志仅 2 条、17-51-20 为 13 条。该条目
        的触发次数与进行中的事务相关，应用侧无产生/抑制 API；换视图预算复位修复可削减
        重建次数，但无法确证每一条的归属或消除该平台条目本身，MUST NOT 过滤该 tag
      - `mali_gralloc`（R8 探测 `format 0x38`）、`wgpu_core::instance`（缺
        `SURFACE_VIEW_FORMATS`）、`libc` 属性 denial、`ziparchive` dex 元数据、`FeatureOpt`
        折叠态、ZTE `Typeface`、debug jdwp、`com.termux`（jdwp 未加载）：平台/厂商/debug
        路径条目，无应用栈；MUST NOT 记为应用缺陷，MUST NOT 降级、过滤
- **AND** 上述条目与「应用自身 W/E 合计 303」中的可修复部分（字体覆盖重复装载、
      logLevel 重复下发、needle 查询原文、panic 副本重复 error、cell metrics /
      render skipped 级别、换视图预算复位）共存，修复与归因都 MUST 见同文件的其余场景

#### Scenario: 模拟器 debug 构建的聚合归因

- **WHEN** 审计模拟器（x86_64）上 debug 构建的一次冷启动加一次真实 HOME 往返
- **THEN** 应用进程 W/E 的计数与处置 MUST 为（合计 42 条）：
      - `com.termux` 12 条（10 条 `Accessing hidden method/field ...` + 2 条 x86 CPU
        变体提示）：隐藏 API 访问告警的调用方是调试期依赖 `curtains.internal.*`
        （LeakCanary 的传递依赖，经 dex 反查定位到具体类）；该依赖是
        `debugImplementation`、发布构建不含，应用自身源码无这些反射调用
        （`WindowManagerGlobal`／`ActivityThread`／`SystemProperties` 均无命中），
        MUST NOT 记为应用侧缺陷，MUST NOT 以过滤该 tag 处置
      - `wgpu_core` 9 条（`Missing downlevel flags` 及其能力清单）：上游对适配器缺少
        `SURFACE_VIEW_FORMATS` 的无条件提示，应用侧无 API 可关，与 13-25-24 的归因同
      - 慢帧 5 条（`SLOW_RENDER_PHASE`／`SLOW_CELL_BUILD`／`SLOW_RENDER_INNER`／
        `SLOW_FRAME`／`SLOW_LOOP`）：模拟器软件渲染下的冷启动首帧（约 30ms），是应用
        侧诊断而非缺陷，处置按限频口径
      - `RemoteInputConnectionImpl` 6 条、`WindowOnBackDispatcher` 3 条、`HWUI` 2 条、
        `ziparchive` 2 条、`InteractionJankMonitor` 1 条、`FrameTracker` 1 条
        （`force finish cuj ... IME_INSETS_SHOW_ANIMATION`）、`platform` 1 条
        （`Failed to open rendernode`）：平台与框架路径，无应用栈；其中失活输入连接的
        告警见同文件场景
- **AND** 本次审计 MUST NOT 发现应用侧 E 级缺陷条目：两条 E 分别来自平台 jank 跟踪
      与平台渲染节点打开，应用自身 tag 里除慢帧诊断外全为 info 级
- **AND** 平台/框架条目 MUST NOT 以过滤 tag 或降级日志处置

#### Scenario: 上游未实现的矩形区域属性动作

- **WHEN** 日志出现上游 VT 解析器对 VT420 矩形区域属性（DECCARA／DECSACE）的未实现告警
- **THEN** 归因 MUST 记录该序列的协议名与发出方（客户端的屏幕清理序列），MUST 说明应用
       在 VT 输入预处理中丢弃该序列，且不自创协议未定义的应答

#### Scenario: 2026-11-20 运行日志（12-45-04）的聚合归因

- **WHEN** 审计 `logcat_2026-11-20_12-45-04.txt`（主进程 7807，冷启动到 12:45:01，
      跨 6 次原生窗口创建、1 次设备丢失重建与 3 次会话退出）
- **THEN** 应用进程 W/E 合计 MUST 为 70 条，逐条处置 MUST 为：
      - `mali_gralloc` 48 条（`Requested R8 format is not supported with this allocator`
        24 + `ERROR: Unrecognized and/or unsupported format 0x38 and usage 0xb00` 24）：
        即 6 次新窗口 × 8 条（每次 12 次格式探测 × 2 行），源头与 13:25-24 相同
        （AOSP `vulkan/libvulkan/swapchain.cpp` 对 `VK_FORMAT_R8_UNORM` 的
        `AHardwareBuffer_isSupported` 探测，厂商 HIDL 分配器不支持 R8 故回这条错），
        应用侧无 API 可改，MUST NOT 以换 surface 格式或过滤该 tag 处置；
      - `libc` 4、`ziparchive` 4、`FeatureOpt` 2、`Typeface` 1：厂商框架的属性读取
        拒绝、dex 元数据探测、特性开关与标字体，均无应用栈，MUST NOT 记为应用缺陷；
      - `wgpu_core::instance` 4：两次适配器枚举（冷启动 + 设备丢失后重建）各 2 条，
        判据与 11-16 相同，应用侧无 API 可关；
      - `Parcel` 2：平台编组路径读 null binder，MUST NOT 记为应用缺陷；
      - `com.termux` 2：1 条 debug 构建 jdwp 未启动、1 条 finalizer 依赖告警
        （`Cleared Reference was only reachable from finalizer`），均无应用栈；
      - `native::render::context::gpu` 2：1 条 error（首帧呈现后 5ms 的设备丢失）
        与 1 条 warn（设备丢失后的重建完成），成因在 `render-stability` 的设备丢失
        场景逐条留档；
      - `native::android::ffi::render::frame` 1：设备丢失当帧的一次 `render skipped`，
        与设备丢失同源，MUST NOT 以过滤该 tag 处置
- **AND** 同 UID 子进程的 50 条 MUST 全部归为用户 shell 的内核审计与程序输出：
      `git` 34、用户 shell 11、`login` 3、`DefaultDispatch` 2，无应用栈
- **AND** 本份日志 MUST 记下首帧与设备丢失的时序：`first frame presented`
      （12:38:40.352）→ `GPU device lost`（12:38:40.357）→ `render skipped`
      （12:38:40.369）→ `surface invalidated (attempt 1/5)`（12:38:40.370）→
      `GPU rebuilt after device loss`（12:38:40.538），即设备丢失发生在首帧呈现后
      5ms，其后的换窗与设备重建在 186ms 内完成，应用侧无需人工干预即恢复
- **AND** 本份日志的 6 条 `I/com.termux: Compiler allocated ...` MUST 记为渲染循环
      的待编译方法体量越线（`RenderSupervisor$startRenderThread$renderThread$1$1
      .invokeSuspend`，4232KB × 3 与 4972KB × 3，三对分布于 12:38:41 至 12:39:26）：
      该数字是编译期的 arena 工作内存而非编译产物字节数（判据见 AOSP
      `runtime/jit/jit.cc` 的 `Jit::AddMemoryUsage` 与
      `compiler/optimizing/optimizing_compiler.cc` 的 `ArenaAllocator::BytesUsed`），
      MUST NOT 写成「单方法被编译成 4.9MB／5.8MB」；处置见同文件的
      「渲染循环的待编译方法必须保持小体量」要求
- **AND** 平台/框架条目 MUST NOT 以过滤 tag 或降级日志处置

#### Scenario: 2026-11-22 运行日志（18-04-41）的聚合归因

- **WHEN** 审计 `logcat_2026-11-22_18-04-41.txt`（主进程 15703，冷启动到 18:00:15，
      跨 9 次原生窗口创建、3 次设备丢失重建、IME 弹出/隐藏与 1 次会话正常退出）
- **THEN** 应用进程 W/E 合计 MUST 为 11 条，逐条处置 MUST 为：
      - `native::render::context::gpu` 6：3 条 error（设备丢失）与 3 条 warn（每次丢失
        后的重建完成）。3 条 error 是**同一冷启动会话内的连续三次**丢失，且第 2、3 次
        丢的是上一次重建刚建好的**新**设备——判死位按设备世代归属，`mark_device_lost`
        只在判死位仍属槽位里当前设备时才记 error，故每条 error 都对应当时那一代设备：
        第 1 次在 `first frame presented`（18:00:16.592）后 6ms（新 surface 挂载后
        28ms），第 2、3 次分别在各自新 surface 挂载（18:00:17.010／17.268）后
        28ms／19ms；第 4 次重建的设备此后到日志结束未再丢失。底层根因未定位，
        MUST NOT 表述为已解决
      - 有界恢复链自愈，MUST NOT 依赖人工干预或进程重启：从丢失到新窗口 `attachWindow`
        分别 252ms／122ms／379ms，到新 surface 挂载完成 412ms／230ms／485ms
      - `native::android::ffi::render::frame` 3：设备丢失当帧的 `render skipped`
        （warn，限频），与设备丢失同源，MUST NOT 以过滤该 tag 处置
      - `Runtime` 2：`surface invalidated (attempt 2/5)` 与 `attempt 3/5`
        （attempt 1 在 18:00:16.611 为 info，未计入 W/E），换视图预算在前台会话内未复位，
        限流生效（快试 5 次 ≥500ms、用尽后转慢重试 5s）
- **AND** `mali_gralloc` 72 条（9 次新窗口 × 8 条）MUST 归为平台 R8 格式探测：
      AOSP `vulkan/libvulkan/swapchain.cpp` 对 `VK_FORMAT_R8_UNORM` 的
      `AHardwareBuffer_isSupported` 探测，厂商 HIDL 分配器不支持 R8 故回这条错；
      应用侧无 API 可改，MUST NOT 以换 surface 格式或过滤该 tag 处置
- **AND** `wgpu_core::instance` 8 条（4 次适配器枚举 × 2）MUST 归为上游
      `SURFACE_VIEW_FORMATS` 缺失，应用侧无 API 可关
- **AND** `Parcel` 3 条 MUST 归为平台编组路径读 null binder，MUST NOT 记为应用缺陷
- **AND** `libc` 4 条（`persist.unipnp.debug` 2 + `persist.vendor.gpu.fbc` 1 +
      `vendor.MB.running` 1）MUST 归为厂商属性读取拒绝，MUST NOT 记为应用缺陷
- **AND** `ziparchive` 4 条（`unisoc-framework.dm` 2 + `unipnp-framework.dm` 2）MUST 归为
      平台 dex 元数据探测，MUST NOT 记为应用缺陷
- **AND** `FeatureOpt` 2 条 MUST 归为厂商折叠态探测，MUST NOT 记为应用缺陷
- **AND** `com.termux` 1 条（debug 构建 jdwp 未启动）与 `Typeface` 1 条 MUST 归为
      平台/debug 路径，MUST NOT 记为应用缺陷
- **AND** SELinux 审计 93 条（`fish` 47 + `git` 35 + `login` 5 + `DefaultDispatch` 2 + <!-- nosemgrep: semgrep.no-prohibited-shells -->
      `chroot-ng` 1 + `denied { link }` 1 + `denied { getattr }` 2）MUST 全部归为内核审计：
      同 UID 子进程的 granted 探测与用户 shell 的 open denied，应用是检查的主体而非成因
- **AND** `Compiler allocated` 4 条（4237KB / 4972KB / 4232KB / 4972KB）MUST 记为
      渲染循环的待编译方法体量越线（`RenderSupervisor$startRenderThread$renderThread$1$1
      .invokeSuspend`），处置见「渲染循环的待编译方法必须保持小体量」要求
- **AND** 上述 4 条 MUST 标为**拆分前**的证据：登记本样本的提交同时完成了渲染循环的
      拆分，即日志先于拆分被采集；MUST NOT 据此宣称拆分后仍越线
- **AND** 拆分后的复核 MUST 以「`invokeSuspend` 的 dex 指令数」与「热循环下该行是否
      出现」两据并用：已安装构建（含拆分）中
      `RenderSupervisor$startRenderThread$renderThread$1$1.invokeSuspend` 仅 87 条
      dex 指令（整条循环已移到独立的 `runOneIteration`）；API 35 x86_64 模拟器上在
      持续输出下渲染线程 60 秒内耗尽 60 秒 CPU（热循环），全程未出现
      `Compiler allocated`。arm64 真机复测证据仍缺失，MUST NOT 据此表述为
      「真机已达标」
- **AND** IME 弹出/隐藏触发的 surface 销毁重建（18:00:18.815 showSoftInput →
      18:00:22.471 setRenderPaused(true) → 18:00:40.957 attachWindow）MUST 记为
      平台窗口生命周期，应用侧按 bounded recovery 自愈，MUST NOT 以降低重试频率处置
- **AND** 本份日志 MUST NOT 被当作性能缺陷的证据：80 条 `loop timing window` 与
      80 条 `frame timing window` 均为 info；`SLOW_LOOP`、`SLOW_RENDER`、`SLOW_CELL_BUILD`
      均为 0；唯一一次 `loop timing window` avg=46.69ms（p95=500.67ms）出现在
      IME 弹出期间的空闲闭锁驻留，按去掉驻留后的判定不构成慢渲染
- **AND** 平台/框架条目 MUST NOT 以过滤 tag 或降级日志处置

#### Scenario: 2026-11-23 运行日志（11-05-03）的聚合归因

- **WHEN** 审计 `logcat_2026-11-23_11-05-03.txt`（主进程 31683，同一冷启动会话内
      14 次设备丢失、14 次重建成功、15 代设备，跨多次换窗与切后台）
- **THEN** 应用自身 W/E 合计 MUST 为 76 条，逐条处置 MUST 为：
      - `native::render::context::gpu` 28：14 条 error（每次丢失）与 14 条 warn
        （每次重建完成）。多数新设备在挂载后 14–20ms（首帧后 6–15ms）再次丢失；
        首次丢失在 `first frame presented`（11:02:36.555）后 6ms；11:03:05.9
        重建的一代存活到日志结束。底层根因（Mali-G57 在首帧后丢失）未定位，
        有界恢复是缓解而非消除，MUST NOT 表述为已解决
      - `native::android::ffi::render::frame` 35：设备丢失期间的 `render skipped`
        （warn，500ms 限频；丢失当帧的余波，MUST NOT 以过滤该 tag 处置）
      - `Runtime` 13：`surface invalidated (attempt 2..5/5)` 与 slow retry 共 11 条
        warn；5 次快试用尽的 error 2 条（11:02:38.599 与 11:03:05.795 两次用尽）。
        11:03:03 单帧成功把预算清回 `attempt 1/5`，随即重演一次 5 连快试——该缺陷
        已修（恢复确认需连续稳定 60 帧，见 `render-stability` 的「单帧成功不能
        清零预算」）
- **AND** `mali_gralloc` 144 条（72 次 R8 探测 × 2）MUST 归为平台 R8 格式探测：
      AOSP `vulkan/libvulkan/swapchain.cpp` 对 `VK_FORMAT_R8_UNORM` 的
      `AHardwareBuffer_isSupported` 探测，应用侧无 API 可改，MUST NOT 以换
      surface 格式或过滤该 tag 处置
- **AND** `wgpu_core::instance` 30 条（15 次适配器枚举 × 2）MUST 归为上游
      `SURFACE_VIEW_FORMATS` 缺失，应用侧无 API 可关
- **AND** `com.termux` 13 条 MUST 全部归为调试依赖与平台：12 条 hidden API 反射
      告警，来源以依赖 jar 的字节码核实——`curtains` 1.2.5（LeakCanary 的传递
      依赖）的 `RootViewsSpy`/`WindowManagerSpy` 读 `WindowManagerGlobal` 的
      `getInstance`/`mViews`；`leakcanary.ServiceWatcher` 读
      `ActivityThread.currentActivityThread`/`mH`/`mServices`、`Handler.mCallback`、
      `Singleton.get`/`mInstance`、`ActivityManager.IActivityManagerSingleton`；
      `leakcanary.RootViewWatcher` 读 `DecorView.mWindow`；`androidx.compose.ui` 的
      `AndroidComposeView` 读 `SystemProperties.addChangeCallback`；
      `androidx.compose.foundation` 的 `ScrollableNode` 调
      `ViewTreeObserver.dispatchOnScrollChanged`。另有 1 条
      `Cleared Reference was only reachable from finalizer`：debug 构建
      `Late-enabling -Xcheck:jni` 下 ART 的引用清理诊断，无应用栈。LeakCanary 为
      `debugImplementation`，release 包不含上述反射；MUST NOT 以过滤该 tag 处置
- **AND** `Parcel` 3 条 MUST 归为平台编组路径读 null binder；`libc` 4 条
      （`persist.unipnp.debug` 2、`persist.vendor.gpu.fbc` 1、`vendor.MB.running`
      1）、`ziparchive` 6 条（`unisoc-framework.dm` 2、`base.dm` 2、
      `unipnp-framework.dm` 2）与 `FeatureOpt` 2 条 MUST 归为厂商属性／dex 元数据／
      折叠态探测；`Typeface` 1 条 MUST 归为平台字体资源；
      `RemoteInputConnectionImpl` 3 条（`requestCursorAnchorInfo on inactive
      InputConnection`）MUST 归为平台输入法对已失效连接的回调；以上 MUST NOT 记为
      应用缺陷
- **AND** 7 条 `native::android::logging` 的 panic（debug 级）MUST 记为被
      `poll_outcome` 接住的预期内设备丢失 panic：`Error in Device::poll: Validation
      Error` 的载荷确含 `Caused by: Parent device is lost`（上游 `format_error`
      的固定格式），真机日志里只剩前半句是**按行采集在空行处截断**所致；修复后
      正文压成单行，原因不再丢失
- **AND** 本份日志 MUST NOT 被当作性能缺陷的证据：`SLOW_LOOP`、`SLOW_RENDER`、
      `SLOW_CELL_BUILD` 均未出现在 W/E 清单中；平台/框架条目 MUST NOT 以过滤 tag
      或降级日志处置

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
- **THEN** 归因 MUST 给出上游判据位置与**可复算的判据值**：AOSP
      `vulkan/libvulkan/swapchain.cpp`（`libvulkan` 的 `GetSurfaceFormats`）在合成
      `vkGetPhysicalDeviceSurfaceFormatsKHR` 的候选列表时，对
      `AHARDWAREBUFFER_FORMAT_R8_UNORM` 无条件调 `AHardwareBuffer_isSupported`；其
      探测描述符在源码里固定为 `1x1x1`、`format = 0x38`、
      `usage = COMPOSER_OVERLAY(0x800) | GPU_SAMPLED_IMAGE(0x100) | GPU_FRAMEBUFFER(0x200) = 0xb00`
      ——日志里那一对「Requested R8 format…」与「Unrecognized…0x38/usage 0xb00」
      与这四个数逐一对应，故可判定调用方就是该函数，而非应用申请的缓冲
- **THEN** MUST 说明同一函数还按同一模板探测 `R5G6B5`、`RGBA16F`、`RGB10A2`
      （Android 14 起另有 `RGBA10x6`），这些在本机 gralloc 上探测通过故无日志——只有
      R8 这一项被拒，正是「列表里只有 R8 报错」的原因；MUST NOT 把「只有 R8 报错」
      解释成应用只申请了 R8
- **THEN** MUST 指出被拒的**是厂商旧 mapper 分配器的能力边界**：日志原文为
      「R8 format is only supported with the AIDL allocator」，即 R8 只有 AIDL
      分配器支持，该机用的是旧 mapper 路径；MUST 说明这是设备侧能力位，MUST NOT
      记为应用选错格式、MUST NOT 建议改用 R8
- **THEN** MUST 指出被拒的是平台格式枚举的支持性探测，不是应用申请的呈现缓冲；应用
      实际请求 `Rgba8Unorm`（与窗口缓冲 `RGBA_8888` 同格式），交换链照常建立、帧
      照常呈现，MUST 说明应用侧无开关（除非放弃 Vulkan 表面），MUST NOT 记为应用
      选错格式；MUST 同时记明该组条数与表面重建次数的倍数关系（8 行/次 = 4 次
      R8 探测 × 每次 2 条 E），以及应用侧减少重建的手段
- **THEN** MUST 写明倍数关系里那两层「翻倍」的来源，使该口径可复算：一次表面能力
      查询在 ash 的 Vulkan 枚举两段式下是 2 次 `vkGetPhysicalDeviceSurfaceFormatsKHR`
      调用（先问个数、再取数据），AOSP 实现两次都重建候选列表、两次都探测 R8；而
      应用侧每次挂载查 2 次表面能力（显式 `get_capabilities` + `configure` 内部的
      能力校验）——2 × 2 = 4 组/次。缺任一层，倍数关系即无法从源码复算

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
- **AND** 逐条 MUST 指认读取方：`persist.vendor.gpu.fbc` 紧跟一条
      `I/mali_gralloc: SPRD: get persist.vendor.gpu.fbc 1`，即分配器自己读、读到的是
      缺省值 1（该读取与同毫秒的 R8 探测同一段分配器初始化，见同文件的分配器场景）；
      `persist.unipnp.debug` 紧邻 `ziparchive` 对 `unipnp-framework.dm` 的探测，属
      厂商框架装载；`vendor.MB.running` 在 `FeatureOpt` 读出 490 项特性之后的进程
      初始化段，属同一厂商框架

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

#### Scenario: 运行时 GC 计数直方图的跳窗告警

- **WHEN** 日志出现 ART 打出的
      `Reducing the number of considered missed Gc histogram windows from N to 100`
      （真机 tag 为进程名）
- **THEN** 归因 MUST 给出 AOSP ART 的判据位置：`runtime/gc/heap.cc` 的
      `Heap::UpdateGcCountRateHistograms` 用 `NanoTime()` 差值除以 10s 窗口得到的
      窗口数超过 `kGcCountRateHistogramMaxNumMissedWindows`（100）时打这条 W 并
      截断，`kGcCountRateHistogramWindowDuration` 与上限都在 `runtime/gc/heap.h`
- **AND** MUST 说明它是平台对「距上次 GC 直方图更新已过多个窗口」的统计保护
      （防 `NanoTime()` 非单调时 CPU 空转），每个虚拟机只打一次，与应用分配量、
      内存泄漏无关，应用侧无开关

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

#### Scenario: 无关设置变化不得重复下发日志等级

- **WHEN** 用户修改字号、主题等与日志等级无关的设置，设置快照重新发射
- **THEN** 终端 MUST NOT 把同一日志等级重新下发给 Kotlin 与原生两侧，MUST NOT 逐次
      记录 `log level set`
- **AND** 用户显式选择新的等级时 MUST 照常下发并记录

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

### Requirement: 用户字体覆盖在同一进程内不得重复装载

`~/.termux/font.*`（及 `~/.config/termux/font.*`）的探测结果按进程缓存：原生渲染
字体库在进程内只有一份，`loadFontFile` 会把文件追加为一个面且不可回收。同一进程内
新建会话、重复下发字族时 MUST 复用已解析的字族，MUST NOT 再次调用 `loadFontFile`。

去重判据 MUST 包含文件大小与 mtime，MUST NOT 只看路径：用户原地替换同名字体后
MUST 重新装载。文件不存在时 MUST 清除缓存，使删除覆盖后恢复系统字体。

#### Scenario: 新建会话不重复装载同一覆盖

- **WHEN** 进程内已有会话装载过 `~/.termux/font.ttf`，随后新建会话再次下发字族
- **THEN** 终端 MUST 复用上次解析出的字族，MUST NOT 再次调用 `loadFontFile`

#### Scenario: 原地替换字体后重新装载

- **WHEN** 字体文件路径不变，但大小或 mtime 变化
- **THEN** 终端 MUST 重新装载并返回新的字族

#### Scenario: 删除覆盖后恢复系统字体

- **WHEN** `~/.termux/font.*` 被删除
- **THEN** 终端 MUST 清除进程缓存并按设置里的字族下发

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

### Requirement: 仪器化运行期噪声必须逐类归因，不得当作产品缺陷或据此改阈值

仪器化用例在模拟器上跑完整套件时，日志里出现的告警有一批来自测试环境与外部二进制，
与真机产品路径无关。处置 MUST 是**写明归因**：MUST NOT 提高慢帧阈值、MUST NOT
过滤日志；30–60ms 的一次性瞬时慢帧 MAY 记为 info，持续超预算或 ≥60ms MUST 记为 warn。

#### Scenario: 软件渲染下的高频慢帧

- **WHEN** 模拟器无 rendernode（`Failed to open rendernode`），走 SwiftShader，
      每帧约 33ms
- **THEN** `SLOW_FRAME`／`SLOW_LOOP` 会成百条出现，MUST 归因为环境渲染性能；
      阈值 `SLOW_FRAME_LOG_THRESHOLD_MS`（30ms）MUST 保持不变

#### Scenario: 用例 teardown 拆队列

- **WHEN** 用例结束、Activity 被强杀，平台在仍有 dequeued buffer 时拆掉队列
- **THEN** `E/Surface freeAllBuffers`、`E/BufferQueueProducer … abandoned`、
      `E/vulkan dequeueBuffer failed: No such device (-19)` MUST 归因为 teardown；
      产品切后台由 surface 挂载门闸先暂停渲染，MUST NOT 出现在真机切后台路径

#### Scenario: 失活输入连接上的文本请求

- **WHEN** 输入法在应用侧 InputConnection 已失活后请求上下文
- **THEN** `RemoteInputConnectionImpl` 的 `getSurroundingText` 与
      `requestCursorAnchorInfo` MUST 归为同一族（调用方在输入法进程），
      MUST NOT 由应用侧吞掉该日志

#### Scenario: 外部 bootstrap 二进制

- **WHEN** 引导安装运行预编译二进制
- **THEN** `W/com.termux Unexpected CPU variant for x86: x86_64` 与
      `SecondStageRunner coreutils postinst exited with code 139` MUST 归因为
      外部二进制在模拟器的表现；安装器 MUST 保留既有重试兜底

#### Scenario: 测试运行器扫描可选 runner

- **WHEN** UTP 扫描 instrumentation APK
- **THEN** `ScanningTestLoader Could not load class:
      androidx.test.uiautomator.UiAutomatorInstrumentationTestRunner` MUST 归因为
      可选 runner 类的探测失败，MUST NOT 当作用例失败

### Requirement: 渲染循环的待编译方法必须保持小体量

渲染循环的各段职责 MUST 各自成方法，循环体 MUST 只做流程编排：MUST NOT 把整条
循环（唤醒门控、视口推送、渲染发布、输出消费、失败处置、事件泵送、诊断发布与
循环计时）留在一个方法里。

判据是 ART 编译期的 arena 工作内存：应用自身的渲染循环方法 MUST NOT 触发
`Compiler allocated` 行（阈值 4MiB）。该行是 info 级，MUST NOT 以过滤它作为处置；
其数字 MUST 写明是编译期工作内存而非编译产物的字节数。

#### Scenario: ART 编译 arena 越线

- **WHEN** 日志出现 `Compiler allocated <大小> to compile ...RenderSupervisor...`
- **THEN** MUST 按职责继续拆分渲染循环，使该行不再出现；MUST NOT 以过滤该行、
      降级日志或调高 ART 阈值作为处置

#### Scenario: 编译 arena 的口径

- **WHEN** 记录该行的数字
- **THEN** MUST 写明它是编译期 arena 工作内存（判据为 AOSP `runtime/jit/jit.cc`
      的 `Jit::AddMemoryUsage` 与 `compiler/optimizing/optimizing_compiler.cc` 的
      `ArenaAllocator::BytesUsed()`），MUST NOT 写成编译产物的字节数

#### Scenario: 不依赖真机日志的复核判据

- **WHEN** 需要复核该要求是否仍成立（真机日志缺失或尚未采集）
- **THEN** MUST 用 `dexdump` 统计 `RenderSupervisor$startRenderThread$renderThread$1$1
      .invokeSuspend` 的 dex 指令数作为结构判据：整条循环留在挂起函数里时该方法是
      数兆 arena 的来源，拆分后它只应是流程编排（数十条指令量级，实测 87 条）
- **AND** 结构判据 MUST NOT 单独作为「已达标」的结论：仍需热循环下不出现
      `Compiler allocated` 的运行证据；两者都缺时结论 MUST 为「未验证」

### Requirement: 三份真机日志的 W/E 条目必须逐类归因

样本为 Mali-G57 设备上的三份同日日志（10-13-40、10-52-29、17-40-54）。全部
W/E 条目 MUST 按来源逐类归因；MUST NOT 以过滤 tag、降级日志或调阈值的方式让其中
任何一条「消失」。逐类判据与计数由下列场景给出。

#### Scenario: 平台与驱动条目的判据

- **WHEN** 归因 `E/mali_gralloc`（40／8／95 条）与 `W/wgpu_core::instance`（10／2／18 条）
- **THEN** MUST 分别归因为每次表面格式查询与每次设备创建的平台/驱动输出；
      `0x38` 即 `AHARDWAREBUFFER_FORMAT_R8_UNORM`，MUST NOT 据此改动交换链格式或
      downlevel 配置
- **AND** 每次格式查询产生 4 对 `Requested R8 format is not supported with this
      allocator` 与 `ERROR: Unrecognized and/or unsupported format 0x38 and usage 0xb00`

#### Scenario: 应用侧设备丢失条目的判据

- **WHEN** 归因 `E/native::render::context::gpu`（3／0／8 条）与
      `W/native::render::context::gpu`（3／1／8 条）
- **THEN** MUST 按每次丢失一条 error、每次重建一条 warn 归因；丢失余波按
      `render-stability` 的「丢失余波不得再记 error」处理
- **AND** `render state prefetch skipped: ... GPU device lost` MUST 记 debug

#### Scenario: 渲染限频与慢循环条目的判据

- **WHEN** 归因 `W/native::android::ffi::render::frame`（3／0／35 条）与
      `W/Runtime`（1／0／8 条）
- **THEN** MUST 把 `render skipped` 记为限频 warn；`SLOW_LOOP` 的 `render` 分量含
      `attach_surface` 首帧的管线创建（实测 32.8ms），MUST 与稳态帧区分；稳态窗口
      实测 avg≈14ms（≈70fps，vsync 16.7ms），MUST NOT 当作卡顿证据
- **AND** `E/Runtime`（0／0／1 条）的换视图快预算降级 MUST 记 warn，MUST NOT 记 error

#### Scenario: 框架条目与用户命令输出的判据

- **WHEN** 归因 `E/com.termux`、`E/Typeface`、`W/libc` 等框架条目，以及以
      `__progname` 为 tag 的终端子进程输出（`W/fish`／`W/git`／`W/login`／
      `W/eza`／`W/nix-your-shell`／`W/chroot-ng`）
- **THEN** MUST 分别记为进程启动提示、框架自身输出、平台噪声与用户命令；
      PTY 不可写时 bionic 把 stderr 回退到 logd，MUST NOT 当作应用日志过滤或改写
- **AND** `W/Parcel`／`W/ziparchive`／`W/DefaultDispatch`／
      `W/RemoteInputConnectionImpl`／`W/FeatureOpt` MUST 按各自既有条目归因

#### Scenario: 极小条目也必须逐项归因

- **WHEN** 归因三份日志里各自只有 1–2 条的 `W/com.termux`（`Reducing the number of
      considered missed Gc histogram windows`）、`W/native::android::ffi::render::diagnostics`
      （`SLOW_RENDER_SAMPLE`）与 `W/native::android::ffi`（`render state prefetch skipped`）
- **THEN** `W/com.termux` MUST 归为 ART 并发 GC 的自适应窗口调整（平台内部启发式，
      无应用栈；本条即 10-52-29 的那 1 条）
- **AND** `SLOW_RENDER_SAMPLE` MUST 与同拍的 `SLOW_FRAME`／`SLOW_LOOP`／
      `SLOW_RENDER_INNER` 并为**同一帧**的记账（17-40-54：四条落在 39.546–39.585 的
      39ms 内，`slow_frames` 均为 1，`phase1` 是 `attach_surface` 的首帧管线创建），
      MUST NOT 计为多次独立慢帧
- **AND** `render state prefetch skipped` MUST 记 **debug** 并保留原因全文，按
      `render-stability` 的「丢失后的预热失败不算新的 error」处置

#### Scenario: 同一条目跨日志的计数口径

- **WHEN** 汇总三份日志的同一 tag
- **THEN** MUST 分别给出每份的条数与出现条件（每次表面格式查询／每次设备创建／
      每次丢失各几条），MUST NOT 只给总数——计数随查询与重建次数变化，
      单看总数无法判断是否回归

#### Scenario: 丢失循环的证据链

- **WHEN** 复核 17-40-54 日志的 8 次丢失
- **THEN** MUST 以「丢失前最后一条 GPU 调用 + 距 `attach_surface: configured` 的
      毫秒数」为证据：8 次全部是非渲染线程的 `device poll`、全部在挂载后 1–14ms，
      据此归因到回读的阻塞等待（见 `render-stability`）

### Requirement: 2026-11-25 两份真机日志的逐类归因

两份日志（`09-45-38`、`09-47-15`，后者覆盖前者并延续至 09:47）共 1393 条记录，
W/E/F 合计 426 条。归因 MUST 逐条给出，MUST NOT 以「平台噪声」概括了事，
MUST NOT 加过滤器、降级日志或提高阈值。

#### Scenario: 应用侧错误与稳定性

- **WHEN** 逐条检查两份日志的 `E` 与 `W`
- **THEN** 应用代码 MUST NOT 出现 panic、`device lost`、`ERROR_SURFACE_LOST_KHR`、
      `Parent device is lost`、`surface invalidated` 或 `readCoverFrame`；
      与 11-24 日志的丢失循环对比，本批日志 MUST 记为「应用侧触发条件已不再出现」
- **AND** `E/com.termux Not starting debugger since process cannot load the jdwp agent`
      MUST 归因为进程无法装载 jdwp 的平台行为，MUST NOT 由应用修代码
- **AND** 唯一的应用侧诊断是 `SLOW_RENDER_INNER`／`SLOW_FRAME`／`SLOW_LOOP`，
      各自只有 3 组，且都落在首帧管线建立或首次新输出的瞬时相位
      （`query` 9–13ms、`draw` 20–28ms），MUST 保留 30ms 阈值与 500ms 限频，
      MUST NOT 调高阈值把它们藏起来

#### Scenario: GPU 与上游告警

- **WHEN** 逐条检查 `mali_gralloc` 与 `wgpu_core::instance`
- **THEN** `Requested R8 format is not supported with this allocator` 与
      `ERROR: Unrecognized and/or unsupported format 0x38 and usage 0xb00`（各 44 条）
      MUST 归因为平台分配器对 R8 格式的能力探测，应用无 API 可关
- **AND** `Missing downlevel flags: DownlevelFlags(SURFACE_VIEW_FORMATS)` 与
      `DownlevelCapabilities`（各 6 条）MUST 归因为 wgpu 对 Vulkan 设备能力的如实上报，
      MUST NOT 当作应用缺陷

#### Scenario: 用户命令与系统集成告警

- **WHEN** 逐条检查 `fish`／`git`／`login`／`bash`／`clear`／`yazi`／`comma`／`kitten`／
      `DefaultDispatch`／`chroot-ng` 的 `avc: granted/denied` 条目
- **THEN** MUST 归因为真机上用户输入命令经 SELinux 审计的结果（`granted` 占绝大多数，
      `denied` 来自 `/proc` 与 tmpfs 的常规访问），MUST NOT 由应用吞掉
- **AND** `ziparchive Unable to open …/*.dm`、`FeatureOpt ensureGetRight`、
      `libc Access denied finding property`、`Parcel Expecting binder but got null!`、
      `Typeface updateDefaultTypeface`、`CtsUtil`、`CheckTime`、
      `Z##ZSSOSConfig parseXml` 与 `Choreographer Skipped … ZTE_SUPERKILLER`
      MUST 归因为厂商系统组件的内部日志，不是应用写入的条目

#### Scenario: 调试工具的隐藏 API 反射告警

- **WHEN** 冷启动出现 `Accessing hidden method/field … (unsupported, reflection, allowed)`
- **THEN** LeakCanary 自动安装造成的 7 条 MUST 由移除
      `MainProcessAppWatcherInstaller` 与 `PlumberInstaller` 消除，泄漏检测改由
      `LeakCanaryIntegrationTest` 在主线程手动安装，MUST NOT 只做归因
- **AND** `WindowManagerGlobal.getInstance`／`mViews` 两条来自 DebugOverlay 使用的
      `curtains`；DebugOverlay 不是规范要求的产物，已连同其 debug 依赖与应用子类移除，
      告警随之消失，MUST NOT 改为过滤日志
- **AND** 余下的 `SystemProperties.addChangeCallback` MUST 归因为
      `androidx.compose.ui:ui` 的 `AndroidComposeView.Companion`：它反射系统属性回调，
      以便属性变化时刷新 Compose 视图；该告警带 `allowed`
- **AND** 消除这一条的可执行路径只有替换 Compose UI 或等待上游移除反射，二者均属产品
      决策，MUST NOT 通过 disable API 检查、过滤器或降级日志把它藏起来
- **AND** 复核判据为同一模拟器冷启动的应用进程反射告警计数：改造前 10 条，
      取消泄漏检测自动安装后 3 条，移除 DebugOverlay 后 MUST 只剩 1 条

#### Scenario: 冷启动主线程停顿

- **WHEN** 09:44:59 的循环窗口出现 `avg=45.5ms p95=500.8ms max=501.5ms`，
      且同期 `CheckTime` 记 `application classloader create took 810ms` 与
      `installContentProviders took 578ms`
- **THEN** MUST 归因为冷启动期间主线程被 ContentProvider 装配与类装载占用
      （`Choreographer Skipped 36/37 frames` 是同一停顿的平台侧记账），
      MUST NOT 把它归为渲染回归或据此放宽慢帧阈值
- **AND** 优化方向的证据 MUST 是合并清单里的 provider 数量与同一设备的新日志，
      MUST NOT 只凭猜测增减 provider

#### Scenario: 近期提交与日志形态的对应

- **WHEN** 核对六日内改动日志、渲染、滚动与 Shell 的提交，并在新日志里找对应形态
- **THEN** `feat(log): Android 日志接入 Timber 并用 governor 节流慢日志`（后续替换为
      Log4K）对应的 `SLOW_RENDER_INNER`／`SLOW_FRAME`／`SLOW_LOOP` MUST 带窗口计数与
      500ms 限频；新日志每组只有一条，MUST NOT 被当成回归
- **AND** `fix(render): 遮罩回读等待本设备提交并按设备丢失归一` 对应的丢失循环在新日志
      为 0，`fix(scroll): 新输出贴底同拍清掉亚行余量` 对应的 245 行回滚仍正常记账；
      `chore(native): 推进 libghostty-vt 并适配破坏性 API` 后无 panic，三者 MUST 记为
      「新日志未复现旧症状」，MUST NOT 升级成「已彻底修复」
- **AND** `refactor(log): 用 Log4K 替换 Timber` 之后的两个优化（移除 Log4K 的
      `ContextProvider`、恢复 R8 优化）MUST 以合并清单 provider 数与 release APK
      的真机冷启动日志复核；日志采集早于这两个优化，MUST NOT 用这批日志为它们背书

#### Scenario: Release 缺省等级与构建规范一致

- **WHEN** 新安装的 release 构建首次启动并取日志等级缺省值
- **THEN** Kotlin 与原生两侧 MUST 落在信息级（构建规范「release 构建的日志等级
      INFO」），MUST NOT 用警告级作缺省；调试级仍由 R8 剥离，不会带回调试噪声
- **AND** 判据 `LogLevelTest` 的「缺省等级与构建类型一致」按 `BuildConfig.DEBUG`
      分别断言 `DEBUG` 与 `INFO`，MUST NOT 只测一侧

#### Scenario: CI 构建与仪器化运行的告警逐类归因

- **WHEN** 检查 GitHub Actions 运行 `45533749679` 的完整日志
- **THEN** 结果 MUST 记为成功：262 个 `connectedDebugAndroidTest`、137 个 benchmark
      与 1 个 nix 用例全部通过（0 failed、0 skipped），debug／nix／release 三个 APK
      均产出并完成发布；MUST NOT 把构建成功写成「无告警」
- **AND** Gradle 的 `Deprecated Gradle features` 提示 MUST 归因为工具链与插件的弃用
      通告，SwiftShader 的 `WARNING: UNSUPPORTED: curExtension->sType` 与
      `cannnot unmap ptr ... protected range` MUST 归因为模拟器图形栈内部日志
- **AND** 模拟器启动期的 `Unable to connect to adb daemon on port: 5037`、
      `adb: device offline`、`Failed to process .ini file`、Quickboot 与 libX11
      装载提示 MUST 归因为设备启动竞态与宿主库探测，日志中随后成功启动并完成全部用例
- **AND** `Failure [not installed for 0]` 与 `Failure [DELETE_FAILED_INTERNAL_ERROR]`
      MUST 归因为 `scripts/test-emulator.nu` 中卸载未安装包的两处预期失败；该脚本是
      受保护文件，MUST NOT 为消除这些行而修改它，也 MUST NOT 在日志侧过滤它们
- **AND** `Emulator client has not yet been configured` 与 `Netsim Wifi … gone due to
      Stream removed` MUST 归因为模拟器关停与 netsim 流结束，均发生在全部用例成功之后
