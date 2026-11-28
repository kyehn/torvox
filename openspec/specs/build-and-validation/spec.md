# build-and-validation Specification

## Purpose

项目构建与验证入口。工具链由 Nix flake 声明，构建顺序、SDK 版本、架构与测试体系
以 `docs/specification/BUILD.md` 和 `docs/specification/TESTING.md` 为准。

## Requirements

### Requirement: 工具链来自声明式开发环境

开发与构建工具 MUST 由 `flake.nix` 声明，并通过 `nix develop` 与
`nix print-dev-env` 生成的环境进入。MUST NOT 使用 `apt` 等包管理器安装工具，
MUST NOT 使用 `which` 做运行时路径探测，也 MUST NOT 另行执行 Zig 版本检查。

#### Scenario: 进入声明式环境

- **WHEN** 开发者按 `BUILD.md` 创建并进入开发环境
- **THEN** 构建与测试所需工具均由该环境提供，不依赖系统临时安装

### Requirement: Android 构建顺序与参数

Android 原生库 MUST 使用 `cargo ndk` 构建，MUST NOT 使用 cargo 的 zig 交叉编译子命令。构建 MUST
先产出 `.so` 并填充 `jniLibs/`，再构建 APK。Gradle 的 `compileOptions` MUST 使用
`JavaVersion.VERSION_17`；`minSdkVersion` MUST 为 31、`compileSdkVersion` MUST 为 37、
`targetSdkVersion` MUST 为 28；`versionCode` MUST 为 2000、`versionName` MUST 为 0.1.0。
Rust 最低版本 MUST 为 1.99，`edition` 最低为 2024。

只支持 `arm64-v8a` 与 `x86_64` 架构，其中 `x86_64` 用于构建、测试与模拟器测试。
构建产物 MUST NOT 提交到仓库。

#### Scenario: APK 构建前原生库已就位

- **WHEN** 执行完整 Android 构建
- **THEN** APK 打包前 `jniLibs/` 已包含本次构建的 `.so`

#### Scenario: 只检查受支持的架构

- **WHEN** 审阅 Gradle 与 Rust 的 Android 构建配置
- **THEN** 目标架构只有 `arm64-v8a` 和 `x86_64`

### Requirement: release 构建以运行性能为取向

`native/Cargo.toml` 的 release profile MUST 以运行性能为取向：`opt-level` MUST 为 3，
MUST NOT 取体积取向的 `"z"`／`"s"`；MUST 保留 `lto = true`、`codegen-units = 1` 与
`strip = true`；`panic` MUST NOT 置为 `abort`（JNI 导出守卫的 panic 处理链依赖展开，
置 abort 会让守卫接住的 panic 直接终止进程）。

#### Scenario: 热路径不被体积取向拖慢

- **WHEN** 以 release profile 跑 `native/benches/` 的三条 criterion 基准
- **THEN** 实测耗时 MUST NOT 慢于下列上界：键入 94 µs、单元格实例构建 130 µs、CJK 解析 237 ns
- **AND** 同机 `opt-level = "z"` 的对应实测为 354–361 µs、265–271 µs、507–513 ns——
      体积取向至少慢一倍，性能取向 MUST NOT 退到该量级
      （`opt-level = 3` 的同机实测为 91–93 µs、128–130 µs、228–237 ns）

#### Scenario: panic 展开保留

- **WHEN** 检查 release profile
- **THEN** `panic` MUST NOT 为 `abort`，JNI 导出守卫接住的 panic MUST NOT 直接终止进程

#### Scenario: 体积代价已记账

- **WHEN** 比较两种取向的 `x86_64` 原生库体积
- **THEN** 取性能取向的体积增幅 MUST 被记录（本仓实测 9 471 952 → 11 199 512 字节，+18.2%）

#### Scenario: VT 核的 Zig 优化档随 profile 走

- **WHEN** 以 release profile 构建 `libghostty-vt-sys`
- **THEN** 其 Zig 优化档 MUST 为 `ReleaseFast`，MUST NOT 为 `ReleaseSmall`——
      上游构建脚本按 Cargo 的 `OPT_LEVEL` 选档（`s`／`z` → `ReleaseSmall`，其余 →
      `ReleaseFast`），故体积取向会连 VT 解析核一起降档

体积取向的取舍 MUST 用实测数字记录，而非凭体感决定：`docs/specification/BUILD.md`
要求「release 构建必须尽可能确保性能，正确设置构建参数」。

### Requirement: zig 崩溃按信号有界重试

原生库构建使用的 Zig MUST 经 `native/tools/zig-build` 包装。包装脚本 MUST 仅在子进程
被崩溃信号（SIGILL、SIGTRAP、SIGABRT、SIGBUS、SIGFPE、SIGSEGV）杀死时重试，最多 3 次；
编译错误、参数错误、普通失败、调用方取消（SIGHUP/SIGINT/SIGTERM）与被 SIGKILL
杀死（含 OOM）MUST 立即原样上抛。每次重试 MUST 把信号写到 stderr；连续 3 次仍崩溃时
MUST 带最后一次的状态失败，MUST NOT 静默降级或吞掉日志。

Zig 的包缓存 MUST 落在 `native/target/zig-global-cache`，使 CI 的 `native/target/`
缓存恢复后不再联网拉包；调用方显式设置同名环境变量时 MUST 以调用方为准。

#### Scenario: 冷缓存崩溃后重试成功

- **WHEN** `zig build` 在包拉取阶段被 SIGABRT 杀死一次，下一次成功
- **THEN** 构建继续，stderr 记录一次重试与信号号

#### Scenario: 编译错误不重试

- **WHEN** ghostty 构建返回普通编译错误
- **THEN** 包装脚本只运行一次并原样返回该退出码

#### Scenario: 外部终止不重试

- **WHEN** 子进程被 SIGKILL（如 OOM 杀手）或 SIGHUP/SIGINT/SIGTERM 终止
- **THEN** 包装脚本只运行一次并原样返回 128+N 的状态，MUST NOT 重试

#### Scenario: 连续崩溃有界失败

- **WHEN** `zig build` 连续 3 次被同一崩溃信号杀死
- **THEN** 包装脚本返回最后一次的状态，构建失败并保留日志

### Requirement: 共存构建

`nix` build type MUST 与 release 使用相同的构建配置，`applicationId` MUST 为
`com.termux.nix`，`app_name` MUST 为 `Nix`。MUST 隐藏 Termux 引导预设等不兼容功能；
MUST 提供 Nix-on-Droid 引导预设（按架构取地址，与 Termux 引导包不兼容，两套不可混用）；
自定义引导（URL 与离线 zip）MUST 仍然可用。文档提供器 authority 与快捷方式
`targetPackage` MUST 跟随实际 `applicationId`。

Nix 变种的引导包安装 MUST 有自动化验证：该变种是 release 构建，而仪器测试默认只跑
debug 变种，没有这条验证时「变种装不上引导包」无人可知。

#### Scenario: 共存包与主包并存

- **WHEN** 同时安装 release 与 nix APK
- **THEN** 两者 applicationId 不同，各自使用独立数据目录，互不覆盖

#### Scenario: Nix 变种装上自己的引导包

- **WHEN** 对 `com.termux.nix` 变种跑引导包安装用例
- **THEN** 用例断言装的是 Nix-on-Droid 引导包且安装成功，MUST NOT 静默跳过

### Requirement: 测试进入既有体系且断言具体结果

新测试 MUST 进入既有 `cargo test`、`testDebugUnitTest` 与
`connectedDebugAndroidTest` 等体系，MUST NOT 新造平行测试入口。每个测试 MUST 断言
具体行为与结果，MUST NOT 只断言非空、非零或“不崩溃”。MUST NOT 跳过、忽略或隐藏
错误；缺少 Vulkan、Rust 产物或其他必要条件时 MUST 失败并如实报告。

#### Scenario: 缺少 Vulkan 环境时失败

- **WHEN** Vulkan 测试环境不可用
- **THEN** 测试失败并报告环境缺失，MUST NOT 被跳过或标记为忽略

#### Scenario: Rust 产物缺失时失败

- **WHEN** Kotlin 测试需要的 Rust 产物不存在
- **THEN** 测试失败，MUST NOT 静默跳过

### Requirement: 针对性验证与性能基准

日常验证 MUST 先运行与改动直接相关的测试，MUST NOT 无差别运行全部测试。
`macrobenchmark` MUST 在模拟器上运行并对结果作断言；性能结论 MUST 来自设备或模拟器
实测，MUST NOT 以桌面测试或主观描述替代。

宏基准 MUST 覆盖下列场景，且每个场景 MUST 有可判定的断言（粗粒度预算或“确实发生了”
的判据），MUST NOT 只打印指标：

- 冷启动 / 热启动：终端界面必须真的出现，且启动到界面可用 MUST 在预算内。
- 修饰键点击反馈、输入法弹出、会话抽屉展开：动画 MUST 产生帧，交互 MUST 生效
  （如粘滞修饰态置起），耗时 MUST 在预算内。

#### Scenario: 修改单个能力后验证

- **WHEN** 某项改动只影响一个能力
- **THEN** 验证先运行该能力的针对性测试，再按风险补充必要检查

#### Scenario: 性能基准缺结果断言

- **WHEN** 性能基准只打印耗时而未断言阈值或趋势
- **THEN** 该基准不合格，必须补充可判定的结果断言

#### Scenario: 界面元素缺失时基准失败

- **WHEN** 被测界面元素没出现（启动未完成、控件被改掉）
- **THEN** 基准失败并指出缺失的元素，MUST NOT 继续测量一段无意义的画面

#### Scenario: criterion 趋势基准同样需要下限

- **WHEN** 开发期趋势基准（`native/benches/*.rs`，criterion）只输出统计区间
- **THEN** 它 MUST 在正式采样前跑一段带阈值的粗粒度吞吐采样，超预算即 panic；
      阈值 MUST 比实测低一个数量级以上，只兜数量级崩塌，不因机器差异判红
      （实现见 `native/benches/support.rs` 的 `Floor`）

### Requirement: 整轮仪器化必须独占设备

同一台设备上 MUST NOT 并发跑两轮仪器化。后启动的一轮在安装测试包时会以「设置调试应用」
名义强杀前一轮的被测进程，被杀的一轮遂报「进程崩溃」，崩溃点落在与被杀时刻无关的用例
上。

跑整轮仪器化前 MUST 确认设备上没有别的仪器化轮次在跑。崩溃归因 MUST 以 logcat 里
同刻的强杀记录为据：有强杀记录即判为并发跑造成，MUST NOT 判为被测代码缺陷。

#### Scenario: 并发仪器化被判为随机崩溃

- **WHEN** 同一设备上后启动一轮仪器化
- **THEN** 前一轮的被测进程 MUST 被框架强杀，该一轮 MUST 报「进程崩溃」
- **AND** 崩溃点 MUST NOT 被当作被测代码的缺陷

#### Scenario: 崩溃归因需要强杀记录佐证

- **WHEN** 整轮仪器化报「进程崩溃」
- **THEN** 归因 MUST 查 logcat 同刻是否出现「设置调试应用」的强杀记录
- **AND** 有强杀记录时 MUST 判为并发跑造成，MUST NOT 判为应用缺陷

### Requirement: Gradle 直接依赖锁更新与验证

`android/build.gradle.kts` 与子模块 `android/benchmark/build.gradle.kts`、
`android/baselineprofile/build.gradle.kts` 等声明的 Gradle 直接依赖版本 MUST 与
`dependencyUpdates` 审计发现的稳定里程碑版本对齐（稳定 + 里程碑；RC/dev 不做为默认更新基线）。
更新后 MUST 通过
`nix develop --command ./gradlew --no-daemon :app:compileDebugKotlin :app:compileDebugUnitTestKotlin`
验证为 BUILD SUCCESSFUL；提交说明记录行号与版本映射。

#### Scenario: 发现稳定里程碑更新

- **WHEN** dependencyUpdates 审计发现稳定里程碑行
- **THEN** 变更 MUST 更新声明版本并通过上述编译验证，提交说明记录映射

#### Scenario: 基线为 RC/dev 行

- **WHEN** 审计仅发现 RC/dev 版本行
- **THEN** 变更记录 MUST 说明保留理由（非安全默认），且不新增此类直接依赖

### Requirement: 编译平台可声明同级别次版本号

依赖的 AAR 元数据要求高于 37 的**同级别次版本号**（如 api 37.1）时，MUST 以次版本号
声明编译平台（如 `compileSdkMinor`），MUST NOT 因此抬高 API 级别；次版本号 MUST NOT
改变 `minSdkVersion` 与 `targetSdkVersion`，MUST NOT 改变应用可用的 API 面。次版本号
MUST 在构建脚本中写明触发原因。

#### Scenario: 依赖要求同级别次版本号

- **WHEN** 依赖的 AAR 元数据要求编译于高于 37 的次版本号（如 api 37.1）
- **THEN** 构建 MUST 以同级别的次版本号声明编译平台，MUST NOT 抬高 API 级别
- **AND** 构建脚本 MUST 写明触发该次版本号的依赖与原因

### Requirement: 构建期弃用警告必须归因到具体调用点

构建期出现的弃用警告 MUST 归因到具体插件与调用点并登记，不得笼统记为「第三方问题」，
也不得凭插件名猜来源。归因 MUST 由实测得出：调用方 MUST 在插件字节码中定位到被弃用
方法的真实调用者。消除该警告若需改动禁改区的构建脚本（插件版本声明），本项只登记、
不实施。

#### Scenario: 当前唯一的弃用警告及其调用方

- **WHEN** 以 Gradle 9.9.3 + AGP 9.6.0-alpha06 执行 `:app:compileDebugKotlin --warning-mode all`
- **THEN** 弃用警告 MUST 只有一条，即 Gradle 对 `Configuration.setVisible(boolean)` 的弃用
      （Gradle 11 移除）
- **AND** 调用方 MUST 登记为构建期插件：`com.ncorti.ktfmt.gradle` 0.28.0
      （`KtfmtPlugin`）、`dev.detekt` 2.0.0-alpha.6（`DetektBasePlugin`）与
      Kotlin Gradle Plugin 2.4.21（`ConfigurationsKt`）各调用一次
- **AND** 仓内 Gradle 脚本对 `setVisible` / `visible =` 的引用 MUST 为 0 处，AGP 的
      `gradle`、`builder`、`gradle-api` 三个 jar 对该方法的引用亦 MUST 为 0 处

#### Scenario: 禁改区内的插件升级只登记不实施

- **WHEN** 消除该警告需要改动禁改区的构建脚本（插件版本声明）
- **THEN** MUST 维持登记状态，未经维护者明确同意 MUST NOT 实施
