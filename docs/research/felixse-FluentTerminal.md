# felixse/FluentTerminal 研究报告

## 仓库概况
- 克隆路径：`/tmp/opencode/ft`
- 技术栈：UWP / C# / XAML / TypeScript / WebView + xterm.js，MSBuild `.csproj`，.NET Standard 2.0 + .NET Core 3.1 测试栈
- 定位：Windows 桌面终端模拟器（WSL/PowerShell/CMD/SSH/Mosh）
- 状态：官方已声明不再维护，推荐 Windows Terminal
- 主要模块：`FluentTerminal.App`、`FluentTerminal.App.Services`、`FluentTerminal.App.ViewModels`、`FluentTerminal.RuntimeComponent`、`FluentTerminal.Client`（xterm 前端）、`FluentTerminal.Models`、`FluentTerminal.SystemTray`、`FluentTerminal.App.Services.Test`
- 主要依赖：xterm.js 4.18.0、xterm-addon-fit/search/serialize/unicode11/web-links、Newtonsoft.Json、RestSharp、PListNet、Serilog、Microsoft.Toolkit.Mvvm、Moq/AutoFixture/FluentAssertions/xunit
- 许可证：GPL-3.0

## 全仓库结构与关键代码
- `FluentTerminal.Client/src`：TypeScript 前端，webpack 产物供 WebView 加载。
- `FluentTerminal.RuntimeComponent/WebAllowedObjects/TerminalBridge.cs`：WebView 与 UWP 之间的事件桥，监听 `Output`/`Paste`/`SessionRestart`，接收输入、尺寸、标题、选区、右键/中键、错误上报。
- `FluentTerminal.App.Services`：服务契约与实现，含设置、主题解析（Fluent/ iTerm）、Shell Profile、更新、通知、托盘 IPC、默认值提供。
- `FluentTerminal.App.Services.Test`：仅服务层单测（约 10 个测试文件）， themes、设置、键盘命令、布局助手、托盘通信、默认值、对话框等。
- `FluentTerminal.App`：UWP UI/XAML、页面与 ViewModel 组合。

## 对比本项目
- 本项目为 Android Rust + wgpu/Vulkan + Kotlin/Compose，终端状态以 `libghostty-vt` 为唯一来源，会话与 PTY 归 Rust。
- FluentTerminal 将 VT 解析与渲染委托给 xterm.js，运行在 WebView 中；平台与渲染管线完全异构。
- UWP/WebView 架构不能直接嵌入 Rust/Android JNI 产物，不能作为库复用。
- 没有 Rust crate、Kotlin/Compose 依赖或 Android 原生渲染路径。

## 可直接使用的库/工具
- 无 Rust/Kotlin/Android 依赖可直接使用。
- `xterm` 及其 addons 仅能在 Web/WebView 场景复用；本仓已用 Ghostty VT + wgpu，不适用。
- 可直接借鉴其测试组织（xunit/Moq/AutoFixture/FluentAssertions）作为 .NET 服务工程组织参考，但本仓不使用 .NET。

## 可间接借鉴
- Shell Profile 数据模型（名称、命令、参数、图标、SSH/Mosh Profile）。
- 设置按页面拆分的交互组织（General/Terminal/KeyBinding/Profile/Theme）。
- iTerm 配色导入与 theme 解析（`ITermThemeParser`、`FluentTerminalThemeParser`、`ThemeParserFactory`）。
- Web UI 事件桥模式：`TerminalBridge` 可作为浏览器/WASM 终端 host 的桥接参考。
- Updater、ShellProfileMigration、ApplicationDataContainer 服务拆分方式。
- Tray process IPC（`TrayProcessCommunicationService`）与通知服务组织。
- iTerm `.itermcolors` 测试数据文件作为配色解析测试样例。

## 可稍微修改使用的代码/设计
- UI 层仅可参考 MVVM/XAML 与 ViewModel 分层思路，无法直接转 Compose。
- 配置模型与序列化可作字段设计参考，但需按本仓设置契约与 Android DataStore/Preference 契约重写。
- `TerminalBridge` 事件接口不能用于本仓 FFI 或 Kotlin 状态持有，只可作桥接模式参考。
- 主题解析器可作为 iTerm/Fluent 主题格式转换逻辑参考，但颜色来源与主题列表必须遵守本仓构建时生成。

## 测试数据/测试用例
- `FluentTerminal.App.Services.Test` 提供 .NET 服务层单元测试：DefaultValueProvider、DialogService、FluentTerminalThemeParser、ITermThemeParser、ThemeParserFactory、KeyboardCommandService、PaneLayoutHelper、SettingsService、TrayProcessCommunicationService。
- 测试栈为 xunit + Moq + AutoFixture + FluentAssertions。
- 含 iTerm colors 测试数据 `AdventureTime.itermcolors`，可作配色解析测试样例。
- 不提供 libghostty-vt 或 wgpu 渲染的终端 VT 测试数据，也无 Android instrumentation 测试。

## 设计模式
- MVVM：ViewModels 集中状态，View 为 XAML。
- Service 层封装持久化、SSH、更新、通知等功能。
- RuntimeComponent 作为 WebView 与原生之间的桥接层（`TerminalBridge`）。
- 设置按功能页面拆分；默认值由 `DefaultValueProvider` + settings service 提供。
- 迁移服务（`ShellProfileMigrationService`）处理旧配置升级。

## 其他结论
- 适合借鉴产品功能与设置布局，不适合作为本项目依赖或核心代码来源。
- 项目已停止维护，不应跟随其实现路径。
- 与本仓“终端状态单一来源”与“无 vendor”约定冲突，GPU 渲染能力不在本仓之外存在。
- iTerm 主题导入逻辑仅可作为试点参考，但自定义主题本仓禁止，主题列表固定且构建时读取。

## 四次审阅记录
- 准确性：已对照克隆仓库实际目录、csproj、RuntimeComponent 与测试栈，无虚构文件。
- 完整性：覆盖仓库概况、结构、对比、库/工具、间接借鉴、可修改代码、测试、模式、结论。
- 对比正确性：未把 xterm.js/WebView 架构描述为可直接复用；已明确与本仓 Rust/Compose/wgpu/Ghostty 边界冲突。
- 文档一致性：报告仅新增 Markdown，未修改受保护文件；与 docs/specification/BUILD 与 STYLE 不冲突（仅文档新增）。
