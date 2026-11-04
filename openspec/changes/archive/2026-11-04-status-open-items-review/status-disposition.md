# STATUS 逐条处置

`docs/STATUS.md` 的 11 条待办逐条给出当前状态、根因、落盘位置与可运行判据。
仓内只留结论与计量，不留原始日志行、时间戳、PID 与设备标识。

状态口径：**已修**＝代码已改且有可运行的自动化判据；**已修待真机**＝代码已改、
自动化判据齐备，但结论涉及真机驱动行为，本仓环境（主机软件 Vulkan 与 CI 的
SwiftShader 模拟器）无法复现；**非本仓缺陷**＝经查证不对应本仓代码路径。

## 一、`kitten icat test.png` 后要 clear 两次

状态：已修。

根因：图像放置缺生成戳。`kitten icat` 这类直接放置的图像没有放置生成戳，清屏（ED2）
走回滚路径时撤不下虚拟放置，屏幕上的图像像素要等下一次全量重查才消失——于是表现为
「第一次 clear 把图放大盖住屏幕，第二次才清掉」。

落盘：`native/src/terminal/ghostty_terminal/internal/run.rs` 的 Kitty 放置登记与
`native/src/render/kitty.rs` 的放置撤销；规约 `openspec/specs/kitty-graphics/spec.md`
「清屏必须立即撤下图像放置并清除像素」。

判据：`KittyGraphicsPlacementTest` 的四个用例（含首次清屏即撤下放置、清屏后放置列表
为空、清屏走回滚路径时图像同样消失）全绿。

## 二、切换应用返回页面闪烁

状态：已修。

根因：`SurfaceView` 的 surface 在 `onStop` 之后被系统拆除时，窗口层的「开孔覆盖」状态
依赖随生命周期停止收集的订阅：写入发生在 `onStop` 之后，`onStart` 重启收集与返回后的
首帧绘制会抢跑，首帧按「有缓冲区」绘制时开孔区就透出桌面或上一个应用。

落盘：`TerminalRuntime.surfaceBufferPresent`（覆盖状态改为常驻收集）与
`TerminalScreen` 的窗口层底色覆盖；规约 `openspec/specs/render-stability/spec.md`
「切后台返回时开孔必须被覆盖且重新出帧」。

判据：仪器用例 `RenderPauseSemanticsTest.pausedWritesArePresentedAfterResume` 与
`SurfaceLossRecoveryInstrumentedTest`；本地 `:app:connectedDebugAndroidTest` 全量通过。

## 三、侧边面板过宽，布局不合理

状态：已修。

根因：会话抽屉宽度带 240dp 下限，在可用宽度小于该值的设备（如 ZTE 折叠屏外屏，
可用宽度约 236dp）上被强制抬宽到比可用宽度还宽，布局随即溢出。

落盘：`SessionDrawer` 的面板宽度改为 `fillMaxWidth(0.84f)` 加 `widthIn(max = 320.dp)`，
不设下限；规约 `openspec/specs/session-list/spec.md`「抽屉面板不遮挡系统栏」。

判据：抽屉面板顶边落在状态栏下沿、面板不覆盖系统栏的仪器断言；窄屏设备上的实机
复核需维护者硬件。

## 四、文档提供者找不出 rollback 文件警告

状态：部分收口——应用侧取证齐备，缺原始告警文本。

查证：AOSP `platform_packages_apps_DocumentsUI` 与 `frameworks/base` 的
`core/java/android/provider` 全文均无 `rollback` 字样；本仓亦无同名文件或符号。
该措辞不对应任何平台 API 或本仓符号，无法从代码侧还原其指向。

应用侧已具备的取证：`TerminalDocumentsProvider.trace` 对每次变更操作记录耗时与结果，
`isChildDocument` 与 `querySearchDocuments` 的告警路径各有落点，失败即抛回客户端而
非吞掉。

剩余条件：需要该告警的原始文本（logcat 的 tag 与整行）才能判断是客户端侧提示还是
平台侧行为。拿到文本前不改代码——凭猜测加兜底分支只会引入第二条假路径。

## 五、不能加载 jdwp agent 错误

状态：非本仓缺陷。

查证：`E/jdwp: ERROR: Could not load the jdwp agent` 由 ART 的 JDWP 代理在无法加载
原生代理时打出，属平台与模拟器层。本仓 manifest 未声明 `android:debuggable`，
发布构建不可调试，也不注册任何调试代理；仓内无 JDWP 相关代码与配置。

剩余条件：若在维护者的设备上复现，需连同模拟器镜像与系统版本一起看，属环境问题。

## 六、mali_gralloc 错误

状态：已修待真机。

根因：R8（`AHARDWAREBUFFER_FORMAT_R8_UNORM`，0x38）在 Mali 的分配器上不被支持。
厂商 Vulkan 驱动建交换链时会试分配该格式，失败即打出该告警。应用侧另有一个口子：
`select_surface_format` 的退化链末端是「任意格式」，设备只报告单通道格式时会把
`R8Unorm` 当交换链格式去申请窗口缓冲，申请必然失败。

落盘：`native/src/render/context/surface.rs` 的 `select_surface_format` 退化链只接受
三通道以上的格式，单通道与双通道格式一律不选，设备只报告这类格式时报错退出；
`TerminalLogcatRule` 把 `mali_gralloc` 与 `gralloc4` 纳入失败报告的取证标签表；
规约 `openspec/specs/render-stability/spec.md`「交换链格式必须是能承载颜色的格式」与
`openspec/specs/logcat-hygiene/spec.md`「设备分配器的格式探测告警」。

判据：`native/src/render/context/tests.rs` 的 `single_channel_format_is_never_selected`
与 `falls_back_to_multichannel_format_when_rgba8_family_is_absent` 两个用例。

剩余条件：真机复跑，确认告警条目数与表面重建次数仍是固定倍数、且画面正常。本仓环境
的 Mesa lavapipe 与 CI 的 SwiftShader 都不是 Mali 驱动，该复测不在本仓完成条件内。

## 七、`docs/persistence` 内容需要重新验证和更新

状态：已修（本轮复核）。

上游调研记录写成时本仓尚未实现响铃三态与 `termux.properties` 体系，文档里的
「本仓忽略 BEL」「`termux.properties` 不适用」等结论随实现推进变成错话。本轮订正：

- `osc-notifications.md`：本仓行的响铃口径改回三态执行，`on_bell` 从「未使用 API」
  改列「已使用 API」，`docs/STATUS.md` 的指针改为 `termux-configuration` 规约。
- `terminal-protocols.md`：桌面通知计划行里「DESIGN 忽略 bell」的同源决策改为与已接线
  的 BEL 区分职责。
- `termux-app.md`：`termux.properties` 的「不适用」改为已按 `CONFIG.md` 声明键集实现，
  上游白名单里未声明的键按「只允许下面声明的键」跳过。
- `libghostty.md`：`session.rs` → `session/`、`render/context.rs` → `render/context/`
  （两处已由单文件拆为目录）。

同时修正 `openspec/specs/upstream-alignment/spec.md` 的响铃取用台账：原把 `on_bell`
列为「不采用 / Kotlin 侧零消费者」，与实现相反；现拆出独立行并改为「采用」，其余
引用该前提的行一并订正。

## 八、`openspec/specs` 需要根据 `docs/specification` 重写

状态：部分收口——本轮修掉与实现相反的台账行，逐规范复核仍待继续。

现状：48 个能力规范全部通过 `openspec validate --all --strict`；`docs/specification/`
的每条声明都有对应 Requirement 或明确的「不实现」登记。本轮修掉响铃取用台账这一处
「规范与实现相反」的矛盾（见第七条）。

剩余条件：逐个规范与 `docs/specification/` 对读仍需一轮系统性复核。本仓的发现即修
机制已就位（发现→改规范→`openspec validate --all --strict`），不做一次性重写。

## 九、`android/benchmark` 需要更充分的性能测试并断言结果

状态：已修。

现状：6 个宏基准用例——冷启动、热启动、满回滚滚动、修饰键按压动画、输入法弹出动画、
会话抽屉动画——全部带断言（帧数下限、帧数下限＋呈现窗口判据、时长上限），实测值
先于断言输出，便于按真值复核。阈值按本地实测收紧：冷启动 3400ms、热启动 2000ms
（本地实测中位约 2259ms 与 1106ms）。

判据：本地模拟器实跑 `:benchmark:connectedBenchmarkReleaseAndroidTest` 6/6 全绿
（两次复跑分别为 3m23s 与 3m40s）。

剩余条件：CI 模拟器只有 2 核，本机 4 核，帧数下限按最差实测校准（满载宿主实测仅
65 帧，判据同时要求「有帧的呈现窗口数 ≥ 4」以免把宿主抢占误判成回归）。

## 十、`termux.properties` 全键对齐并逐键测试

状态：已修。

对齐口径：`docs/specification/CONFIG.md` 声明 18 条（其中 `shortcut.*` 一条含三个键，
共 20 个键），全部实现且全部被生产代码消费：取值集合、默认值、精度、范围与上游
`TermuxPropertyConstants` 一致，越界回退默认值而不钳到边界，未知键跳过且不使整份
文件作废。上游白名单里未在 `CONFIG.md` 声明的键（`night-mode`、`disable-*`、
`allow-external-apps` 等）按「只允许下面声明的键」跳过。

逐键测试：`TermuxPropertiesKeyContractTest` 逐键钉住合法取值必须生效、上游没有的
别名不接受（如 `terminal-cursor-style = beam`）、越界与非法值回退默认值并留痕；
`TermuxPropertiesKeysTest` 钉默认值与默认布局。

## 十一、检查 `docs/specification` 的修改并对照项目修复

状态：已修（本轮抽查）。

抽查结论：

- `BUILD.md`：`compileSdk 37`、`minSdk 33`、`targetSdk 28`、`versionCode 2000`、
  `versionName 0.1.0`、`JavaVersion.VERSION_17` 与 `android/app/build.gradle.kts` 逐项
  一致；Gradle 配置里无 `cargo build`／`cargo test`／`./gradlew`／`git clone` 字样。
- `TESTING.md`：全仓无 `#[ignore]`、无 `@Ignore`、无 `assumeTrue`/`assumeFalse`，
  「不存在跳过、不隐藏错误」成立；性能用例均有断言。
- `PROHIBITED.md`：「不解析 `colors.properties`、`termux.float.properties`」成立——
  全仓无这两条路径的读取点。
- `DESIGN.md`：响铃三态、shell 环境变量集合（`HOME`/`PREFIX`/`TMPDIR`/`LANG`/
  `COLORTERM`/`TERM`/`TERMUX_VERSION`）、侧边面板布局、启动即弹输入法均已落地。
