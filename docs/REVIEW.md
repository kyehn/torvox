# 全面审查报告

审查日期：2026-09-30
审查范围：`native/src`、`android/app/src`、`scripts/`、`flake.nix`、`openspec/`
方法：`npx aislop@latest scan` + `jscpd` + code-review-skill（五轴：正确性/可读性/架构/安全/性能）
基线：`check-rust.nu` 与 `check-gradle.nu` 全绿（fmt/clippy/machete/semgrep/test/doc/markdownlint/bench/detekt/lint）

本轮只审查，未改动代码。

> 维护注：P0-4（二段安装假成功）已修复并验证，对应小节删除；其余编号保持不变。

---

## 一、致命缺陷（P0）

### 1. `setTheme` 与 `render_inner` 锁序反转 → 硬死锁

- `native/src/android/ffi.rs:2747-2768`：`setTheme` 持 `SESSION_REGISTRY` 读锁 + `entry.session` 锁时调用 `render_state_mut()`（2761）。
- `native/src/android/ffi.rs:1573-1580`、`:1606-1613`：`render_inner` 阶段 3 持 `RENDER_STATE` 时再取 `rlock_session_registry()` + `session.lock()`。

```text
渲染线程: RENDER_STATE ──等──▶ session 锁
主题线程: registry读 → session锁 ──等──▶ RENDER_STATE
```

触发条件：终端画过任意 Kitty 图像（`kitty_generation != 0`）后切会话或网格变化。两线程互等 → JNI 全线冻结、永久 ANR。

同一文件 `setSelection`（`ffi.rs:2694-2695`）已显式 `drop(session); drop(registry);` 后再取 `render_state_mut()`——规则已知，`setTheme` 漏改。

**修法**：把 `{ render_state_mut() { … } }` 整体移到 guard 释放之后；并让 `render_inner` 在阶段 2（已持会话锁）一并采集 Kitty 所需数据，消除「RENDER_STATE → 会话锁」这条边。

### 2. `renderWithNewOutput` 持注册表读锁取 `RENDER_STATE` → 三方死锁

`ffi.rs:1876` 取 registry 读锁，直到 `:1889` 才析构，`:1883` 在其内取 `render_state_mut()`。

与 `render_inner`（`1522 → 1573`）构成 `RENDER_STATE → registry读`；与 `setScrollOffset`（`ffi.rs:3124` 取**写**锁）构成 parking_lot 读写公平性死锁：写者排队后新读者阻塞，形成三方循环等待。

**修法**：在 `:1882` 后提前 `drop(registry)`，或把 `:1883` 移到块外。

### 3. `acquire_texture` 回落路径把无限阻塞搬回渲染线程 → 永久 ANR

`render/pass.rs:154-159`：`try_send` 失败走 `texture_or_reconfigure`（`:116-130`），其中 `:121` 是**无超时**的 `surface.get_current_texture()`。

设计意图（`:138-139` 注释「绝不让渲染线程无限阻塞」）正是为规避 Mali-G57 的 `vkAcquireNextImageKHR` 永久卡死。工作线程只有 1 个且 `sync_channel(1)`：一旦它卡死，第 2 帧 `try_send` 仍成功但 `recv_timeout` 必超时（每帧 2s，永不恢复），第 3 帧起走内联分支 → 把工作线程要规避的无限阻塞原样搬回渲染线程。

**违反** DESIGN:24「不做任何未要求的 Fallback 机制」——注释与代码自相矛盾。

**修法**：`try_send` 失败与超时只记 error 并返回 `None`；检测「工作线程已取请求未应答」直接 `panic!`（合 DESIGN:16）。

### 5. 前台服务/看门狗/热管理整块功能无规范声明

`service/TerminalForegroundService.kt`（175 行）、`monitor/{AnrWatchDog,BootGuard,MemoryMonitor,ThermalMonitor}.kt`（494 行）、`AndroidManifest.xml:4-7` 的四个权限。

`grep -E '前台服务|通知|唤醒锁|看门狗|热' docs/specification openspec/specs` → **0 命中**。

**违反** STYLE:65「不允许实现任何未在 docs/specification/ 声明的功能/逻辑」＋STYLE:64。且这些自造策略会杀进程丢会话（`TerminalForegroundService.kt:149` 30 分钟 wakelock、`ThermalMonitor.kt:65-67` CRITICAL 杀进程、`AnrWatchDog.kt:84-91` 5s 无响应即 kill）。

**修法**：删除整块 + manifest 权限 + Runtime 中四处调用；或先补 openspec change 与 docs/specification 声明。

---

## 二、高危缺陷（P1）

### 6. CellData 帧丢弃后永不补发 → 最后几行输出永久不显示

`ghostty_terminal/internal.rs:1233-1234`：先 `*last_push = Some(data.clone())` 记账，再 `let _ = tx.try_send(data)`——失败即丢，**无 error、无重试**。通道容量 4（`types.rs:231`），渲染线程卡顿即触发。

此后 50ms 空转分支重跑 `refresh_cell_data`，因 `unchanged` 命中（`:1230-1232`）**提前 return，永不重发**。用户看到「最后几行永远不显示」。

**违反** DESIGN:24。

**修法**：`try_send` 失败时不更新 `last_push`（或记 pending 标志），并对 `Err` 记 error。

### 7. `resize` 不跳过空操作，且会话创建时立即 resize → 直接违反 DESIGN:242

`runtime/TerminalRuntime.kt:3029-3048`：`resize` 无 `(rows,cols)` 与当前网格比对；`switchSessionInternal`（`:2672-2675`）对刚 `spawnTerminal` 的新会话无条件 resize。

**违反** DESIGN:242 逐字要求：「`resize` 跳过空操作的调整大小。**不要在会话生成时重新执行调整大小的操作。**」且该条同时指出「mksh 会在收到 SIGWINCH 信号时清除提示符」——每次切换/退出回收都发一次可能同尺寸的 SIGWINCH。

**修法**：`resize` 首行比对 `getGridRowsColsPacked()`，相同直接 return；创建路径跳过该 resize。

### 8. `ResizeOutcome::Dropped` 被完全忽略

`ffi.rs:679`：`if let Err(e) = session.resize(...)`——`Ok(Dropped)` 既不抛异常也不记日志、不通知 Kotlin。PTY winsize 已变而 Ghostty 网格仍是旧尺寸，行折叠几何错乱，直到下次 resize 事件才自愈。

`session.rs:74-77` 精心定义了 `Applied/Dropped` 并在 `Dropped` 置 `grid_dirty` 以重试，调用方却丢弃该信号。

**修法**：`Dropped` 时 `throw_new` 或至少 `log::error!`。

### 9. `wait_exit_code` 超时上报 `0`，把崩溃伪装成正常退出

`ffi.rs:1065-1077`：超时返回 `0`，注释已自认「与正常退出无法区分」。DESIGN:192 要求崩溃保留现场（`[Process completed (code 255) - press Enter]`），而超时路径让上层按「正常退出」关闭会话；`mark_exit_reported` 已置位，事件不可重发 → 错误永久丢失。

**修法**：等待线程先写 `exit_code` 再写 `exited`（`session.rs:314-325` 已是此顺序，可延长等待），超时上报 `-1` 哨兵而非 `0`。

### 10. `REQUEST_REGISTRY` 条目泄漏

`ffi.rs:293-298` `register_request` 插入全局 HashMap，只有 `clipboardResult`（`:1976`）或会话消失（`:1192`）才移除。`wait_for_clipboard_answer` 2s 超时返回空串后线程结束，`tx` 永久滞留。每次 OSC 52 读请求泄漏一个 `Sender`＋两个 u64，`tail` 可稳定触发。

**修法**：应答线程结束无条件 `cancel_request`（幂等）。

### 11. `pty_write` 的 LF→CRLF 与 `>0xF7`→空格改写破坏二进制 VT 载荷

`ghostty_terminal/public_api.rs:160-165`：改写对**所有字节**生效，包括 DCS 载荷内部。Kitty 图形协议 `m=1`（直接 RGB，非 base64）载荷里 `0x0A` 与 `0xF8..0xFF` 都是合法数据，会被插入 `\r` 和替换成空格 → **图像静默损坏**。

同时属「篡改 Ghostty 输出流」，违反 DESIGN:58 单一状态源。

**修法**：LF→CRLF 交还内核（`configure_raw_mode_child` 已保留 OPOST/ONLCR，`pty.rs:659-661`）；`>0xF7` 防护若确有崩溃证据，应限于非 DCS/OSC 上下文，或直接崩溃而非静默替换。

### 12. 软换行搜索的匹配列号越过网格宽度

`internal.rs:2167-2202`：匹配完全落在软换行第 2+ 物理行时 `start_col ≥ cols`，而 `SearchMatch` 只有单个 `row`，渲染高亮定位不到单元格或完全不显示。同时 `insert_str(0, …)` 每次前插 O(len)，整体 O(n²)。

**违反** DESIGN:216-221「匹配到的单元格反色」「搜索可滚动显示的区域」。

**修法**：逻辑行内匹配后按物理行切分 `SearchMatch`，一次扫描完成。

### 13. `surfaceDestroyed` 声称的 UAF 不变式未接线

`ui/TerminalSurface.kt:2710-2713` 注释承诺「仅在渲染线程 join 之后才释放 Surface」，但代码只做 `currentSurface = null`；`pauseRendering()` 是异步投递。为该不变式专门写的 `TerminalRuntime.runAfterRenderThreadsStopped()`（`:3343-3349`）**全仓零调用点**——既是死代码，也证明不变式确实没接线。

**违反** STYLE:63，并在 GPU 挂起场景留下 UAF 风险。

### 14. `RenderWatchDog.stop()` 在持全局 `sessionLock` 时 `runBlocking`

`monitor/RenderWatchDog.kt:41-43`；`stopRenderThread()` 首行调用，`switchSessionInternal`（`:2487`）与 `startRenderThread`（`:1041`）都在 `synchronized(sessionLock)` 内调用。`sessionLock` 是所有会话操作的串行点 → 一次挂起把创建/切换/关闭全部拖到最长 2s。

**违反** DESIGN:20/22（性能优先、减少兜底）。

**修法**：`stop()` 改 `job.cancel()` + 异步 join，或锁外先取引用。

### 15. `Bridge.onSession` 把所有 native 异常转成缺省值

`bridge/Bridge.kt:98-107` catch `RuntimeException` → `onUnavailable`，覆盖 `writeToPty`→false（IME 提交静默丢弃）、`renderWithNewOutput`→`RenderResult(0,false,-1)`（会话丢失被当 idle）。

只有「id==0 / 会话刚销毁」这一种竞态可辩护（DESIGN:64-66），应与真实异常分开。

**违反** DESIGN:24。`:581-644` 的 `runCatchingCancellable{}.getOrNull/getOrDefault()` 同类。

### 16. `detectArchFromAbi` 把任何非 x86_64 静默当 aarch64

`FontUtils.kt:42-46`，被 `BootstrapOrchestrator.kt:119` 与 `SecondStageRunner.kt:210` 使用：32 位/riscv64 设备会下载 aarch64 引导 zip 并执行其 postinst，再被 P0-4 的 `success=true` 掩盖。BUILD.md 只列 arm64-v8a/x86_64。

**修法**：`else -> error(...)` 并让 `ensureBootstrap` 失败返回。

---

## 三、规范偏离与死代码（P2/P3）

### 17. 查询 API 固定 fallback 掩盖 VT 线程死亡

`ghostty_terminal/public_api.rs:445-460` + `types.rs:179-194`：`rows()→24`、`cols()→80`、`cursor_visible()→true`、`title()→""`、`dump_grid()→空网格`、`GridSnapshot::fallback()` 返回**全 true 的空白网格**（`internal.rs:1751/1758/1766`）。

同文件 `take_snapshot`（`:286-296`）却选 `panic!`——两套策略并存。`types.rs:2` 注释「运行时查询失败不致命，只回退为默认值」是**代码自述的、未在规范声明的** Fallback。

**修法**：统一为 `panic!`，删除 `DISCONNECTED_*` 与 `GridSnapshot::fallback`。

### 18. `session.rs` 内裸调 `unsafe libc::kill`

`session.rs:452-478` 的 `send_signal` 绕过同文件 `deliver_signal`（`:754-768` 已用 `nix::sys::signal::kill`）直接裸调。

**违反** AGENTS「核心终端数据路径禁用 unsafe」（`session.rs:244` 的 `poll` 已论证可保留）。

**修法**：改用 `nix::sys::signal::kill`，删除两处 `unsafe`。

### 19. 死代码清单（STYLE:63）

| 位置 | 问题 |
| --- | --- |
| `TerminalRuntime.kt:3343-3349` | `runAfterRenderThreadsStopped()` 全仓零调用 |
| `TerminalRuntime.kt:2813-2814` | `applySettings()` 的 `currentRows/currentCols` 计算后未使用 |
| `TerminalRuntime.kt:1936` | `else -> {}` 空分支 |
| `runtime/InputBatchBuffer.kt:136-141` | `reset()` 生产无调用方 |
| `ui/ModifierBar.kt:103-115` | 13 个 `ToolbarKey` 不在 `TERMUX_EXTRA_KEYS`，无布局编辑器（PROHIBITED:43-49），零引用 |
| `input/KeyboardMode.kt:9-30` | `Standard/Raw/Custom` + `ImeFlagSet` 生产不可达（`keyboardMode` 恒 `Secure`），连带 `TerminalScreen.kt:166` 恒真分支 |
| `ui/SearchDebouncer.kt:55-61` | `flush()` 仅测试调用，注释称「供输入法 Search 使用」但 `TextSearchBar` 不用 |
| `monitor/AnrWatchDog.kt:49-61` | `stop()` 自注释「当前无生产调用方」 |
| `monitor/ThermalMonitor.kt:47-55` | `unregister()` 零调用 |
| `TerminalViewModel.kt:799` | `private const val TAG` 未使用（日志全用字面量） |
| `MainActivity.kt:162-164` | `onInstallBootstrap` 的 `installContext` 形参未使用 |
| `MainActivity.kt:303,305` | `TerminalNavHost(viewModelReady = {})` 唯一调用点不传，`LaunchedEffect` 恒空跑 |
| `TerminalScreen.kt:557-570` + `TerminalSurface.kt:1299` | `onCopyRequested` 仅赋值无 invoke——用户长按 COPY 后无任何反馈 |
| `PollEvent.kt:37-40` → `Bridge.kt:355` → `TerminalRuntime` | BEL 振铃全链路接通但 Runtime **零消费** `poll.bell` |
| `TestUtils.kt:404-417` | 遍历 `TextureView` 的回退分支，而 `TerminalSurface.kt:2618-2620` 明确用 SurfaceView → 恒不命中的死代码 |

### 20. 测试后门常驻 release 包

`runtime/TestBackdoorReceivers.kt`（115 行、7 个接收器）在 release 里仍被实例化（`MainActivity.kt:75` 字段初始化），仅靠 `BuildConfig.DEBUG` 决定 register。`TerminalRuntime.kt:1854` 的 `test.minSurface` 与 `:1894` 的 `test.bootstrapUrl` 也在生产路径内。`installer/BootstrapInstallService.kt` + `AndroidManifest.xml:46-49`（`:install` 进程）完整进 release。

**违反** STYLE:65。**修法**：移入 `src/debug` 源集。

### 21. 设置数据错误处理不符 DESIGN

- `theme/TerminalTheme.kt:501` `byName(...) ?: draculaPlus` 静默替换未知主题名，既不报错也不清设置（DESIGN:16「设置数据错误 → 清除设置数据」）。
- `SettingsRepository.kt:93` 的 `SettingsState.fontSize = DEFAULT_FONT_SIZE(14f)` 与 `:108` 的 `deviceDefaultFontSize` 不一致，而 `:88` 注释声称「字段默认值与上方按字段流保持一致」——文档与代码相反。
- `TerminalViewModel.kt:679-683` `clearUnknownFontFamily` 只删 `font_family` 一个键，DESIGN:95 要求「重置应用数据」。

### 22. `ModifierBar.sendPlainOrModified` 静默吞按键

`ui/ModifierBar.kt:476-484` `encodeKeyEvent(...) ?: return`——CTRL/ALT 激活且编码器无映射时（Ctrl+PAGE_UP/END/HOME）按键完全无反馈。与 `Bridge.kt:536` 有意丢弃 Ctrl+9/0 的注释不同，这是无声明的静默丢弃（DESIGN:24）。

### 23. 空 shell 路径静默回退

`ffi.rs:398-401` 空 shell 回退 `/system/bin/sh`。DESIGN:188 的三段探测由 Kotlin 侧完成（`TerminalRuntime.kt:1682` 已实现），Rust 侧再兜一层把「入口失败不得 Fallback」（DESIGN:194）变成静默兜底，且掩盖 Kotlin 解析 bug。

---

## 四、工具报告

### aislop scan（0.16.1，rust，40 文件）

| 类别 | 结果 |
| --- | --- |
| Code Quality | 10 warnings：8 个文件超 1000 行、2 个函数超 120 行 |
| AI Slop | 2 warnings（`cell_builder.rs:778`、`render/tests.rs:761` 叙述式注释块） |
| Formatting / Security / Linting | 0 issues |

超大文件：`android/ffi.rs` 3295 行、`render/cell_builder.rs` 1840、`render/font/mod.rs` 1909。
超长函数：`ffi.rs:1431 render_inner` 390 行、`internal.rs:472 run_inner` 457 行。

违反 STYLE:68「按功能/作用/目的放置代码」——`ffi.rs` 把 60+ JNI 导出、全局状态、渲染编排混在一起。

### jscpd（5.3.3）

总计 71 clones / 1691 行（2.43%）。

| 格式 | 克隆 | 重复率 | 说明 |
| --- | --- | --- | --- |
| markdown | 23 | 21.43% | 全部为 `openspec/changes/archive/**/spec.md` ↔ `openspec/specs/**/spec.md` 的归档副本（最大：text-selection 162 行逐字相同） |
| yaml | 2 | 6.25% | `.github/workflows/{build,check,fmt}.yml` 的双 checkout + install-nix + cache 段（24/22 行） |
| rust | 27 | 1.64% | 多数在 `#[cfg(test)]` 脚手架（`render/tests.rs` 14 处）；生产仅 3 处：`cell_builder.rs:1314↔1380`(27L)、`cell_builder.rs:1583↔1641`(13L)、`font/mod.rs:1197↔1254`(16L) |
| kotlin | 19 | 0.92% | 全在 `androidTest` 脚手架：等 TerminalScreen 样板在 5 个 cucumber step 重复、@get:Rule 三件套在 ≥7 个类重复、diag 探针重复 |

生产 rust 的 3 处重复已核实为 `#[cfg(test)]` 内的夹具样板（`mod tests` 起于 `cell_builder.rs:948`、`font/mod.rs:176`），非真实重复。

---

## 五、构建与依赖

### BUILD.md 违反/缺失

| # | 位置 | 问题 |
| --- | --- | --- |
| B1 | `scripts/build-android-libs.nu:42` | `cargo ndk --platform 21` 与 `minSdk = 33`（`app/build.gradle.kts:41`）矛盾，产物 API level 低 12 级 |
| B2 | `scripts/build-android-libs.nu:47-57` | 缺 `NEEDED libghostty-vt.so` 校验（BUILD:15）。当前实测 NEEDED 仅系统库（静态链接），属**潜伏风险**：上游 rev 一旦改动态链接即静默产出无法加载的 APK |
| B3 | `scripts/build-apk.nu:23-27,32-36` | 缺「APK 至少含一个 `.so`」校验（BUILD:16） |
| B4 | `scripts/build-android-libs.nu:47-57` | 缺 `.so` 体积检查（BUILD:17）。实测 dev `.so` 26–27 MB、release 7.5–8.7 MB，dev 根本不进 APK 却仍产出 |
| B5 | `scripts/setup-emulator.nu:65,69` | 用 `sdkmanager --install` 装 system-image/emulator，违反 BUILD:6 |
| B6 | `scripts/build-android-libs.nu:37-38` | 两次宿主 `cargo build` 产物完全未被使用 |
| B7 | `flake.nix:91`（`zig_0_16`）、`:77`（`gradle`） | 未被任何脚本使用的 devShell 依赖，违反 AGENTS「不得保留未使用依赖」。注意 BUILD:9 声明「Zig 版本以 flake.nix 为准」与 BUILD:13 禁 `cargo zigbuild` 本身有张力，删除前需确认 |

### STYLE.md 违反（保护文件，只报告）

| # | 位置 | 问题 |
| --- | --- | --- |
| S1 | `scripts/setup-emulator.nu` 全文 | 8 个脚本全 kebab-case（STYLE:8 要求 snake_case）；且该脚本全仓零引用＝死代码（STYLE:63） |
| S2 | `scripts/setup-emulator.nu:3,59-60,88` | 硬编码 `/usr/local/lib/android/sdk` 并拼路径调 `sdkmanager/avdmanager/emulator`（STYLE:30 要求直接用命令）。附带：该路径是 GitHub runner 路径，在 `nix develop` 下不存在 |
| S3 | `scripts/setup-emulator.nu:29` | `let start = (date now)`（STYLE:29 逐字点名） |
| S4 | `scripts/setup-emulator.nu:16-20` | `else { print }` 回退分支（STYLE:20） |
| S5 | `scripts/build-android-libs.nu:41` | `each { \|a\| … }` 单字母变量（STYLE:57） |
| S6 | `flake.nix:143-144` | `nu scripts/download-*.nu`（STYLE:31 要求 `./scripts/xxx.nu`） |
| S7 | `android/app/build.gradle.kts:91` | `jniLibs.directories.add("src/main/jniLibs")` 已是 AGP 默认目录＝死代码 |
| S8 | `.semgrep/kotlin-deny-patterns.yml:120-139` | 针对 `**/*.gradle.kts` 的规则放在 kotlin 文件（STYLE:68）；`:34-35` 与 `android-deny-patterns.yml:59-60` 的 `fix:` 写入未绑定的 `$SCOPE`/`delay($MILLIS)`，`--fix` 会产出不可编译代码 |
| S9 | `detekt.yml:7-72` | 抑制范围超 STYLE:37 允许（仅参数数量/行数/嵌套/缺失文档），实际关了 CognitiveComplex/Cyclomatic/LongMethod/MagicNumber/MaxLineLength/ReturnCount/ThrowsCount/UnusedParameter 等 |
| S10 | `Cargo.toml:48-50` | `type_complexity = "allow"` 不在 STYLE:37 允许清单（仅 `too_many_arguments` 算参数数量） |

### 依赖过期（违反 BUILD:23 / DESIGN:5）

Rust：`fontdb = "0.23"` 锁 `0.23.0`，最新稳定 **0.24.0**。其余 29 个直接依赖均已是最新稳定。

Gradle（6 处「有更高稳定版却用预发布」）：

| 位置 | 当前 | 最新稳定 |
| --- | --- | --- |
| `android/build.gradle.kts:5` | `dokka 2.3.0-Beta` | **2.2.0** |
| `app/build.gradle.kts:133-135` | `lifecycle-* 2.12.0-alpha04`（×3） | **2.11.0** |
| `app/build.gradle.kts:136` | `activity-compose 1.14.0-alpha03` | **1.13.0** |
| `app/build.gradle.kts:148` | `datastore-preferences 1.3.0-alpha11` | **1.2.1** |
| `app/build.gradle.kts:161` | `kotlinx-serialization-json 1.12.0-RC` | **1.11.0** |
| `android/build.gradle.kts:2-4` | `AGP 9.5.0-alpha07` | 9.4.1（边界情况，见待确认） |

合规的预发布：`detekt 2.0.0-alpha.6`（稳定线仍 1.23.8）、`leakcanary 3.0-alpha-9`（最高预发布）。

结构问题：无 `gradle/libs.versions.toml` 版本目录；`android/benchmark/build.gradle.kts` 与 `android/baselineprofile/build.gradle.kts` 是逐字重复的两份文件（仅 `namespace` 不同）。

### CI 工作流

| # | 位置 | 问题 |
| --- | --- | --- |
| E1 | `.github/workflows/build.yml:45-48,70-74` | **发布链路断裂（高危）**：`build-apk.nu:6-13` 每次删除**两个**变体的 APK，`:45-48` 以 `--debug` 再调一次删掉已构建的 release APK，`:70-74` 上传时文件已不存在 → 打 tag 产出空 release |
| E2 | `fmt.yml:44` | `bash -c "pushd …"`（STYLE:5 禁 bash/sh） |
| E3 | `build.yml:67-69` | `nu <path>` 而非 `./scripts/xxx.nu`（STYLE:31 意图未落实） |
| E4 | `build.yml:2-5`、`check.yml:2-5` | 仅 `schedule` + `workflow_dispatch`，**无 push/PR 触发** → PR 合并前无质量门 |
| E5 | `fmt.yml:2-3` | 仅 `workflow_dispatch`，格式化从未自动执行 |
| E6 | 三个 workflow `:35` | cache key 用 `${{ github.run_id }}` → 每次必变，缓存**永不命中** |
| E7 | `fmt.yml:47` | `git commit --amend` 改写 committer（STYLE:77 禁止其他提交者） |
| E8 | `fmt.yml:39` | `rm -f ~/.config/nix/nix.conf` 破坏性副作用 |
| E9 | `fmt.yml:44` | 只做 check 不做 format |

---

## 六、规范缺口

### openspec/specs 缺失的 DESIGN 声明功能

Bootstrap 安装（DESIGN:126-142，含环境变量白名单、原子化替换、nix-on-droid 兼容）是项目最大声明功能，`openspec/specs/` 下**零 spec**。其余缺失：字体大小调节条（:90）、实际字体信息框（:104）、软件/终端主题（:106,108）、修饰键栏布局（:112-118）、清除应用数据按钮（:144）、脏跟踪（:150）、kitty 图像协议（:180）、鼠标（:182）、回滚 2K（:196）、侧边面板其余条目（:229-233）。

`PROHIBITED.md` 全部禁止项均无「不得实现」的可验证 spec。

### 反向缺口（openspec 有、docs/specification 无）

`backspace-input-wake/spec.md:32,43`（空闲回落策略）、`ime-animation-smoothness/spec.md:31,49`（insets 读取隔离）、`render-stability/spec.md:9`（字形首帧幂等）均为实现级不变量，`DESIGN.md` 无对应描述。

### 活动 change 未同步

`openspec/changes/2026-09-28-render-idle-cursor/` 的 delta 尚未进入 `openspec/specs/ime-animation-smoothness/spec.md`，`tasks.md:11-17` 已把 `ImePopupPixelInstrumentedTest` 标为受阻未解。

---

## 七、需你决策的规范问题

1. **BUILD:6 禁 sdkmanager 装工具 vs `scripts/setup-emulator.nu` 依赖它**：该脚本全仓零引用（死代码）。删除它，还是把组件安装移入 `flake.nix`？
2. **BUILD:9「Zig 版本以 flake.nix 为准」vs BUILD:13 禁 `cargo zigbuild`**：`zig_0_16` 实际无人使用（`zigbuild` 被 semgrep 禁止），删除是否与 BUILD:9 冲突？
3. **PROHIBITED:19「内嵌 bootstrap，预装发行版」vs DESIGN:126-142 完整下载式安装声明**：现有实现是下载式（非预置），按 DESIGN 合规，但两文档字面矛盾，建议澄清。
4. **P0-5 前台服务/看门狗/热管理**：删除（合 STYLE:65），还是补规范声明后保留？
5. **归档重复 21.43%**：AGENTS 要求归档不得删除。推荐在 jscpd 配置中 `ignore` 掉 `openspec/changes/archive/**`（同时消除归档副本与主 spec 漂移隐患），而非改变归档语义。是否接受？
6. **`AGP 9.5.0-alpha07` 是否降到 9.4.1**：按 BUILD:23「有更高稳定版则用稳定」字面应降级；但 9.5 是最高预发布，属边界情况。
7. **P1-11 LF→CRLF 改写**：删除后 PTY 输出的换行依赖内核 `ONLCR`（`pty.rs:659-661` 已保留）。是否有实测证据表明必须保留此改写？

---

## 八、建议修复顺序

P0-1（死锁）→ P0-2（死锁）→ P0-4（安装假成功）→ P1-6/7/8（数据与状态不一致）→ P0-3（ANR）→ P0-5（规范缺口）→ P1 剩余 → P2/P3 → E1（CI 发布）→ B1–B7（构建校验）→ 依赖降级 → S 组（保护文件，需授权）。

---

## 九、循环复审记录

连续三轮扫描（`aislop` + `jscpd` + code-review-skill），逐轮降低 jscpd 阈值并核实每项发现。

| 轮次 | jscpd 阈值 | clones | 重复率 | aislop | 新增问题 |
| --- | --- | --- | --- | --- | --- |
| 1 | 12 行 / 60 词 | 71 | 2.43% | 12 warnings | 初始清单（本报告一至六节） |
| 2 | 12 行 / 60 词 | 71 | 2.41% | 12 warnings（一致） | 3 项（见 N1–N3） |
| 3 | 10 行 / 50 词（排除归档） | 83 | 1.80% | — | 3 项（见 N4–N6） |

第 2、3 轮 aislop 与 jscpd 数值与第 1 轮一致（差异仅来自新增本文档），确认无回归。

### 第 2 轮新增（经核实修正了首轮判断）

#### N1. `build_snapshot` / `GridSnapshot.dirty` / `cached_snapshot` 整条链零生产消费者

首轮判为「脏跟踪形同虚设」（P3-11），本轮核实**更严重**：`take_snapshot`（`ghostty_terminal/public_api.rs:284`）的全部调用方都在 `test_helpers.rs` 与 `tests.rs`，`Command::TakeSnapshot`（`internal.rs:856`）唯一发送点是 `take_snapshot` 自身。`ffi.rs` 与渲染路径都不使用快照——`build_snapshot`（`internal.rs:1736`）、`GridSnapshot::fallback`、`dirty` 字段（`types.rs:141/185`）、`cached_snapshot`（`internal.rs:649`）全部只服务测试。

同时 DESIGN:150「脏跟踪，跳过干净快照」在**生产路径**由 `build_cell_data` 的 `row_cache` 实现（`internal.rs:1477-1484`），快照路径是遗留的第二套机制。

违反 STYLE:63「不得保留死代码」＋STYLE:64「必须最小实现」。

**修法**：删除 `take_snapshot`、`build_snapshot`、`Command::TakeSnapshot`、`GridSnapshot`、`cached_snapshot`、`GridSnapshot::fallback` 及其 `DISCONNECTED_*` 常量组；测试改用 `dump_grid` 或直接断言 `build_cell_data`。这同时消除首轮 P2-17 的一半。

#### N2. 软换行搜索的列号错位（首轮 P1-12 成立并强化）

`internal.rs:2167-2202`：`logical_row` 记录拼接后**最早**的物理行，而 `start_col`/`end_col` 来自拼接后**逻辑行**（`search_line_columns(&search_line, ...)`）。匹配落在续接段时，返回的列号属于逻辑行但 `row` 属于首行 → `cell_highlight` 定位到错误单元格。

**违反** DESIGN:216-221。

**修法**：逻辑行匹配后按物理行切分 `SearchMatch`（`row + i` + 列偏移），一次扫描完成；同时消除 `insert_str(0, …)` 的 O(n²) 前插。

#### N3. `InputBatchBuffer.close()` 并非缺陷（首轮判断过重，已撤回）

首轮 P1-10 曾列为「无重开路径导致输入被静默吞掉」。本轮读 `InputBatchBuffer.kt:100-119` 原文确认：注释明确「应在宿主视图的 detach 路径调用」「shutdown 与入队竞争；在 detach 时丢弃可接受（视图已不存在）」——这是**文档化的 detach 语义**，非缺陷。**从清单移除。**

### 第 3 轮新增（降低阈值后暴露的生产重复）

#### N4. `ffi.rs:3060 ↔ :3076` — `getCellWidth` / `getCellHeight` JNI 样板重复（11 行）

两者除末尾 `.cell_metrics().0` / `.1` 外完全相同：`jni_export_guard!` + `render_state_mut()` + `let Some(...) else { return Ok(0.0) }`。

**修法**：提取 `fn cell_metric(state, pick: impl Fn(&FontPipeline) -> f32) -> f32`，两处各传闭包。

#### N5. `ffi.rs:2029 ↔ :2154` — 「取 registry + session + 抛异常」样板重复（12 行）

`getTitle` 与 `getTerminalText` 的前 12 行同构：`rlock_session_registry()` → `registry.get(&id)` → `throw_new(IllegalArgumentException, "<name>: session not found")` → `entry.session.lock()`。

该模式在 `ffi.rs` 中出现约 20 次（`getTitle`/`getTerminalText`/`selectionText`/`scrollbackLine`/`hyperlinkAt`/`selectWordAt` 等）。

**修法**：提取 `fn with_session<T>(env, id, name, f: impl FnOnce(&Session) -> T) -> Option<T>`，统一「查会话 + 抛异常 + 锁」。

#### N6. `render/font/rasterization.rs:27 ↔ :55` — `scaled_metric` 与 `cell_metrics` 同构（11 行）

两者都是 `font_id` → `db.with_face_data` → `FontRef::from_index` → `metrics(&[])` → `upem == 0` 早退 → 按 `font_size / upem` 缩放。

**修法**：提取 `fn face_metrics<R>(&self, f: impl FnOnce(swash::Metrics, f32) -> Option<R>) -> Option<R>` 统一缩放逻辑。

### 第 3 轮排除项

`event.rs:127 ↔ :173`（13 行）在 `#[cfg(test)]` 内为测试夹具；`cell_builder.rs:1314↔1380`、`1583↔1641`、`font/mod.rs:1197↔1254` 经核实同样位于 `mod tests`（分别起于 `cell_builder.rs:948`、`font/mod.rs:176`），非生产重复。

### 连续三轮无新问题的判定

第 2、3 轮相对第 1 轮**未发现新的 P0/P1 缺陷**，仅在第 3 轮降低阈值后暴露 3 项生产代码重复（N4–N6）与 1 项死代码链（N1）。三轮工具数值一致，无回归。

---

## 十、第 4 轮：Manifest / 资源 / ProGuard / CI / semgrep

前三轮覆盖 native Rust、Kotlin main、scripts、gradle、openspec、CI。本轮转向 **Manifest、res 资源、ProGuard、semgrep 规则本身、活跃 change 状态**。

工具复核：`aislop` 12 warnings（与前三轮逐字一致）；`jscpd` 83 clones / 1.82%（与第 3 轮一致，排除 `docs/**` 后数值差 0.02% 来自文档本身）。

### 确认升级的两项

#### N7. 发布链路断裂追加两处断点

首轮 E1 已记录 `build-apk.nu:6-13` 删除两变体 APK。本轮核实追加：

- `.github/workflows/build.yml:73` `generate_release_notes: false` 且未给 `tag_name`/`name`/`body` → 即使 APK 修好，Release 仍是无标题、无说明的空发布。
- `build.yml:2-5` 只有 `schedule` + `workflow_dispatch`，**无 `push: tags` 触发器** → `:70` 的 `startsWith(github.ref, 'refs/tags/')` 在常规 `git push --tags` 下永不为真，该分支实际不可达。

#### N8. release 变体在 CI 中零冒烟，且 R8 配置自相矛盾

- `app/build.gradle.kts:62-63` release 开 `isMinifyEnabled` + `isShrinkResources`；`build.yml:48` 只构建 debug，`test-emulator.nu:8` 只跑 `connectedDebugAndroidTest` → **被发布的 release APK 全流程零验证**。
- `proguard-rules.pro:1-2` 的 `-dontoptimize` / `-dontobfuscate` 立刻把 `:66` 引入的 `proguard-android-optimize.txt`（优化/合并/内联）全部关掉；`:3` 的 `-keep class terminal.emulator.** { *; }` 又让全部应用类不可裁剪。

净效果：release 变体 = 保留全部符号名 + 不优化 + 应用代码零裁剪，与 `isShrinkResources = true` 的意图完全相反。

**当前无运行时缺陷**（`-keep` 覆盖了 `@Serializable` 的 `FontInfoDto`/`PollEvent`），但正确性完全依赖这一条兜底：任何「去掉 `-dontobfuscate`」或「启用 optimize」的改动都会让 `FontInfoDto.fromJson()` 在 release 上静默抛 `SerializationException`，而 CI 只测 debug **无法发现**。

**违反** STYLE:64「必须最小实现」——`isMinifyEnabled = true` 当前不产生任何收益，只增加构建时长与不确定性。

### 第 4 轮新增

#### N9. 日间软件主题下系统窗口与选择菜单恒为夜间配色

`res/values/themes.xml:9-11` 的 `windowBackground`/`statusBarColor`/`navigationBarColor` 全硬编码 `#1E1E2E`；`:18-19` 把 `colorSurface`/`colorOnSurface` 固定为 **M3 dark baseline**；仓库**无 `values-night/`**。

`TerminalSurface.kt:307/315` 的 `resolveThemeColor(R.attr.colorSurface, …)` 因此永远命中属性、永远走不到回退 —— 选择菜单在日间主题下仍是深色背景（`colors.xml:6` 硬编码 `#21222C`）。

`colors.xml:7-8` 注释自述「选择菜单等系统窗口按主题取色」——**声明与代码事实不符**。同时 `TerminalTheme.kt:534-543` 的 `resolveMaterialColorScheme` 正确地在 `dynamicLight/DarkColorScheme` 间切换，两套主题机制脱节。

**违反** DESIGN:106（软件主题日间/夜间/跟随系统）、:108、:200（启动动画结束后直接显示主题背景）。

#### N10. baseline profile 两文件逐字节相同，基准模块 0 覆盖

`android/app/src/release/generated/baselineProfiles/{baseline-prof,startup-prof}.txt` 均为 18,913 行，`cmp` 结果 **IDENTICAL**；`grep -c 'terminal/emulator/benchmark' baseline-prof.txt` = **0**。

`build.gradle.kts:108-110` 注释声称宏基准采集结果会合并进 `assets/dexopt/baseline.prof`，但入库产物不含任何 `:benchmark` 模块类 → 宏基准那一路未产出或未合并，`:192` 的 `baselineProfile(project(":benchmark"))` 形同虚设。37,826 行机器产物直接入库。

**违反** STYLE:63（不得保留死内容）。

#### N11. `res/values/ids.xml` 四个 id 零引用

`terminal_menu_copy/paste/select_all/share` 在整个 `android/app/src`（含 androidTest）零引用，且 `res/` 下无任何 `layout/` XML。

**违反** STYLE:63。

#### N12. semgrep 规则的漏报与噪声配置错误

| 位置 | 问题 |
| --- | --- |
| `.semgrep/rust-deny-patterns.yml:105-109` | `no-prohibited-shells` 声明 `languages: [rust, kotlin]`，**不扫描 `.github/workflows/*.yml`** —— 而 `fmt.yml:44` 恰恰用 `bash -c`（违反 STYLE:5/61）。这是规则漏报的真实实例。 |
| `.semgrep/rust-arch.yaml:11-23` | `no-allow-in-prod` 用无锚点 `pattern-regex: '\#\[allow\('`（会命中注释与字符串字面量）配 `pattern-not-inside: '#[cfg(test)]'`（独立属性不是合法 Rust 节点，范围判定不可靠）。既漏报测试模块内的 `#[allow]`，也对注释误报。应改用 `paths.exclude`。 |
| `.semgrep/kotlin-deny-patterns.yml:91-96` | `no-map-get-nonnull` 的 `message` 里写了 `$MAP`/`$KEY`，但 semgrep **不对 message 做 metavariable 插值** → 告警永远显示字面 `$MAP.get($KEY)!!`，等于噪声。 |
| `.semgrep/kotlin-deny-patterns.yml:66-67` | `no-runblocking-production` 用单文件白名单 `**/installer/BootstrapInstallService.kt`，与同文件 `:74-78` 的目录级排除风格不一致；该文件改名/拆分即静默失效。 |

**修法**：补 workflow 语言的 shell 规则；`no-allow-in-prod` 改 `paths.exclude`；删除 message 中的 `$` 占位；统一排除风格。

### 第 4 轮排除项（经核实撤回）

- **`configChanges` 缺 `density`/`fontScale`**（`AndroidManifest.xml:25`）：系统字号/密度变化会重建 Activity。子代理推断会「会话丢失」，但项目在设置内自行处理字号（`MIN/MAX_FONT_SIZE_TENTHS` + `applyFontSettings`），且 DESIGN 未声明跟随系统字号。**属设计选择，非缺陷**。
- **Manifest 导出属性**：`exported="true"` 仅 `:73` 的 DocumentsProvider，由 `:75-76` 的 `MANAGE_DOCUMENTS` read/write permission 保护（AOSP 强制要求）；`MainActivity` 是 LAUNCHER 入口；service/FileProvider 均 `exported="false"`；备份规则全排除。**无缺陷**。
- **release 用 AOSP testkey 签名**（`build.gradle.kts:64`）：DESIGN:86 明确要求「使用 AOSP testkey 签名，禁止 debug 签名，禁止使用其他签名」。**符合规范，非缺陷**（但公开发布 testkey 包的供应链风险值得你单独决策，见第八节新增第 8 项）。
- **字符串国际化**：`strings.xml` 105 条零未引用；英文残留均为技术词，符合 STYLE:72「Shell 及其他技术词汇不得翻译」。**无缺陷**。
- **`rust-toolchain.toml:2` `channel = "stable"` 未钉 1.98**：BUILD:22 的下限已由 `Cargo.toml:6-7`（`edition 2024` + `rust-version 1.98`）强制，**不会编译失败**。仅工具链版本跨机漂移，属 BUILD:9「环境确定性」的关注点，非缺陷。
- **`fmt.yml:41` `markdownlint-cli2 --fix` 全仓生效未排除 `docs/specification/`**：确认存在（AGENTS 声明该目录只读）。属 CI 越权风险，见第八节新增第 9 项。

### 第 4 轮新增待决策项

- **D8. release APK 用公开 AOSP testkey 签名并公开发布**（`build.gradle.kts:64` + `build.yml:70-74`）：DESIGN:86 要求 testkey（已遵守），但任何人持同一公钥即可签出 `com.termux` 的更新包。是否接受该风险，或为发布单独配置上传密钥？
- **D9. `fmt.yml:41` 的 `markdownlint-cli2 --fix` 会改写 `docs/specification/`**：AGENTS 明令该目录不可修改。CI 越权改写保护目录。是否需要在 workflow 中加 `--ignore`？

### 连续四轮无新问题的判定

第 4 轮相对前三轮**未发现新的 P0/P1 逻辑缺陷**，新增问题集中在资源配置（N9 主题色）、生成物失真（N10）、死资源（N11）与工具规则配置（N12），并确认升级了发布链路（N7）与 R8 配置（N8）两项。四轮工具数值一致，无回归。

### 第 5 轮：收敛确认

工具逐字复核第 4 轮结果：

- `aislop` 12 warnings（4 个 Code Quality + 2 个 AI Slop 组，与第 1–4 轮完全一致）
- `jscpd` 83 clones / 1.82%（与第 3、4 轮一致）
- `check-rust.nu`：fmt / clippy / machete / semgrep(32 规则 0 发现) / test(504 全过) 全绿
- `check-gradle.nu`：semgrep(16 规则 0 发现) / detekt / spotlessCheck / lintDebug / testDebugUnitTest 全绿

**第 5 轮无任何新问题**，连续四轮（第 2、3、4、5 轮）未发现新的 P0/P1 逻辑缺陷。审查收敛。

剩余待处理项按优先级：

- **P0**（真实缺陷，需改代码）：锁序死锁 ×2、安装假成功、acquire 内联阻塞、快照链死代码
- **N7–N12**（配置/资源/生成物，需改文件）
- **P2/P3**（静默降级、死代码清单）
- **S/B/E 组**（保护文件，需你授权）
- **D1–D9**（需你决策的规范冲突）
