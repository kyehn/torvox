# 第 7 轮全面审查

审查日期：2026-09-30
范围（本轮新增覆盖）：`native/src/terminal/{session,pty,output_processor,mod,mock_pty}.rs`、`native/src/event.rs`、`native/src/lib.rs`、`native/src/render/{context,pipeline,mod}.rs`、`android/app/src/main/java/terminal/emulator/{settings,FontUtils,MainActivity,TerminalApp,DocumentMutations,TerminalDocumentsProvider,bridge,installer,monitor,service,ui/theme}`、`ui/SettingsScreen.kt`、`ui/SettingsComponents.kt`
方法：两个并行审读子代理（Rust 进程/渲染生命周期轴、Kotlin 设置/主题/安装/文档提供器轴），逐条回读核实
本轮只审查，**未改动任何源码**。

---

## 一、工具基线

与第 6 轮一致，未跑重复工具（`aislop` 12 warnings、`jscpd` 108 clones / 3.02%、`check-rust.nu` exit 0、`check-gradle.nu` exit 0）。上一轮已证明**五个自动化工具全绿不构成收敛证据**，因此本轮把全部预算投入人工逐文件读码。

---

## 二、第 6 轮的收敛结论再次作废（第一次）

第 6 轮刚写下「距连续四轮无新问题尚需至少四轮」，本轮立刻产出 **2 个新的 P0、3 个新的 P1（另有 8 个 P1/P2）**。

第 6 轮暴露的规律在本轮被重复验证：**未被任何一轮真正逐行读过**的文件里，几乎每 200 行就有一个真实缺陷。第 6 轮读的是 `ffi.rs` / `ghostty_terminal/` / `render/cell_builder` / Kotlin 的 runtime 与 surface 层；本轮读的是**它们的上游**——PTY 进程生命周期、`Session` 通道、渲染器表面生命周期，以及 Kotlin 的设置、主题、安装、文档提供器。

四轮计数归零，从第 7 轮重新开始。

---

## 三、新的 P0 / P1

> 维护注：N0-6（PTY 写入截断）、N1-1（读取线程重复剥离 NUL）、N1-2（fork 后泄漏活子进程与僵尸）、N1-4（离线安装无上限写 cacheDir）已修复并验证，对应小节删除；其余编号保持不变。

### N1-6 KGP 管线在 surface 格式变化时不失效；且每帧重建 TextureView + BindGroup

`native/src/render/context.rs:465-472` 在格式变化时只重置 `cell_pipeline` / `cell_bind_group`；而 `native/src/render/pipeline.rs:187-191` 的 `kgp_pipeline` 只在 `is_none()` 时创建，`:180-185` 只在首次创建时读取 `surface_config.format`。

**后果**：一次产出不同 `caps.formats[0]` 的重新挂载之后，KGP 管线保留旧的 `ColorTargetState::format`，而 `begin_frame`（`context.rs:196`）每帧都用新配置调用 `ensure_kgp_pipeline` → 附件不匹配，每帧校验失败（经 `context.rs:9` 的 `log_gpu_error`），**整条 KGP 通道静默失效**。`Renderer::set_surface_config`（`:503`）有同样的缺口。可达性标为「需真机复现」。

**同时**，`pipeline.rs:229-230` 与 `:239` 每帧无条件分配一个 `TextureView` 和一个 `BindGroup`。屏幕上只要有 Kitty 图像，这就是每帧一次分配 —— `context.rs:580` 的 `self.kgp_bind_group = None` 因此完全失去意义。

违反 `DESIGN.md:20`（高性能、低功耗是目标）。

---

## 四、新的 P2

| 编号 | 位置 | 问题 |
| --- | --- | --- |
| N2-1 | `FontUtils.kt:25-39` | `resolveEffectiveFontFamily` 把 `mono`/`monospaced`/`sans`/`sans serif` 硬编码改写为 `monospace`/`sans-serif`。而用户选择的列表来自 `NativeBridge.listFontFamilies()`（fontdb 目录），一个真名为 “Sans” 的字族会被静默换成**另一个**字族。`DESIGN.md:101` 明文「不得做手动判断，而是要求外部库 API 提供正确的字体列表」，`DESIGN.md:102`「不使用任何硬编码」。 |
| N2-2 | `Bridge.kt:409` vs `TerminalRuntime.kt:1958-1967` | `~/.termux/fonts` 的内容只在字体管线重建时重新扫描，而管线每个会话只重建一次。运行中的 shell 往该目录拷 `.ttf` 后**必须重启进程**才生效。`_availableFonts`（`TerminalViewModel.kt:899`）同样只加载一次。`~/.termux/font.ttf` 反而是好的（`Bridge.kt:419-432` 有 mtime+长度探针）。违反 `DESIGN.md:99`。 |
| N2-3 | `SecondStageRunner.kt:79` | `postinstDir.listFiles()?.filter { … }` 无排序 —— postinst 按文件系统 readdir 顺序执行。dpkg 的 postinst 之间有依赖顺序，provider 未配置就 configure consumer 会留下半配置树。违反 `DESIGN.md:142`。 |
| N2-4 | `BootstrapDownloader.kt:36-43` + `BootstrapOrchestrator.kt:104-108` | `file://` URL 直接返回用户自己的 `localFile`（不是副本），编排器的 `finally { zipFile.delete() }` 会**删掉用户的文件**。用户在设置里粘贴 `file:///…/home/x.zip` 即可触发。跨存储删除会被 scoped storage 挡住，但 app 可写路径上就是破坏性行为。 |
| N2-5 | `Bridge.kt:308` | `LogUtil.w(TAG, "pollAll: bad JSON: ${exception.message}")` 与 `PollEvent.kt:46-47`（`exceptionsWithDebugInfo = false`，解码错误不得把涉事 JSON 写入日志，可能含剪贴板文本/URL）自相矛盾 —— kotlinx 会把 JSON 路径与出错字符嵌进 `SerializationException.message`。自述的脱敏策略被自己破掉。 |
| N2-6 | `pty.rs:311-316, 328-333, 337-343` | fork 子进程的 `setsid` 失败、孤儿化、`TIOCSCTTY` 失败三条路径都是裸 `_exit(2/1/3)`，**不写 fd 2**（而 `execve` 路径 `:365-371` / `:394-425` 写了）。TIOCSCTTY 失败的用户表现是：会话瞬间死亡、终端全空、只有 `[Process completed (code 3)]`。违反 `DESIGN.md:16`（极端情况 → 输出日志并崩溃退出）、`DESIGN.md:194`（启动入口失败保留输出显示）。 |
| N2-7 | `context.rs:201-222` | `begin_frame` 在**取得纹理之后**才因尺寸不符重配交换链，然后返回包裹着**旧配置下**纹理的 `FrameContext`。wgpu 在 `configure` 时会使未提交的 `SurfaceTexture` 失效，于是 `FrameContext::submit`（`:28-31`）提交的是表面已放弃的缓冲。`:211` 的 `self.surface_config.take()?` 还会在 `None` 时丢弃一个已取得的 `SurfaceTexture` 而不 `present()`，在 `desired_maximum_frame_latency: 3`（`:445`）下会烧掉缓冲队列槽位。 |
| N2-8 | `Session::drop`（`session.rs:703-746`） | `Drop::drop` 在字段析构**之前**运行，此刻 `output_rx` 仍存活。若读取线程正阻塞在 N0-7 的 `output_tx.send` 上，四次 `wait_finished` 窗口（`:668-678`）全部超时 → 句柄被丢弃（线程分离），并且**每次关闭会话都要付 350ms + 50ms `sleep(TRAILING_EXIT_GRACE)`**。违反 `DESIGN.md:160`。 |
| N2-9 | `event.rs:23` + `:9` | `Event::Clipboard { text: String }` 载荷无上限，队列容量 `MAX_QUEUED_EVENTS = 1024`，FIFO 只淘汰最老的非 `Exit` 事件。Kotlin 轮询之间若程序反复写大段 OSC 52，会滞留 1024 份载荷。违反 `DESIGN.md:20`。 |
| N2-10 | `event.rs:88-93` | 队列只剩 `Exit` 时 `None => { return; }` —— 新的 `Bell` / `ClipboardRead` 被直接丢弃，只有一条限流的通用警告（`:103`），既不点名事件也不点名会话。违反 `DESIGN.md:24`。 |
| N2-11 | `session.rs:646-651` | `focus_event` 在**持有 session 锁**的情况下做 50ms 有界同步 RPC（`mode_get_with_timeout(1004, 0, …50ms)`）。在第 6 轮 N0-2 的条件下（VT 线程可被 park 数分钟），每次焦点变化都要付满 50ms 且全程持锁。违反 `DESIGN.md:160`。 |
| N2-12 | `output_processor.rs:132` | `self.buf.len() >= MAX_SCAN_BYTES` 超限时整个 `ESC ] 52 ; …` 原样透传给 Ghostty，而上游忽略读取请求 → 请求永不被应答，也**没有任何日志**。已核实：所有溢出路径都正确 drain 并回到 `Ground`/重推 `ESC`，`buf` 上界 65 字节，**不存在扫描器失步**；但静默丢弃请求本身违反 `DESIGN.md:24`。 |
| N2-13 | `context.rs:536-575` | `set_kgp_atlas` 在 `queue.write_texture` 前不校验 `rgba_data.len() >= 4*width*height`。当前由 `kitty::pack_and_build` 保证成立，但一旦上游改动，短切片会带着无日志的校验失败进入 wgpu。与第 6 轮 N12 同类。 |
| N2-14 | `context.rs:585-593` + `:503` | `orthographic_projection(0.0, 0.0)` 在除法里产出 `2.0/0.0 = inf`。`attach_surface`（`:903`）用 `.max(1)` 钳住了，`set_surface_config`（`:503`）**没有** → 离屏 0×0 配置得到全 `inf` uniform，静默空白帧。 |

### 新的 P3 与死代码

- **死代码（`STYLE.md:63`，全仓 grep 确认零生产调用）**：`Pty::wait`（`pty.rs:68`，实现在 `:434`、`:563-565`、`mock_pty.rs:171-181`）；`Pty::master_fd`（`pty.rs:62`、`:555`，仅测试）；`Renderer::release_gpu_surface` / `GLOBAL_SURFACE` / `clear_global_surface`（`context.rs:869-892`，约 45 行表面缓存机制）；`MockPty::spawn`（`mock_pty.rs:187`）；`SettingsComponents.kt:61/155/201/244` 的 `SettingsRow`/`SettingsSwitchRow`/`SettingsSelectorRow`/`SettingsSelectorPill`（约 140 行，该文件只有 `rememberIsSmallScreen` 与 `SettingsColors` 在用）；`TerminalTheme.kt:547-551` 的 `enum ThemeMode`；`TerminalForegroundService.kt:158` 的 `onBind`（无任何 `bindService` 调用点）；`FontUtils.kt:19-20` 的 `termuxDefaultFontFile(Context)` 重载；`TerminalApp.kt:20,84` 写入后从不读的 `anrWatchDog` 字段。
- **恒假分支**：`SettingsScreen.kt:378-379` 与 `TerminalTheme.kt:515-517` 的 `"day"`/`"night"` 分支不可达 —— `setThemeMode` 只被以 `"fixed"`/`"follow_system"` 调用（`SettingsScreen.kt:840`）。
- **`STYLE.md:57` 禁止单字母变量**（生产代码）：`context.rs:202/215/229/234/741/768/771/801`（`p`/`v`/`s`/`buf`）、`session.rs:699`（`e`）、`output_processor.rs:400-415`（`i`/`e`/`d`）、`pty.rs:197/241`（`c`）。
- **`AGENTS.md:25` 禁止魔数**：`pty.rs:397` `let mut buf = [0u8; 64]`、`:403` `[0u8; 10]`；`MainActivity.kt:198` `requestPermissions(arrayOf(…), 1)`；`RenderWatchDog.kt:20` `10_000_000_000L` 与 `:42` `2000L`（同文件已有具名常量 `CHECK_INTERVAL_MS`）。
- **`STYLE.md:58` 要求简体中文注释，两处英文残留**：`SettingsDataStoreProvider.kt:32` `/** Screen width in dp, used for the device-adaptive default font size. */`；`FontUtils.kt:41` 的英文说明与中文注释并存。
- **`STYLE.md:59` 违规**：`TerminalForegroundService.kt:22` `WAKE_LOCK_TAG = "termvox:wakelock"` —— 全仓唯一一处软件名出现，且唤醒锁 tag 会通过 `dumpsys power` 对用户可见。
- **重复代码**：`pty.rs:189-192` 与 `:220-223` 逐字重复 `"/system/bin/linker64"`/`"/system/bin/linker"` 与同一段 `cfg!` 架构判定（jscpd 也识别为克隆）。
- **`pty.rs:508-516`** `set_nonblocking` 用 `OFlag::from_bits_truncate`，会静默丢弃 `F_GETFL` 中未知的 `O_*` 位；`from_bits` 能暴露出来。
- **`output_processor.rs`** 溢出时序（`MAX_SCAN_BYTES`）已核实**无扫描器失步**，但 >59 字节的选择器名会让请求静默消失（N2-12）。
- **下载进度**：`BootstrapDownloader.kt:103-111` 在 `contentLength <= 0` 时 `pct` 固定为 `-1`，进度条全程停在 0%。
- **`BootstrapDownloader.kt:47` 直接拒绝 `http://`**，而 `DESIGN.md:126` 写的是「支持 HTTP/HTTPS URL」。有意的加固，但等于静默收窄了已声明能力。
- **`startBootstrapJob`（`TerminalViewModel.kt:1072`）用 `catch (exception: Exception)`**，在 `viewModelScope` 销毁时会吞掉 `CancellationException`；仓库里已有 `runCatchingCancellable` 专门解决这个。
- **`ThermalMonitor.kt:29-33`** 用 `?:` 重建 `thermalListener` 的错误分支不可达（`:19` 已赋值），`:35` 的 `catch (exception: Exception)` 吞掉无 thermal 服务设备上的注册失败。
- **`mock_pty.rs:83-87`** `MockPtyHandle::resize` 改 `rows`/`cols` 却不增 `resize_count`（`MockPty::resize` 在 `:136` 会增），两个「PTY 尺寸」视图可以静默不一致。
- **`context.rs:874`** 生产代码里的 `.expect("surface confirmed Some by is_some guard")`；**`context.rs:195`** `refresh_cell_uniforms` 写的是重配前的尺寸，同帧 `:225` 又用重配后的覆盖 —— 白做的工作。

---

## 五、修复顺序（本轮增量）

1. **N1-6** —— KGP 管线失效与每帧分配。
2. **N2-1 / N2-2** —— 违反 `DESIGN.md:99/101/102` 的显式条款。
3. **N2-6 / N2-7** —— 崩溃无诊断、重配后提交失效纹理，两者都直接破坏 `DESIGN.md:194` 与 `DESIGN.md:160`。
4. 剩余 P2/P3 与死代码清单。
5. 第 6 轮第八节的 1–7 项。

---

## 六、收敛状态

- 本轮**不是**「无新问题」的一轮。新增 **2 个 P0、1 个 P1 组（另 5 个独立 P1）、14 个 P2、10+ 个 P3**、**12 处死代码**。
- 「连续四次无新问题」计数**第二次归零**，从第 7 轮重新开始。
- 两次归零的共同成因已经清楚：**按「文件/目录」分轴审读，但每个轴的样本量远小于文件总量**。第 6 轮读了 6 个 Rust 文件 + 3 个 Kotlin 区域，本轮读了 8 个 Rust 文件 + 9 个 Kotlin 区域，两轮合计仍只覆盖 `native/src` 的 40 个文件与 `android/app/src/main` 的 123 个文件中的约三分之一，而每一轮都在未读区域找到 P0。
- **建议**：下一轮不要按目录分配审读预算，改为**按缺陷类别做全仓横切**（例如「所有 `try`/`catch`/`runCatching` 的吞错点」「所有 `return Err` 后不清理的路径」「所有持锁跨阻塞调用」「所有 `listFiles`/目录遍历」「所有 JNI 导出与其 Kotlin 对应方的契约一致性」），这样能一次性覆盖尚未读过的文件。
