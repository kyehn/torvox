# AI Native 方案调研

调研日期：2026-11-28。目标是把本应用从「只能被用户手动操作的终端」变成「super agent 可直接调用的 Android 执行底座」：不堆 MCP 服务、不写几十个逐项接口、不 root、不 adb、尽量不依赖 Shizuku，允许 WASM 与热更新。

本文件只做技术选型与接口形状设计，不改代码。现有规范和约束仍以 `docs/specification/` 为准；其中 [PROHIBITED.md](PROHIBITED.md) 明确否决「终端任务的会话列表/新建/读取/操作会话」这类新增持久化状态，因此本文中的会话能力若要落地，需要先由用户明确修改该规范，或把能力限定为**进程内临时会话**、不引入跨进程持久化。

## 结论

1. **最省代码的运行时就是已有终端本身**。`feedPty` + `getTerminalText` + `waitForOutput` + `pollEvent` 已经是「执行 / 读取 / 等待 / 事件」完整闭环；agent 逻辑写成 shell 脚本丢进 PTY 即可，不用新嵌 Python、JS 或 JVM 脚本引擎。
2. **对外只开一个通用 IPC 入口**：新增一个 `exported` 的 `ContentProvider`，只实现 `call(method, arg, extras)`，把能力收敛成 `schema` / `call` / `events` 三个方法。不是 MCP，不写 per-tool 的 AIDL/Binder/HTTP 服务。
3. **底层能力直接复用现有 78 个 JNI 导出**：`NativeBridge.kt`（78 个 external fun）与 `Bridge.kt`（75 个 fun，其中 73 个是 `Bridge` 对象的成员）已覆盖 PTY、终端文本、选区、搜索、主题、字体、渲染、日志等。派发层只做「名字 → 现有函数」的一次转发，不再复制实现。
4. **脚本/热更新分层**：默认用 PTY 脚本；需要跨语言、可回放、可校验的插件时再上 `wasmi`；纯逻辑编排可用 `rhai`。**不上 Wasmtime**：官方把 Android target 列为 Tier 3（无 CI、无专职维护者，明确「not production ready」）。
5. **UI 交互与跨应用调用走 Android 平台 API**，不走 shell：Compose 状态 + `AlertDialog`、`Intent`/SAF、`NotificationCompat`。普通 app 的 shell 无法越过自身 UID 和签名权限边界，凡有 framework API 的能力都不靠 fork 命令实现。

## 硬边界（先看不能做什么）

| 需求 | 结论 | 依据 / 替代 |
| --- | --- | --- |
| root、adb、Shizuku | 全部排除 | 普通应用 UID 边界无法用自身 shell 越过；用 JNI/Kotlin 直调平台 API |
| termux 系列 API | 排除 | 本项目是自研终端，不是 Termux；不依赖 `termux-api`、`termux-am`、Termux 文件布局约定 |
| MCP 服务栈 | 排除 | 用户明确不要一堆 MCP；一个 `ContentProvider.call` 就够。Android `AppFunctions` 虽被官方称为 on-device MCP，但面向系统/合作伙伴、跨包调用有特权门槛，仅作设计参考，不接 |
| 现有 `.documents` provider | 不可复用 | `AndroidManifest.xml` 中它对 `MANAGE_DOCUMENTS` 有 read/write 权限；外部 agent 拿不到。新建独立 provider，不复用也不改它 |
| 会话持久化 | 当前规范否决 | [PROHIBITED.md](PROHIBITED.md) 禁止终端任务新增持久化状态；先改规范或限定为进程内临时会话 |

## 仓库里已经存在的能力（不需要重写）

| 能力 | 现成入口 | 位置 |
| --- | --- | --- |
| 会话 shell 执行 | `NativeBridge.feedPty(sessionId, bytes)` | `android/app/src/main/java/terminal/emulator/bridge/NativeBridge.kt` |
| 读取会话 | `getTerminalText` / `getVisibleText` / `scrollbackLine` | 同上 |
| 等待输出 | `waitForOutput(sessionId, lastGeneration, timeoutMillis)` | 同上 |
| 会话事件 | `pollEvent()`（serde JSON，含 clipboard/title/exit/pty 等） | 同上、`native/src/event.rs` |
| 无 Surface 建会话 | `initSession(rows, cols, shell, home, workingDirectory, prefix, scrollbackLines)` | `NativeBridge.kt`：spawn 与 Surface 无关，可后台建 headless 会话；推进状态用已有的 `pumpSessions()`，它专门解决「渲染暂停时无人取事件导致输出停滞」 |
| 会话列表 | `getSessionTitles(longArray)` / `getSessionPwds(longArray)` | `NativeBridge.kt`；`TerminalRuntime.state.sessionIds` 是 Kotlin 侧真源 |
| 搜索 / 选区 / 渲染 | `searchSetNeedle` / `selectAll` / `selectionText` / `setScrollOffset` / `setRenderPaused` 等 | `NativeBridge.kt`，共 78 个导出 |
| 主题 / 字体 / 调试 | `setTheme` / `getThemeCatalog` / `loadFontFile` / `getFontInfo` / `setLogLevel` / 两个注入钩子 | 同上 |
| UI 对话框 | `SettingsPickerDialog`、`SettingsClearDataSection` 的 `AlertDialog` 用法 | `android/app/src/main/java/terminal/emulator/ui/` |
| 跨应用 / 文件 | `Intent`、`ActivityResultContracts`、`TerminalDocumentsProvider` | 现有 Activity 与 provider |

**推论**：派发层几乎不用写业务代码。它只需要把 agent 的方法名映射到这些函数；调试能力也直接复用 `setSurfaceLossInjected`、`setDeviceLostInjected`、`setLogLevel`、`getFontInfo` 等已有钩子。

## 三入口接口（签名示意，不是实现）

```kotlin
// 新增的独立 provider，manifest 中 android:exported="true"，不设 readPermission/writePermission。
// ContentProvider.call(String, String, Bundle) 自 API 11 可用；客户端 ContentResolver.call 同。
class AgentProvider : ContentProvider() {
    override fun call(method: String, arg: String?, extras: Bundle?): Bundle?
}

// 单一派发器：只有一个白名单注册表，不为每个能力写 wrapper。
object AgentApi {
    fun schema(): String                       // JSON Schema：方法名 + 参数类型 + 返回值说明
    fun call(name: String, arguments: String): String
    fun events(cursor: String, limit: Int): String // cursor + 事件数组，取自现有 pollEvent
}
```

调用方（外部 super agent 所在 app，仅需几行）：

```kotlin
val uri = Uri.parse("content://<applicationId>.agent")
val reply = contentResolver.call(
    uri,
    "call",
    "session.write",
    bundleOf("arguments" to """{"sessionId":1,"bytes":"..."}"""),
)
```

### 方法分层

| 层 | 示例方法 | 落到哪里 |
| --- | --- | --- |
| 会话 | `session.list` / `session.create` / `session.read` / `session.write` / `session.wait` / `session.close` | `NativeBridge` 现有函数 + `TerminalRuntime`（进程内） |
| 调试 | `debug.logLevel` / `debug.fontInfo` / `debug.injectSurfaceLoss` / `debug.injectDeviceLoss` | 现有 JNI 钩子，零新逻辑 |
| UI | `ui.dialog`（标题、消息、按钮标签） | ViewModel 的单个 `AgentDialogState` + 一个 `AlertDialog` |
| 应用 | `app.openDocument` / `app.startActivity` | `ActivityResultContracts` / `Intent`，由 Activity 转发 |
| 热更新 | `hotload.script` / `hotload.wasm` / `hotload.dex` | 见下节 |

**建会话不走 `TerminalViewModel.createSession`**：它要求有效 `Surface`（Surface 无效时直接拒绝）。agent 无界面时应直接调 `NativeBridge.initSession`，得到 headless 会话，并周期性调用 `pumpSessions()` 推进 VT 与事件入队；需要显示时再由 UI 侧 `switchSession` 挂到 Surface。`session.close` 没有对应 JNI 导出，只能由 `TerminalRuntime` 在进程内完成。

## 技术选型与实际库

| 关注点 | 选择 | 理由 / 官方来源 |
| --- | --- | --- |
| 跨进程 IPC | Android `ContentProvider.call` + `ContentResolver.call` | API 11/29 两重载；框架对 `call` 不做逐项权限检查，正因为「不要安全」才用这个入口 [官方](https://developer.android.com/reference/android/content/ContentProvider#call(java.lang.String,%20java.lang.String,%20java.lang.String,%20android.os.Bundle)) |
| 参数/结果编码 | 复用已有 `kotlinx-serialization-json 1.13.0`（Gradle 已接入 serialization 插件） | 无需新增依赖；schema 与 `call` 用同一份方法描述生成，避免漂移 |
| agent 逻辑（默认） | 会话内脚本 + 已有 PTY | 零新增依赖；脚本语言取决于用户安装的 bootstrap 环境；热更就是覆盖脚本文件 |
| 结构化脚本（可选） | `rhai 1.27.0` | Rust 嵌入式脚本，`register_fn` 直接注册宿主函数，`serde`/`metadata` feature 可导出函数元数据 [官方](https://docs.rs/rhai/latest/rhai/) |
| 跨语言插件（可选） | `wasmi 2.0.3` | 纯解释型 Wasm VM，`Linker::func_wrap` 暴露 host function，无 JIT 平台适配负担 [官方](https://docs.rs/wasmi/latest/wasmi/) |
| 明确不选 | Wasmtime | `aarch64-linux-android` / `x86_64-linux-android` 为 Tier 3，缺 CI 与专职维护者，官方标注 not production ready [官方](https://docs.wasmtime.dev/stability-tiers.html#current-tier-status) |
| Kotlin 热更新（可选） | `InMemoryDexClassLoader` | API 26 起，明确「execute code that has not been written to the local file system」，避开可写目录执行代码 [官方](https://developer.android.com/reference/dalvik/system/InMemoryDexClassLoader) |
| APK 自升级（可选） | `Intent.ACTION_INSTALL_PACKAGE` + `FileProvider` URI | 普通应用路径，需用户确认安装；不 root、不 adb。不做静默安装 |
| UI 交互 | ViewModel `StateFlow` + 单个 `AlertDialog` + `Snackbar` | 现有 Compose/Material3 依赖已具备；一次状态变化驱动一种对话框，不为每个弹窗写代码 |
| 调用其他应用 | `Intent` / `ActivityResultContracts` / SAF | 普通应用可见性规则内工作；不靠命令行 wrapper |

## 热更新策略（按代价排序）

1. **脚本热更（默认）**：agent 把脚本文件写入应用私有目录或 `$PREFIX`，`session.write` 里执行；版本切换就是换文件，回滚就是换回旧文件。零新增框架。
2. **Wasm 模块热更（可选）**：`.wasm` 字节由 `hotload.wasm` 交给 `wasmi`，host function 只暴露 `AgentApi.call` 一个函数；模块内部组合调用，外层不需要为每个新能力重新打包。
3. **Kotlin 插件热更（可选）**：下载 dex 到内存，`InMemoryDexClassLoader(ByteBuffer, parent)` 加载实现了固定接口的类。适合必须直接调 Android framework 的场景。
4. **完整 APK 自升级（可选）**：走系统安装器并弹确认，不能静默。

> 注意：项目 `targetSdk = 28`，Android 10 起对可写应用目录的 W^X 限制主要针对 `targetSdk >= 29` 的应用。该豁免不可长期依赖，因此 dex/wasm 优先走内存加载，不把「从可写目录 exec」当设计前提。

## 落地顺序（每步都能独立验证）

1. 先写 `AgentProvider` + `AgentApi` 的 `schema/call/events` 与白名单注册表，只用现有 `NativeBridge` 函数；加单元测试验证 schema 与注册表一致、未知方法报错。
2. 接通会话四件事：`session.list/create/read/write`；`create` 走 `NativeBridge.initSession`，不改 `TerminalViewModel.createSession`。
3. 事件流接到 `pollEvent`，用 cursor 增量返回；不引入额外持久化。
4. 再加 `ui.dialog`、`app.startActivity`、`debug.*`；对话框只有一个通用 Compose 节点。
5. 只有在确实需要跨语言插件时才引入 `wasmi`；只有确实需要 Kotlin 插件时才引入 dex 加载。`Cargo.toml` 是受保护文件，新增 Rust 依赖前必须取得用户明确授权。

## 来源核实

- Android `ContentProvider.call`：`call(method, arg, extras)` 自 API 11；`call(authority, method, arg, extras)` 自 API 29；文档明确框架不做细粒度权限校验。
- Android `InMemoryDexClassLoader`：API 26 起；支持内存中的 DEX 字节；API 27/29 增加多 buffer 与 native library 搜索路径重载。
- Wasmtime Tier 表：`aarch64-linux-android`、`x86_64-linux-android` 在 Tier 3，缺失项为 CI 测试与专职维护者；Tier 3 明确 not production ready。
- docs.rs：`wasmi 2.0.3` 与 `rhai 1.27.0` 的 host function 注册方式确认；两者文档均未给出 Android 平台支持等级，需实机构建验证。
- 本仓库：`NativeBridge.kt` 78 个 JNI 导出、`Bridge.kt` 75 个 `fun`（73 个在 `Bridge` 对象内）、Gradle 已接入 `kotlinx-serialization-json 1.13.0`、`AndroidManifest.xml` 中 `.documents` provider 带 `MANAGE_DOCUMENTS` 权限，均为直接读取源码所得。

## 未验证 / 风险

- `wasmi`、`rhai` 在 Android NDK 交叉编译与包体增量未实测；文档未声明 Android 支持等级。
- 外部 agent app 与 provider 的可见性、后台启动限制、通知权限在真机上的行为未实测。
- APK 自升级在不同 OEM 上的安装器行为未实测。
- 本文不改任何受保护文件；会话能力的持久化口径仍需用户确认。
