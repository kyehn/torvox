# logcat-hygiene Specification

## MODIFIED Requirements

### Requirement: 外部与上游条目必须逐条归因并留档

日志审计 MUST 把每条 W/E/F 归到「应用自身」「上游终端」「平台」「外部程序」之一，
并为每一类写明结论：应用自身条目 MUST 有根因与落地位置；上游条目 MUST 记录
终端上游与绑定层两处事实；平台与外部条目 MUST 说明无应用栈、不做绕过。

审计结论 MUST 以聚合形式留档（逐类计数、归因、处置），MUST NOT 把原始日志行、
时间戳、PID、设备标识或下载链接写入仓库。

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
        36.916ms 超过 30ms 阈值。属一次性冷路径，处置 MUST NOT 提高阈值或降级日志
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
        渲染成功后由 RenderSupervisor 复位），MUST NOT 以降低重试频率或隐藏日志处置
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

## ADDED Requirements

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
