# 第 9 轮全面审查（FFI 契约与 Android 组件/资源生命周期）

审查日期：2026-09-30
方法：两个此前从未系统审过的面 —— (1) JNI/FFI 契约的机械索引与逐对核对，(2) Android 组件与资源生命周期。子代理静态交叉索引 `ffi.rs` ↔ `NativeBridge.kt`，并直接阅读 jni 0.22.4 源码确认 `jni_export_guard!` 的 panic 语义。关键结论逐条回读核实。
本轮只审查，**未改动任何源码**。

---

## 一、工具基线

第 8 轮已证明自动化工具全绿不构成证据，本轮未重复运行工具，全部预算投入人工读码。

---

## 二、新的 P0

### N0-12 `:install` 进程会执行完整的 `Application.onCreate`，重置启动崩溃循环计数器

`android/app/src/main/AndroidManifest.xml:11` 声明 `android:name=".TerminalApp"`，而 `:46-49` 声明：

```xml
<service
    android:name=".installer.BootstrapInstallService"
    android:exported="false"
    android:process=":install" />
```

`Application.onCreate` 在**每一个**进程里都会执行。于是 `android/app/src/main/java/terminal/emulator/TerminalApp.kt:65-68`：

```kotlin
monitorScope.launch {
    delay(HEALTHY_UPTIME_MS)
    BootGuard(stateDir).markHealthy()
}
```

会在安装进程里也跑一遍。`BootGuard.kt:43-48` 的 `markHealthy()` 把**按 UID 共享**的计数器（`stateDir = getDir("boot_state", MODE_PRIVATE)`）写回零。

**故障场景**：主进程在 10 分钟内崩溃 3 次（`BootGuard.kt:19-26` 据此禁用自杀保护）→ 用户触发一次 bootstrap 安装 → `:install` 进程把共享计数器清零 → 循环检测器从头开始，`DESIGN.md:237`（应用启动时检查兼容性并清除应用数据）**永远不会触发**。崩溃循环的兜底被一个从不运行终端的进程静默拆掉。

同一进程还继承了 `TerminalApp.kt:93-99` 的 `installThermalMonitor()`：

```kotlin
thermalMonitor = ThermalMonitor(this) { BootGuard.exit(stateDir, "Thermal CRITICAL+") }.also { it.register() }
```

而 `BootGuard.kt:75` 是 `Process.killProcess(Process.myPid())`。**设备处于 thermal CRITICAL 时的任何一次 bootstrap 安装，会在 `BootstrapInstallService.kt:67-88` 的 `runBlocking` 中途被杀，留下半解压的 `usr-staging/`。**

这两条都是 `DESIGN.md:237` 与「不干涉用户数据」的直接违反，且安装进程的存在本身就是 P2-20 记录的「测试后门进 release」的一部分。

**修法**：`Application` 本身无法按进程跳过，但可以在 `TerminalApp.onCreate` 里用进程名判定（`getProcessName()`）跳过安装进程的监控安装；更彻底的做法是把 `BootstrapInstallService` 移出 release 源集（它本来就只在 debug 下可达）。

---

## 三、新的 P1

### N1-14 `Bridge.getTitle()` 是唯一没有异常护栏的查询，而它的 Rust 侧会抛异常

`android/app/src/main/java/terminal/emulator/bridge/Bridge.kt:564`

```kotlin
override fun getTitle(): String? = queryPort.getTitle()
```

同文件 `:581-620` 的十四个兄弟方法全部包了 `runCatchingCancellable { … }.getOrNull()` / `.getOrDefault(…)`，**唯独这一个没有**。`NativeQueryPort.kt:13` 直接调 `NativeBridge.getTitle(sessionIdProvider())` —— 绕过了 `onSession` 的 catch。

而 Rust 侧确实会抛：`native/src/android/ffi.rs:2021-2027`

```rust
let Some(entry) = registry.get(&id) else {
    let _ = env.throw_new(jni_str!("java/lang/IllegalArgumentException"),
                          jni_str!("getTitle: session not found"));
    return Ok(std::ptr::null_mut());
};
```

`sessionIdProvider()` 每次调用都重新求值，所以「读取活动会话 id」与「实际 JNI 调用」之间发生会话关闭/切换，就会产生一个合法但无人处理的 `IllegalArgumentException`。

**可达路径**：`NativeQueryPort.kt:15` 的 `getActiveSessionTitle() = getTitle() ?: ""` → `TerminalRuntime.kt:3290 updateState()`（在 `sessionLock` 内）→ 被 `MainActivity.kt:274 onDestroy`（**主线程**）调用。同一个 `updateState` 还被 `:525 / :2800 / :3217 / :3136 / :3147` 调用。

若从组合中调用（会话抽屉标题），这是主线程进程终止。

这与第 8 轮 N1-8（14 个**无日志**的 `getOrNull`）不是一回事：那里是护栏存在但无日志，这里是**护栏根本不存在**。

**修法**：照兄弟方法包一层 `runCatchingCancellable { }.getOrNull()`。

### N1-15 `scrollbackLine` 对负行号抛异常，而该行号在正常滚动中就会出现

`native/src/android/ffi.rs:2127-2132`

```rust
let Ok(row) = u32::try_from(row) else {
    let _ = env.throw_new(jni_str!("java/lang/IllegalArgumentException"),
                          jni_str!("scrollbackLine: row must be non-negative"));
    return Ok(std::ptr::null_mut());
};
```

而 `ffi.rs:2137-2140` 的注释说明了 Kotlin 侧传的是什么：

```rust
// Kotlin 传入的是绝对行号（回滚 + 视口偏移，经
// `scrollbackLength - scrollOffset + row` 计算），故直接透传给期望绝对行号的
```

`scrollbackLength - scrollOffset + row` 在**视口被拖过缓冲区顶部**、或回滚缓冲区在滚动后缩短时会变成负数。此时 `Bridge.kt:581-585` 把 IAE 吞成 `null`，终端**静默不画那一行**。

用户表现：向上拖动时回滚区突然出现空白行，且**不能选中、不能复制**（`N1-8` 的第 1 条后果）。`DESIGN.md:29`（TESTING 覆盖范围「旧行必须按顺序进入回滚」）、`DESIGN.md:170`。

**修法**：负行号应返回 `null`（无数据），而不是抛异常 —— `NativeQueryPort.kt:9` 的契约「null/0/空表示引擎『无数据』，绝不可伪造」说的就是这个。

### N1-16 `EXTRA_OPEN_SETTINGS` 在 `onNewIntent` 中被静默丢弃

`android/app/src/main/java/terminal/emulator/MainActivity.kt:232-234`

```kotlin
if (intent.getBooleanExtra(EXTRA_OPEN_SETTINGS, false)) {
    launchOpenSettings = true
}
```

而 `:302`

```kotlin
var showSettings by remember { mutableStateOf(openSettingsOnLaunch) }
```

`remember` 只在首次组合时求值，而 `handleLaunchIntent` 运行在 `onNewIntent`（`:248`）里 —— 远晚于 `setContent`。

**故障场景**：Activity 已在栈中时，第二次启动带 `terminal.emulator.open_settings`（通知点击、快捷方式、Tasker 自动化）→ **打开的是终端而不是设置页**，无任何提示。

对照 `:230` 的 `runtime.requestFailsafeSession()`：它在每个 intent 上都被重新应用，`:209-212` 的注释也明确要求「必须在每个 intent 上重新应用」—— 同一要求没有在 `EXTRA_OPEN_SETTINGS` 上兑现。`launchOpenSettings` 本身是普通 `var` 而非 `State`，更不会触发重组。

### N1-17 `openDocumentThumbnail` 忽略 `sizeHint` 与 `CancellationSignal`，返回整个文件

`android/app/src/main/java/terminal/emulator/TerminalDocumentsProvider.kt:276-286` 声明了 `sizeHint: Point?` 与 `signal: CancellationSignal?` 并**两个都不用**，直接 `AssetFileDescriptor(parcelFileDescriptor, 0, file.length())` 把整个文件当缩略图返回。

`DocumentQueries.kt:73` 给**每一个** `image/*` 行都设了 `FLAG_SUPPORTS_THUMBNAIL`。

**故障场景**：系统文件选择器浏览一个含 40MB PNG 的目录 → 通过 Binder 传输的文件描述符把每个文件完整读一遍，无上限、无取消 → 选择器长时间无响应，可能触发 `AnrWatchDog`（第 7 轮 N11 路径）→ 杀进程丢会话。

同文件 `:232` 的 `openDocument(documentId, mode, signal)` 也从不调用 `signal.throwIfCanceled()` —— 已取消的 SAF 打开仍会完整执行。

违反 `DESIGN.md:238`（实现文档提供器，向系统文件选择器暴露用户文件）、`DESIGN.md:20`（低内存友好）。

### N1-18 写回通知绑定在主 Looper 且在主线程做磁盘 I/O

`TerminalDocumentsProvider.kt:159-161` 用 `Looper.getMainLooper()` 构造 handler，`:271-273` 把它交给 `ParcelFileDescriptor.open`；回调是 `mutations.notifyWritten(docId, file)` → `DocumentMutations.kt:304-307` → `notifyParentOf(written, rootDir())` → `DocumentQueries.kt:17-28` 的 `rootDir()`，其中含 `dir.mkdirs()` + `dir.isDirectory`。

**两个后果**：

1. 外部编辑器每关闭一个文件，主线程就做一次文件系统 I/O（debug 下 `TerminalApp.kt:31-40` 已武装 StrictMode `detectDiskWrites()`）。
2. 若进程在主 Looper 排空之前被切后台或被杀，**通知永久丢失** —— 选择器里的文件大小/mtime 保持陈旧，且没有任何重试路径。`Mutation.notifyWritten` 只由这一条路径可达，不存在兜底。

违反 `DESIGN.md:22`（减少兜底，尽早抛出错误）、`DESIGN.md:80`（日志可见但不写文件，隐含的进程生命周期假设）。

### N1-19 `setCursorColor` 文档声明的清除哨兵不存在，主题切换后光标色永久残留

`android/app/src/main/java/terminal/emulator/bridge/NativeBridge.kt:243`

```kotlin
/**
 * 应用层光标颜色覆盖，线性 RGB（每通道 0..1）；0xFFFFFFFF 哨兵值表示清除覆盖（跟随终端）。
 */
external fun setCursorColor(sessionId: Long, red: Float, green: Float, blue: Float)
```

而 `native/src/android/ffi.rs:2805-2812` 只收**三个 `f32`、没有哨兵**；`:2816` 无条件写 `render_state.cursor_color = Some([…])`，该字段仅在 `ffi.rs:191` 初始化为一次 `None`。

**结论：根本不存在清除路径。** 从一个带自定义光标色的主题切到不带的主题时，`Bridge.kt:395` 仍会用 `argbToRgbFloats(theme.cursor)` 调用 `setCursorColor`，旧覆盖**持续到进程结束**。

违反 `DESIGN.md:14`（「有 `a` `b` `c` 项」指有且只有，不可有未声明行为）。

---

## 四、新的 P2

| 编号 | 位置 | 问题 |
| --- | --- | --- |
| N2-15 | `ffi.rs:2261-2262` vs `:2275-2279` | `select_bounds_export` 的注释写「会话锁与注册表读锁都在构造界限后立即释放：选区查询走 VT 线程，绝不能持锁跨越」，而代码恰恰持着 `registry` 与 `session` 跨越 `select(...)` 这个 VT RPC，只在构造完界限**之后**才 drop。**注释与代码相反** —— 与第 6 轮 N0-1 同一模式。 |
| N2-17 | `DocumentQueries.kt:62-64` | `flags = flags or Document.FLAG_SUPPORTS_DELETE or Document.FLAG_SUPPORTS_WRITE` 是**无条件**的，目录也被标成可写。SAF 客户端会对文件夹显示「粘贴/编辑」，点进去得到系统的 `FileNotFoundException`。`FLAG_DIR_SUPPORTS_*` 存在的意义正是与此对应。 |
| N2-18 | `TerminalForegroundService.kt:121-136` | `startForeground` 抛异常后 catch 并继续，注释承认 `foregroundServiceRunning` 标志会陈旧并压制后续重启。Android 12+ 上这**阻止不了终止**：`startForegroundService` 要求 5 秒内 `startForeground`，否则系统抛 `ForegroundServiceDidNotStartInTimeException` 并杀进程 —— 该异常在应用侧**无法捕获**。即：一个无效的 Fallback（`DESIGN.md:24`）加上一个把重试彻底堵死的陈旧标志。 |
| N2-19 | `ffi.rs:2439-2446`、`ffi.rs:2066-2072` | `isCellEmpty` 与 `scrollbackLength` 在抛异常**之后**仍返回 `JNI_TRUE` / `0`。JNI 层面合法（值被忽略），但 Kotlin 侧签名是非空 `Int` / `Boolean`，于是 `NativeQueryPort.kt:9` 声明的「null/0/空表示无数据」通道对这两个方法是不可达的。契约自相矛盾。 |
| N2-20 | `ffi.rs:1245-1261` | `consumeNewOutput` 是 57 个导出中**唯一**没有套 `jni_export_guard!` 的（`selectWordAt`/`selectLineAt` 委托给了已守卫的 `select_bounds_export`）。函数体全用 `parking_lot`（无中毒），今天不存在 panic 路径，但 `ffi.rs:1250` 的注释声称「仍保持与相邻导出点一致的守卫写法」—— **该说法不成立**。 |
| N2-21 | `FontInfoDto.kt:28-31` | `FontInfoDto.fromJson` 复用了 `pollEventJson`，而后者带 `classDiscriminator = "event"` 与 `coerceInputValues = true`，是为 `PollEvent` 设计的。字体 DTO 的 schema 因此耦合在事件解析器的全局配置上；未来任何 `coerceInputValues`/`isLenient` 的改动都会静默改变字体信息解析，而 `catch (_: Exception) { null }` 会把破坏完全掩盖。 |
| N2-22 | `NativeQueryPort.kt:13-21` | `sessionIdProvider()` 每次调用重新求值，因此**一组**端口调用不是原子的 —— 第 N 次与第 N+1 次可能指向不同会话。全类没有线程约束、没有超时。 |
| N2-23 | `themes.xml:9-11, 24` | 硬编码 `#1E1E2E` 三次加 `:24` 一次，而同目录 `colors.xml:7-8` 的注释写着「spec：菜单样式 MUST 使用 Material 3 主题属性，MUST NOT 硬编码颜色」。`themes.xml:6-7` 声明的 `<attr name="colorSurface">` / `colorOnSurface` 在资源树里**没有任何东西读取** —— 四个字面量用法完全绕过了它们。这与第 6 轮 N9 是同一根因（无 `values-night/`），但这里多了「文件自己的注释被自己违反」这一层。 |
| N2-24 | `NativeBridge.kt:238-268` | 57 个 `external fun` 中 14 个是实例方法（缺 `@JvmStatic`），Rust 侧全部声明 `_class: JClass`。ABI 相同所以 JNI 短名解析能绑定（不会 `UnsatisfiedLinkError`），但 debug 下 CheckJNI 会报「bad argument … jobject instead of jclass」，且任何将来的 `RegisterNatives` 或工具链路径都会断裂。 |
| N2-25 | `event.rs:9` | `MAX_QUEUED_EVENTS = 1024` 的丢弃最老事件策略下，一次突发可以丢掉 `ClipboardRead` —— 而该事件对应的是一个**永不会被重新派发**的 VT RPC。 |

---

## 五、本轮的正面结论（同样重要）

不是所有东西都有问题。以下是本轮**核实为健康**的部分，记录下来以免后续轮次重复怀疑：

1. **JNI 符号表完全对称：57 个 Rust 导出 ↔ 57 个 Kotlin `external fun`，零缺失、零孤儿、零命名不符、零参数个数不符、零类型不符。**（机械索引 + 逐对手工核对。）
2. **`jni_export_guard!` 的 panic 语义是健全的。** jni 0.22.4 的 `with_env`（`src/env.rs:4801`）用 `catch_unwind(AssertUnwindSafe(..))` 包裹闭包，`EnvOutcome::resolve_inner`（`src/env.rs:4700-4731`）把 `Outcome::Panic` 路由到 `P::on_panic` 并转成 Java 异常 —— **没有任何 panic 能跨过 `extern "system"` 边界**。`AttachGuard::from_unowned` 不会 detach 调用方。
3. **无 JNI 全局引用泄漏。** 全仓 `NewGlobalRef` / `DeleteGlobalRef` 出现次数为 **0**，也没有手动 `push_local_frame` / `pop_local_frame`。
4. **全部字节数组线格式经长度校验，不可能让 Kotlin 解析器失步**：搜索高亮（`ffi.rs:2604-2650`）的前缀计数被钳到可用完整记录数（`:2611-2614`），逐记录有 `offset + 16 > payload.len()` 边界检查（`:2619`）；`setTheme` 在任何索引前做精确 54 字节检查（`:2733-2746`），所有 i32 用显式 `from_le_bytes`（与 Kotlin 打包端同为小端）；`renderWithNewOutput`（`ffi.rs:1890`）的位打包 `(new_output << 32) | (cursor_bits << 33) | (count & 0xFFFF_FFFF)` 与 `Bridge.kt:203-211` 的两个掩码解码**完全对应**（`cursor_bits` 在 `:1836` 已预掩到 `0xFFFF`），错误路径 `count = -1` 恰好产出 `toInt() == -1` / `new_output == 0` / `cursorRow == -1`。
5. **`PollEvent` ↔ Rust `Event` 的 schema 逐字段一致**（`event.rs:21-43` vs `PollEvent.kt:17-40`）：`event` tag、snake_case、`session_id`/`request_id`/`alive_ms` 名称、`u64`↔`Long`、`i32`↔`Int`。
6. **四处在生产代码里的 `unwrap`/`expect` 可证明不可达**：`ffi.rs:2625/2630/2635` 的 `.expect("4-byte slice")` 各自由 `:2619` 的守卫保护；`ffi.rs:1354` 是不变式。
7. **Manifest 与代码其余部分一致**：5 个声明的组件都存在为类；`exported` 标志正确（`MainActivity` true + LAUNCHER filter，其余 false，`TerminalDocumentsProvider` 带 `MANAGE_DOCUMENTS` 权限对）；5 个声明的权限全部被使用且无过度声明；`shortcuts.xml` 的 target/action/extra 与 `MainActivity.kt:54/227-231` 匹配。
8. **`TerminalForegroundService` 的 `onCreate` 通知渠道建立（`:62-75`）与 `onDestroy` wakelock 释放（`:160-163`）是对称且正确的。**
9. **`DocumentQueries.rootDir()` 的 `mkdirs` 与 `TerminalApp.monitorScope` 不可取消是可接受的**（分别是进程级与 Application 级生命周期）。

---

## 六、修复顺序（本轮增量）

1. **N0-12**（`:install` 进程跳过监控安装）—— 一处 `getProcessName()` 判定，同时修复崩溃循环计数被拆掉与 thermal 杀安装两条。
2. **N1-14**（`getTitle` 加护栏）—— 一行，与十四个兄弟方法保持一致，消除主线程进程终止路径。
3. **N1-15**（`scrollbackLine` 负行号返回 `null` 而非抛异常）—— 与 `NativeQueryPort.kt:9` 的既有契约对齐，删除一处自相矛盾。
4. **N1-16**（`EXTRA_OPEN_SETTINGS` 在 `onNewIntent` 生效）—— 把普通 `var` 改为 `MutableState` 或直接用 `state`。
5. **N1-19**（`setCursorColor` 实现文档声明的清除哨兵）—— 当前文档与实现互相矛盾，二者必须对齐。
6. **N1-17 / N1-18**（缩略图大小上限 + 取消检查、写回通知移出主 Looper）—— 两个都是 DocumentsProvider 的可扩展性问题。
7. **N2-15 / N2-16 / N2-20** —— 三处「注释/契约与代码相反」，其中 N2-16 与第 6 轮 N0-2 构成必现的长时锁占用。
8. **N2-18**（`startForeground` 的无效 Fallback）—— 与 P0-5 的删除建议绑定处理。
9. 其余 N2 项。

---

## 七、收敛状态

- 本轮**不是**「无新问题」的一轮。新增 **1 个 P0、6 个 P1、11 个 P2**，同时产出 9 组经核实的**正面结论**。
- 「连续四次无新问题」计数**第四次归零**，从第 9 轮重新开始。
- 四轮归零（6、7、8、9）的规律完全一致：**换一个审读维度就立刻在未被该维度覆盖的位置产出 P0**。第 6 轮换到上游进程/表面层，第 7 轮换到类别横切，第 8 轮换到 FFI 契约与组件生命周期 —— 每次都产出了新的 P0。
- 累计（第 6–9 轮）：**9 个 P0、28 个 P1、约 55 个 P2/P3**，外加 30+ 处死代码。
- **明确建议：停止审查，改为修复。** 理由：
  1. 缺陷密度高到任何抽样都会漏 P0 —— 四轮、四种维度、四次验证。
  2. 至少 8 个 P0 的修法是「几行改动消除一个永久错误状态」：`join` → `join_with_timeout`；`take_kitty_placements` 移出 `RENDER_STATE`；`Result(errors.isEmpty(), …)`；`flush()` 与输出通道移出 `pollEvent` 锁区；`DocumentQueries` 两侧同口径 canonical；复用已有的路径谓词；`getTitle` 加护栏；`:install` 跳过监控。
  3. 继续审查的边际收益已低于先把这些改掉 —— 且改完之后**必须**重跑四轮复审，那时「连续四次无新问题」才是一个有意义的验收标准（当前它衡量的是抽样运气，不是代码质量）。
- 本轮任务限定「不实际修改代码」，上述内容以文档形式留存，等待授权后按各轮第八节的顺序执行。
