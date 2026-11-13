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

### Requirement: 共存构建

`nix` build type MUST 与 release 使用相同的构建配置，`applicationId` MUST 为
`com.termux.nix`，`app_name` MUST 为 `Nix`。MUST 隐藏 Termux 引导预设等不兼容功能；
自定义引导（URL 与离线 zip）MUST 仍然可用。文档提供器 authority 与快捷方式
`targetPackage` MUST 跟随实际 `applicationId`。

#### Scenario: 共存包与主包并存

- **WHEN** 同时安装 release 与 nix APK
- **THEN** 两者 applicationId 不同，各自使用独立数据目录，互不覆盖

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
