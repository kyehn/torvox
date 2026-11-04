# 平台日志噪声：jdwp 与 mali_gralloc

STATUS「未被解决的问题」列出「不能加载 jdwp agent 错误」与「mali_gralloc 错误」。
两条都来自**平台侧**（ART 与厂商图形栈），本文以静态取证说明应用代码既不是成因、
也没有可操作的抑制点；结论以用户设备上的完整 logcat 行为最终确认条件。

## `Failed to load jdwp agent`

**来源**：ART 调试子系统。JDWP（Java Debug Wire Protocol）agent 由运行时在
`ro.debuggable=1` 的系统上为「可被调试」场景加载；`adb jdwp`、Android Studio 断点
附加、userdebug ROM 的 shell 都会触发。agent 加载失败时 ART 打出该行——发生在
**进程外**的系统侧尝试，不属于任何应用的代码路径。

**应用侧取证**：

- 全仓（Kotlin / Rust / Manifest）对 `jdwp` 零引用（`grep -ri jdwp` 无命中）：
  应用不加载、不请求、不引用 JDWP。
- release 构建非可调试：`app/build.gradle.kts` release 块 `isMinifyEnabled = true`
  且未设 `debuggable`，AGP 默认 release `android:debuggable=false`——即使有外部
  附加尝试，ART 也只会拒绝并打日志。
- 应用无法抑制该行：JDWP 的加载决策在 ART 与 system_server，进程内没有任何
  API 可以关掉别人对调试通道的尝试。

**判定**：平台噪声，非应用缺陷。若用户设备 logcat 中该行与应用进程名成对出现且
伴随崩溃/卡顿，才需要升级为问题处理；孤立的日志行无功能影响。

## `mali_gralloc` 错误

**来源**：Mali GPU 设备的厂商 gralloc HAL（`mali_gralloc_buffer_allocator` 等）。
gralloc 负责图形缓冲区分配；HAL 在特定尺寸/格式/用途组合下会打出 error 级日志，
其中多数是**厂商实现的告警**（分配照样成功），少数才是真实分配失败（伴随黑屏或
渲染异常）。

**应用侧取证**：

- 表面格式取自 wgpu 适配器的首选：`pipeline.rs` 的 `surface_config.format` 是
  `Surface::get_capabilities` 列表中被适配器选中的格式（RGBA8/BGRA8 系），应用
  没有硬编码异形格式；离屏 KGP 纹理回退 `Rgba8Unorm` 是标准格式。
- 缓冲区的分配方式（stride、usage、YCbCr 布局）全部由系统合成器与 HAL 协商，
  应用经 `SurfaceView`/wgpu 只提交标准 usage（RENDER_ATTACHMENT）。
- CI 的 SwiftShader 模拟器上从未出现该类日志——它只存在于 Mali 硬件栈，
  佐证其来源是厂商 HAL 而非应用行为。

**判定**：厂商 HAL 日志，非应用缺陷。应用侧没有可修改的分配参数（改了反而偏离
wgpu 的标准路径）；若该行伴随真实渲染异常（黑屏、花屏），需要完整 logcat 行
（伴随的 status code 与尺寸）才能定位到具体分配场景——届时以
`SurfaceLossRecoveryInstrumentedTest` 同套取证方法在设备上复现。

## 状态

- 静态取证完成：应用代码对两者均无贡献路径。
- 待用户设备 logcat 原始行（`logcat_*.txt` 已被 `.gitignore` 排除，不入仓）：
  若两行均无伴随异常，按平台噪声结案；若有伴随异常，按上文升级路径处理。
