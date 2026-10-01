# 第 6 轮全面审查

审查日期：2026-09-30
范围：`native/`（48 个 `.rs`）、`android/app/src/main/`（175 个 `.kt` + Manifest + res）、`scripts/`、`.github/workflows/`、`flake.nix`、`.semgrep/`
方法：`npx aislop@latest scan` + `jscpd` + code-review-skill（按 Rust / Kotlin / 构建配置三轴并行子代理审读）
本轮只审查，**未改动任何源码**。

---

## 一、结论先行：前五轮「已收敛」的判定不成立

`docs/REVIEW.md` 第 502 行写着「第 5 轮无任何新问题，连续四轮（第 2、3、4、5 轮）未发现新的 P0/P1 逻辑缺陷。审查收敛」。

本轮按同样的方法重跑，**发现 4 个新的 P0 与 11 个新的 P1**，其中多个是前五轮已在清单里但被归错级或未追到根因的问题。原有 P0 清单逐条复核后**全部仍然成立**，无一被修复。

前五轮的真实覆盖边界是：native Rust 的 JNI 层与 ghostty 适配层、Kotlin main 的 UI 与运行时骨架、Manifest/资源、CI 表面。**未触及**的正是本轮新问题的密集区：

- VT 线程的析构路径与阻塞搜索算法
- 渲染状态的会话归属
- `BootstrapOrchestrator` 的第二处假成功点
- DocumentsProvider 的 Binder 线程遍历
- `flake.nix` 声明与 `BUILD.md` 声明的差距
- CI 工作目录里嵌着的第二个 git 仓库

因此本轮不宣称收敛。当前状态：**第 6 轮，距「连续四轮无新问题」尚需至少四轮**。

### 工具基线

| 工具 | 本轮结果 | 与前五轮对比 |
| --- | --- | --- |
| `aislop 0.16.1`（rust，40 文件） | 12 warnings（8 文件超 1000 行、2 函数超 120 行、2 叙述式注释块） | 逐字一致 |
| `jscpd 5.3.3`（阈值 10 行 / 50 词） | 108 clones / 3.02%（markdown 26、kotlin 39、rust 40、yaml 2、wgsl 1） | 与第 3/4/5 轮同源，数值差来自本文件本身 |
| `check-rust.nu` | **exit 0**：fmt / clippy / machete / semgrep（32 规则 0 发现）/ test（504 全过）/ rustdoc / markdownlint（111 文件 0 问题）/ bench | 全绿 |
| `check-gradle.nu` | **exit 0**：semgrep（16 规则 0 发现）/ detekt / spotlessCheck / lintDebug / lintVitalRelease / assembleDebugAndroidTest / testDebugUnitTest | 全绿 |

`check-rust.nu` 与 `check-gradle.nu` 均无错误、无警告。本文档本身已按 `.markdownlint.jsonc` 校验。

**关键教训**：五个自动化工具全绿、连续四轮「无新问题」，仍然漏掉了下述 15 个 P0/P1。静态工具与既有清单都不能替代逐文件读码。

---

## 二、原有 P0/P1 复核：全部仍然成立

逐条重新读码确认，行号以当前 `HEAD`（`656f41b`）为准。

| 编号 | 判定 | 证据 |
| --- | --- | --- |
| P0-1 `setTheme` 锁序反转 | **成立** | `ffi.rs:2745` `rlock_session_registry()` → `:2754` `entry.session.lock()` → `:2761` `render_state_mut()`。两个 guard 都是 `Drop` 类型，活到闭包结束。`render_inner` 在 `:1522` 持 `RENDER_STATE` 后于 `:1601-1611` 再取 registry 读锁 + session 锁，构成 ABBA。 |
| P0-2 `renderWithNewOutput` 三方死锁 | **成立** | `ffi.rs:1876` 取 registry 读锁，guard 活到 `:1889`，`:1883` 在其内取 `render_state_mut()`。 |
| P0-3 `acquire_texture` 回落阻塞 | **成立** | `pass.rs:154-159` `try_send` 失败 → `texture_or_reconfigure` → `pass.rs:121` 无超时 `get_current_texture()`。 |
| P0-4 二段安装假成功 | **成立且更严重** | 见 N1，第 4 节。 |
| P0-5 前台服务/看门狗整块无规范声明 | **成立** | `docs/specification/` 对这五个类零命中。 |
| P1-6 CellData 帧丢弃不补发 | **成立** | `internal.rs:1233-1234`。 |
| P1-7 `resize` 不跳空操作 | **成立** | `TerminalRuntime.kt:3029-3048`。 |
| P1-8 `ResizeOutcome::Dropped` 被忽略 | **成立** | `ffi.rs:681-688` 只匹配 `Err`。 |
| P1-9 `wait_exit_code` 超时上报 0 | **成立** | `ffi.rs:1078`，且 `mark_exit_reported` 已锁存，事件不可重发。 |
| P1-10 `REQUEST_REGISTRY` 泄漏 | **成立** | `ffi.rs:296` 插入，仅 `:1976` 与 `:1192` 移除；2s 超时路径不清理。 |
| P1-11 `pty_write` 改写破坏二进制载荷 | **成立且被加强** | 见第三节 N5。 |
| P1-12 软换行搜索列号越界 | **成立** | `internal.rs:2173-2194`。 |
| P1-13 UAF 不变式未接线 | **成立** | `runAfterRenderThreadsStopped` 全仓零调用。 |
| P1-14 `RenderWatchDog.stop()` 持锁 `runBlocking` | **成立且被低估** | 四个调用点在 `synchronized(sessionLock)` 内：`TerminalRuntime.kt:483 / :1041 / :1540`（经 `:2520 / :2974 / :3222`）。 |
| P1-15 `onSession` 吞掉全部 native 异常 | **成立** | `Bridge.kt:98-107`。 |
| P1-16 `detectArchFromAbi` 静默兜底 | **成立** | `FontUtils.kt:43-46`。 |
| P2-17 查询 API 固定回退 | **成立** | 与 `internal.rs:856-884` 的 `panic!` 策略并存，两套矛盾。 |
| P2-18 裸调 `unsafe libc::kill` | **成立** | `session.rs:456`、`:468`，同文件 `:759` 已有 `nix` 版本。 |
| N1 快照链死代码 | **成立** | `take_snapshot` 调用方全在测试。 |
| N4/N5/N6 重复 | **成立** | jscpd 复核：`ffi.rs` 4 处、`rasterization.rs` 2 处。 |
| N9 系统窗口恒夜间配色 | **成立** | `themes.xml:9-11,24` + 无 `values-night/`。 |
| N11 四个 id 零引用 | **成立** | `res/values/ids.xml:3-6`。 |

**已部分修复的两项**（前五轮清单可下调）：

- **E6 cache key 用 `run_id`「永不命中」→ 部分证伪**。精确 key 确实永不匹配，但 `restore-keys: ${{runner.os}}-${{runner.arch}}-`（`build.yml:36-37`）能前缀命中最新条目，恢复有效。真实危害是每次运行都要上传数 GB 直到被淘汰，不是完全失效。
- **E9 `fmt.yml` 只 check 不 format → 已证伪**。`fmt.yml:41-45` 依次跑 `markdownlint --fix`、`cargo fmt`、`clippy --fix`、`spotlessApply`、`detekt --auto-correct`、`nix fmt`，全部在改写。

**E7 实际比原记录更严重**：`fmt.yml:37-52` 同一个 `run:` 块先 `nix flake update`，再全仓自动改写，最后 `git commit --amend --no-edit` + `ad-m/github-push-action` 带 `force_with_lease: true`，而 `workflow_dispatch` 的默认 ref 就是 `main`。等于把一次未经审阅的 nixpkgs 版本提升 + 受保护目录改写直接强推到默认分支。

---

## 三、新的 P0

> 维护注：N0-1（析构超时等待）、N0-2（回滚搜索二次复杂度）、N0-3（渲染帧会话归属）、N0-4（安装假成功）已修复并验证，对应小节删除；其余编号保持不变。

---

## 四、新的 P1

> 维护注：N1（渲染状态锁外取图放置）已修复并验证，对应小节删除；编号保持不变。

### N2 `grid_size()` 缓存与实时网格发散，会让每一帧渲染都失败

`ffi.rs:1508` 的 `rows/cols` 取自 `session.grid_size()`（`session.rs:438-443` 的**缓存**），而 `cell_data` 由 VT 线程用**实时** `grid_rows` 构建（`internal.rs:1458`）。两者存在时间窗。

`render/cell_builder.rs:295-312` 的 `build_row_ranges` 在 `cell_data` 行数多于 `rows` 时返回 `None`，于是 `pass.rs:693-695` 报 `CellData conversion failed`，`render_inner` 返回 `-1`。

**故障场景**：`Command::Resize` 是 `try_send` 异步投递的，缓存立即更新（`session.rs:401-412`），VT 线程要到下一轮循环才应用。若 JNI resize 恰好落在 VT 线程两次批量构建之间、且新尺寸更小，则**此后每一帧都返回 -1**，只有一条 `log::error!`。更糟的情形是 `internal.rs:777-781` 在 `terminal.resize` 出错时只记日志并保留旧网格，而 session 缓存已经前进 —— 那是**永久**发散。

**修法**：`rows/cols` 与 `cell_data` 取自同一数据源；或 `build_row_ranges` 返回 `None` 时强制全量重建而非报错。

### N3 `dump_grid` 在 VT 线程上同步执行约 110 万次 ghostty FFI 调用

`internal.rs:961-1026` 的 `build_dumped_grid` 对 `(回滚 2000 + 可见行) × cols` 个单元格逐个执行 `grid_ref` + `cell` + `style` + `fg_color` + `bg_color` + `underline_color`（+ 调色板）。入口是 `getTerminalText`（`ffi.rs:2170`），也就是用户「全选 → 复制」的路径。

单次调用足以让 VT 线程停顿数百毫秒；调用方 500ms 超时后走 `public_api.rs:536-541` 的**空网格回退** —— 即静默向用户报告「终端是空的」。`Query::ReadVisibleText`（`internal.rs:201-213`）同理。

违反 `DESIGN.md:24` 与 `DESIGN.md:176`（「全选后复制功能必须能够正常工作」）。

### N4 复制/粘贴的 UI 反馈链路已写好但从未接线

`android/app/src/main/java/terminal/emulator/ui/TerminalSurface.kt:1299-1300`

```kotlin
var onCopyRequested: ((text: String) -> Unit)? = null
var onPasteRequested: (() -> Unit)? = null
```

全仓 grep 只有 4 处命中：这两行声明，加上 `TerminalScreen.kt:557` 与 `:571` 的**赋值**。没有任何一处 `.invoke()`。

赋值体是完整实现（`TerminalScreen.kt:557-586`：dismiss 当前 snackbar、显示 `R.plurals.copied_chars` / `pasted_chars`），唯独没人调用。

**故障场景**：用户长按选中终端文本 → 菜单出现 → 点「复制」→ 剪贴板确实写入了，但**屏幕上没有任何反馈**，用户无从确认是否成功。违反 `DESIGN.md:170`「按钮必须一次点击即生效」与 `TESTING.md:8`。

前五轮把它记为「仅赋值无 invoke」的死代码，低估了：这是**已声明功能缺失**，不是冗余代码。

### N5 `pty_write` 的字节改写确实会损坏 Kitty 图像数据（补强 P1-11）

`native/src/terminal/ghostty_terminal/public_api.rs:151-177`

```rust
pub fn pty_write(&mut self, data: &[u8]) {
    …
    for &raw in data {
        if raw == 0x00 { continue; }
        let sanitized = if raw > 0xF7 { b' ' } else { raw };
        if sanitized == b'\n' && prev != b'\r' {
            buf.push(b'\r');
        }
        buf.push(sanitized);
```

doc 注释（`:149-150`）自己写着「VT 控制序列、DEC 矩形操作与**二进制 VT 数据**应改用 `Self::vt_write`」。但生产路径只有一条：

- `session.rs:497` `self.terminal.pty_write(&snap.filtered)` —— 真实 PTY 输出
- `native/src/terminal/output_processor.rs:44-55` 的 `process()` 只剥离 OSC 52 读取请求，**其余字节逐字透传**

也就是说 DCS 载荷（Kitty 图形 `m=1` 直接 RGB、Sixel、iTerm2 内联图像）原样进入这个改写循环。`0x0A` 被插入 `\r`，`0xF8..0xFF` 被替换成空格 —— **图像静默损坏**。

同时，`pty.rs` 保留了内核 `OPOST`/`ONLCR` 行规程（LF→CRLF 本就该由内核做），这里的转换是重复且多余的。

另注：改写是**有状态**的（`last_pty_write_byte`），会把 DCS 载荷中间的 `0x0A` 与上一个块的末字节做跨块判断，进一步扩大污染面。

违反 `DESIGN.md:58`（Ghostty 是终端状态单一来源，不篡改其输入流）与 `DESIGN.md:180`（kitty 图像协议）。

### N6 `TerminalDocumentsProvider` 在 Binder 线程上无界遍历整个 bootstrap prefix

`android/app/src/main/java/terminal/emulator/TerminalDocumentsProvider.kt:337-353`

```kotlin
val pending = ArrayDeque<File>()
pending.addLast(decodeDocId(rootId, rootDir))
while (pending.isNotEmpty() && cursor.count < MAX_SEARCH_RESULTS) {
    …
    current.listFiles()?.forEach { pending.addLast(it) }
}
```

`rootDir` 是 Termux home，其中包含约 150MB 的 `usr/` prefix。BFS **没有深度上限、没有 visited 集合、不检查 `CancellationSignal`**，且运行在 Binder 线程上。`queryChildDocuments:215` 同样是无界 `listFiles()`。

**故障场景**：用户在系统文件选择器里搜索一个不存在的词 → 遍历整个 prefix（数万次 `listFiles()`）→ 选择器长时间无响应，足以触发 ANR 看门狗（`AnrWatchDog.kt:84` → `Process.killProcess`）→ 由于 `PROHIBITED.md:10` 禁止会话持久化，**所有进行中的 shell 会话被销毁且无法恢复**。

这同时是 N0-5 的一个具体触发场景。

### N7 `test-emulator.nu` 先关掉动画，再跑动画宏基准

`scripts/test-emulator.nu:11-16`

```nu
    for scale in ["window_animation_scale", "transition_animation_scale", "animator_duration_scale"] {
        try { ^adb shell settings put global $scale 0 } catch { null }
    }
    ^./gradlew ":benchmark:connectedBenchmarkReleaseAndroidTest"
    ^./gradlew ":benchmark:connectedBenchmarkReleaseAndroidTest" -P…#modifierKeyPressAnimation
    ^./gradlew ":benchmark:connectedBenchmarkReleaseAndroidTest" -P…#imeShowAnimation
```

`:12` 把 `animator_duration_scale` 设为 0，`:15-16` 才跑两个**动画**基准。零时长动画会让这两个基准测出 0 并通过，断言不到任何东西。

配合 `benchmark/build.gradle.kts:14` 的 `androidx.benchmark.suppressErrors = EMULATOR`（把基准框架的硬失败 `BenchmarkStateError` 降级为警告），仓库里唯一的宏基准证据既测不到东西、也不能因配置原因失败。违反 `TESTING.md:8`、`TESTING.md:6/11`。

### N8 `test-emulator.nu:9` 的 `try` 缺 `catch`，脚本会在清理阶段中止

```nu
    try { ^adb shell am force-stop com.termux }
    try { ^adb uninstall com.termux } catch { null }
```

`:5` 和 `:10` 都有 `catch { null }`，唯独 `:9` 没有。Nushell 的 `try` 不带 `catch` **不会捕获**：任何非零 `adb` 退出都会中止脚本，`:11-17` 的宏基准与基线 profile 采集**永远不会执行**，且报错指向 `:9` 而非真实原因。

违反 `STYLE.md:14`（`try/catch` 只用于「失败本身即为预期状态」）——这里恰恰是预期状态却漏了 `catch`。

### N9 CI 的门禁作用域里混着另一个 git 仓库

`.github/workflows/build.yml:16-20`、`check.yml:16-20`、`fmt.yml:16-19` 三处都做了第二次 checkout：

```yaml
      - uses: actions/checkout@main
        - uses: actions/checkout@main
          with:
            repository: ${{ github.actor }}/kudzu
            path: result-kudzu
```

`result-kudzu` 落在 `$GITHUB_WORKSPACE/result-kudzu`，而所有 `run:` 步骤的工作目录是 `$GITHUB_WORKSPACE`。仓库内**没有** `.semgrepignore`，也没有任何 `result-kudzu` 排除。

**故障场景**：`semgrep scan --error`（`check-rust.nu:7`）与 `markdownlint-cli2 "**/*.md"`（`:10`）都会递归进 `result-kudzu` —— 用**本仓库**的 `.markdownlint.jsonc` 去判定**另一个项目**的 Markdown。一个由外部仓库内容决定通过与否的质量门，比没有门更糟。

### N10 `flake.nix` 没有声明 NDK，`BUILD.md:7` 的「已预设」不成立

`flake.nix:63-111` 的 `packages` 里没有 `androidenv` 或 `android-sdk`，`:112-139` 的 `env` 只设了 `LD_LIBRARY_PATH` / `VK_ICD_FILENAMES` / `FONTCONFIG_FILE`，**没有 `ANDROID_NDK_HOME`**。

而 `docs/specification/BUILD.md` 逐字写着：

- `:5` 「所有工具由 `flake.nix` 声明」
- `:7` 「`ANDROID_NDK_HOME` 已预设，无需回退查找」
- `:20` 「`ndkVersion` 为 `r30`」

本机实测 `ANDROID_NDK_HOME=/usr/local/lib/android/sdk/ndk/27.3.13750724`（**r27d**），来自环境而非 flake。`scripts/build-android-libs.nu:42` 的 `cargo ndk` 因此依赖宿主环境。

**故障场景**：在没有 GitHub runner 镜像的机器上 `nix develop`，`cargo ndk` 直接失败；在 CI 里则静默链接 r27c，**违反 `BUILD.md:20` 的 r30 要求**，且本地与 CI 产物可能不一致。这是 BUILD:5/7/9/20 四条的共同根因。

### N11 `AnrWatchDog` 是「丢会话」而不只是「未声明功能」

`TerminalApp.kt:82-84` 只在 release 安装它。`AnrWatchDog.kt:84-91` → `onAnr()` → `BootGuard.exit` → `Process.killProcess(Process.myPid())`，触发条件是**一次 5 秒的主线程卡顿**，预热期仅 20 秒。

一次 GC、一次 IME 动画、一次首次字形光栅化的停顿就足以触发。由于 `PROHIBITED.md:10` 禁止会话持久化，**所有 shell 与 PTY 全部销毁且不可恢复**。

这是 P0-5「整块功能无规范声明」的具体危害，也是 N6 的下游后果。

---

## 五、新的 P2 / P3

| 编号 | 位置 | 问题 |
| --- | --- | --- |
| N12 | `render/kitty.rs:95-96` | `vec![0u8; stride.saturating_mul(layout.height as usize)]`：`width` 是所有源宽之和、`height` 是最大高（`:45-46`），条带面积可远超输入面积。`saturating_mul` 去掉了溢出 panic，于是超大值变成**分配失败 → `handle_alloc_error` → 无日志直接 abort**。尺寸完全由 PTY 控制（KGP 上限 64MiB，`types.rs:245`）。违反 `DESIGN.md:16`（崩溃退出前须先输出日志）。 |
| N13 | `render/context.rs:367-371` | `attach_surface` 快路径在 `surface.is_some() && surface_config.is_some()` 时直接 `reconfigure_swapchain` 并返回，**不绑定调用方新拿到的 window**；而 `ffi.rs:1370` 无条件 `ANativeWindow_release(ptr)`。若旧 owner 的 detach CAS 失败，渲染器会重配一个包裹**已释放旧 window** 的交换链 → 永久黑屏。注释 `:364-366` 声称的不变式代码从未校验。标为「需真机复现」。 |
| N14 | `Bridge.kt:220` + `NativeBridge.kt:131` | `consumeNewOutput()` / `@JvmStatic external fun consumeNewOutput` 在生产代码零调用点（`TerminalRuntime.kt:1197` 用的是 `renderWithNewOutput` 的合并标志）。`ffi.rs:1245` 的 JNI 导出也随之只剩测试价值。违反 `STYLE.md:63`。 |
| N15 | `public_api.rs:344-408`、`commands.rs:121-128`、`internal.rs:280-360`、`keymap.rs`（整文件 261 行） | Kitty 键盘编码链路**生产零调用**，仅测试使用；`ffi.rs:937-941` 的注释仍把它描述成现役路径。违反 `STYLE.md:63`。 |
| N16 | `public_api.rs:478`、`commands.rs:72`、`internal.rs:201-213`、`public_api.rs:180`（`is_alive`）、`public_api.rs:323`（`cursor_visible`）、`pass.rs:746+`（`render_to_buffer`，约 370 行） | 同上，生产零调用。其中 `is_alive` 零调用尤其讽刺：P2-17 的固定回退正是为了「不致命」，但**没有任何生产代码检查 VT 线程是否已死**。 |
| N17 | `ffi.rs:845-881` | `feedTerminal` 是 release 包里的活跃 JNI 入口，绕过 `OutputProcessor` 且从不置 `new_output`，注释自述「供测试注入」。与 P2-20 的 `TestBackdoorReceivers` 同类。违反 `STYLE.md:65`。 |
| N18 | `app/build.gradle.kts:121` | `force("androidx.concurrent:concurrent-futures:1.3.0")` 是精确版本声明，直接违反 `DESIGN.md:5`「版本声明不写精确版本」。且它作用于**所有** configuration，传递依赖要求 ≥1.4 时会被静默降级，代码里也没有记录原因。 |
| N19 | `flake.nix:124` | `VK_ICD_FILENAMES` 硬编码 `${pkgs.mesa}/share/vulkan/icd.d/lvp_icd.x86_64.json`，而 `:16` 的 `systems` 含 aarch64-linux。aarch64 宿主上 `nix develop` 会把 Vulkan 指向不存在的路径，所有 GPU 测试失败，而 `TESTING.md:12` 禁止跳过。 |
| N20 | `flake.nix:140-144` | shellHook 里放了 `download-aosp-testkey.nu`（未固定 ref 的 AOSP master）与 rapidocr 下载。`build.yml:39-40` 一次构建触发 2 次、`check.yml:41-43` 触发 3 次。网络抖动即硬失败；AOSP testkey 轮换会静默作废升级路径。违反 `BUILD.md:9`。 |
| N21 | `ui/ModifierBar.kt:123,168,384-388,425` | 文档注释承诺 DRAWER 键长按粘贴（Termux `popup: 'PASTE'`），但 `secondaryLongPressAction` 的 `when` 对 DRAWER 落到 `else -> null`，`secondaryLabel` 在 `:425` 硬编码为 `null`。长按 DRAWER 只会打开会话抽屉。违反 `DESIGN.md:115-116`（「布局/按键和 Termux 完全相同」）。 |
| N22 | `runtime/TerminalRuntime.kt:3309-3329`（经 `TerminalSurface.kt:2699` 调用） | `surfaceDestroyed` 在主线程上对**每个会话**发起一次未同步的 `NativeBridge.setRenderPaused` JNI 调用，而该 JNI 可能阻塞在挂死的 GPU 上（同文件 `:2579-2580` 自述）。其余拆卸路径都已用 `surfaceTransitionExecutor`（`:2970`）移出主线程，唯独这条没有。违反 `DESIGN.md:160`。 |
| N23 | `TerminalViewModel.kt:1407-1410` vs `TerminalRuntime.kt:2785` | UI 用 `remaining.last().id` 依赖 `current.sessions` 已排序，runtime 用 `sessions.keys.sorted()`。两处对「最后一个」的假设不同源，列表变化时可能让抽屉与 runtime 指向不同活动会话。当前不可达，属脆弱耦合。 |
| N24 | `TerminalScreen.kt:142,810` | `var searchJob by remember { mutableStateOf<Job?>(null) }` 把 `Job` 存进快照状态，且从非组合回调写入，每次去抖搜索都触发一次重组，而该值在组合中从未被读。违反 `DESIGN.md:20`。 |
| N25 | `.github/workflows/build.yml:2-5`、`check.yml:2-3` | 仍只有 `schedule` + `workflow_dispatch`，**无 `push` / `pull_request` 触发器**。PR 合并前没有任何质量门。（前五轮 E4 记录，本轮复核未修。） |
| N26 | `check.yml:14` | `timeout-minutes: 30` 要装下 504 个 Rust 测试 + 2 个 criterion 基准 + rustdoc + `assembleDebugAndroidTest` + R8（`lintVitalRelease`）+ detekt，且有效缓存因 E6 基本失效、`cachix-action` 是 `continue-on-error: true`（secret 未设时静默 no-op）。冷启动必然超时被杀，并被报成测试失败。 |
| N27 | `.semgrep/rust-deny-patterns.yml:105-109` | `no-prohibited-shells` 的正则是 `\b(fish\|dash\|zsh)\b` —— **恰好漏掉 `bash` 与 `sh`**，也就是 `STYLE.md:5` 明令禁止、且 `fmt.yml:44` 实际在用的那两个。`STYLE.md:61` 说这三个词「不得出现在任何文件中」，但规则只扫 `[rust, kotlin]`。 |
| N28 | `.semgrep/rust-arch.yaml:51-59`、`rust-deny-patterns.yml:80-89,115-119` | `anyhow` / `proot` / `nix/store` 三条用无锚点 `pattern-regex`，会命中注释与字符串字面量。`AGENTS.md:9` 明确「代码」一词不包括「注释」，因此 `// 不要引入 anyhow` 这样的正确注释会导致构建失败。 |
| N29 | `.semgrep/kotlin-deny-patterns.yml:34-35`、`android-deny-patterns.yml:59-60` | `fix:` 块引用了未绑定的 `$SCOPE.launch { … }` 与 `delay($MILLIS)`。`--fix` 会产出不可编译代码。 |
| N30 | `app/build.gradle.kts:72-74` | `publishing { singleVariant("release") }` 而全仓未应用 `maven-publish` 插件 —— 空配置，连带 `check-gradle.nu:6` 的 `app:dokkaGenerate` 产出无人消费。违反 `STYLE.md:63`。 |
| N31 | `native/Cargo.toml:61-64` vs `check-rust.nu:11` | 声明了 3 个 bench（`cell_builder`、`vt_typing`、`cjk_resolve`），门禁只跑前两个。`cjk_resolve` 从未进入任何门禁。违反 `STYLE.md:65`。 |
| N32 | `android/build.gradle.kts:5,11`、`app/build.gradle.kts:21-23,133-136,148,161` | 无 `gradle/libs.versions.toml`。detekt 版本在 `android/build.gradle.kts:11` 与 `app/build.gradle.kts:21-23` 重复三处（一旦漂移，`buildUponDefaultConfig=true` 会抛错）；hilt、benchmark 同样多处声明。漂移**已经发生**：`benchmark/build.gradle.kts` 与 `baselineprofile/build.gradle.kts` 除 `namespace` 外还差一个 `suppressErrors`（N7 的 `benchmark` 侧有）。违反 `DESIGN.md:5`。 |
| N33 | `settings.gradle.kts:4` | 第三方镜像 `mirrors.cloud.tencent.com` 排在 `gradlePluginPortal` **之后**、`google()`/`mavenCentral()` **之前**。第三方可向 CI 投放 AGP / detekt / ktlint / spotless 构件；该镜像未在 `flake.nix` 声明，境外不可达。 |
| N34 | `scripts/setup-emulator.nu:33,54,76`、`rust-deny-patterns.yml:167-182` | `no-nu-step-label` 只匹配 `print\s+"===`，漏掉仓库自己用的 `print $"=== …"` 插值形式（`build-android-libs.nu:53` 在用）；`no-nu-done-print` 只硬编码了 `Done!`/`Boot verified`，漏掉 `print "Emulator booted"`。规则形同虚设，违反 `STYLE.md:22-23`。 |
| N35 | `.gitignore:13,27-33,41,42,45` | 无差别忽略 `*.apk *.png *.ttf *.otf *.woff* *.eot *.meta *.profraw *.p12`。今天 `res/` 只有矢量图所以没出事，但仓库永远无法新增任何位图/字体资源，且这些规则未限定在构建产物目录下。 |
| N36 | `check-gradle.nu:6` | `--continue` 让 8 个独立门禁在一次调用里并发跑完（`gradle.properties:2` 开了 parallel），失败后报错根因不明确；`:baselineprofile` 唯一有功能的 `collectBaselineProfileRelease` 从未被任何地方执行，`app/build.gradle.kts:192 baselineProfile(project(":baselineprofile"))` 完全未被验证。 |

### 死代码汇总（`STYLE.md:63`，均经全仓 grep 确认零生产调用）

Rust：`take_snapshot` / `build_snapshot` / `GridSnapshot` / `GridSnapshot::fallback` / `cached_snapshot` / `Command::TakeSnapshot`（N1 快照链）；`key_encode` / `key_encode_submit` / `Query::KeyEncode` / `keymap.rs` 整文件（N15）；`read_visible_text` / `Query::ReadVisibleText`；`is_alive`；`cursor_visible()`；`render_to_buffer`（约 370 行）；`session.rs` 的 `exited_flag()` / `title()` / `mode_get()`；`internal.rs:930` 的 `apply_style_to_snapshot`。

Kotlin：`TerminalRuntime.kt:3343 runAfterRenderThreadsStopped`；`:2813-2814` 计算后未用的 `currentRows`/`currentCols`；`InputBatchBuffer.reset()`；`SearchDebouncer.flush()`；`Bridge.consumeNewOutput` + 对应 JNI 导出；`KeyboardMode.Standard/Raw/Custom` + `ImeFlagSet`；13 个 `ToolbarKey` 枚举项；`AnrWatchDog.stop()`（自注释「当前无生产调用方」）；`ThermalMonitor.unregister()`；`TerminalViewModel.kt:799` 的 `TAG`；`MainActivity.kt:162` 未用的 `installContext`；`MainActivity.kt:303-305` 永不覆盖的 `viewModelReady`；`TerminalSurface.kt:1299-1300` 的 `onCopyRequested`/`onPasteRequested`（N4）；`Bridge.PollResult.bell` + `PollEvent.Bell`；`res/values/ids.xml:3-6` 四个 id；`BootstrapInstallService` 整类。

---

## 六、受保护文件的改动请求

以下文件按 `AGENTS.md` 不可修改，全部只报告，需你明确授权才能动：

1. `.github/workflows/build.yml` — 加 `push: tags`（否则 `:70` 分支不可达）；让 `build-apk.nu` 不再删除 release 变体；release 构建排在 debug 之后；cache key 去掉 `run_id`；补 `pull_request` 触发。
2. `.github/workflows/check.yml` — 补 `pull_request`/`push` 触发；调高 `timeout-minutes`；把 `result-kudzu` 排除出 semgrep/markdownlint（或加 `.semgrepignore`）。
3. `.github/workflows/fmt.yml` — 停用 `--amend` + `force_with_lease`；移除 `nix flake update`；把 `markdownlint --fix` / `nix fmt` 的作用域排除 `docs/`、`.github/`、`.semgrep/`；替换 `bash -c "pushd …"`（`STYLE.md:5/61`）。
4. `scripts/*.nu` — `build-android-libs.nu` 改 `--platform 33`、删 `:37-38`、补 `NEEDED`/`.so` 体积/APK 含 `.so` 三项校验（`BUILD.md:15-17`）；`test-emulator.nu` 补 `:9` 的 `catch` 并把动画缩放重置移到动画基准之后；`check-gradle.nu` 去 `--continue`、补第三个 bench、执行 `collectBaselineProfileRelease`；`setup-emulator.nu` 零引用，建议删除或迁入 `flake.nix`。
5. `flake.nix` — 声明 NDK 并导出 `ANDROID_NDK_HOME`（修 BUILD:5/7/9/20）；按系统修正 `VK_ICD_FILENAMES`；移除未使用的 `zig_0_16`/`gradle`；`nu scripts/…` 改 `./scripts/…`；把 shellHook 里的网络下载移出每次 `nix develop`。
6. `Cargo.toml` — `fontdb` 升到 `0.24`（现锁 `0.23.0`，最新稳定 `0.24.0`，违反 `BUILD.md:23`）；移除 `type_complexity = "allow"`（不在 `STYLE.md:37` 允许清单内）。
7. `android/app/build.gradle.kts` — 删 `force(…1.3.0)`（`DESIGN.md:5`）；删空的 `publishing`；删 `:91` 的 `jniLibs` 冗余；建 `libs.versions.toml` 消除四处版本重复；6 处「有更高稳定版却用预发布」。
8. `android/build.gradle.kts` + `settings.gradle.kts` — 移除腾讯云镜像的优先位（N33）。
9. `android/detekt.yml` — 8 条抑制超出 `STYLE.md:37` 允许范围。
10. `.semgrep/*.y*ml` — `no-prohibited-shells` 补 `bash`/`sh`；`no-torvox-identifier`/`no-nix-store-paths` 扩展到非 `[rust, kotlin]`（`STYLE.md:59/60/61` 是绝对表述）；删掉未绑定的 `fix:` 块；把针对 `**/*.gradle.kts` 的规则移到合适文件；给正则规则加注释排除。
11. `.gitignore` — 把资源类通配限定到构建产物目录。
12. `docs/specification/BUILD.md` — `:7`「`ANDROID_NDK_HOME` 已预设」与 `:20`「`ndkVersion` 为 `r30`」目前都是事实错误，需在 N10 修好后同步。

---

## 七、仍需你决策的规范冲突

前五轮的 D1–D9 依然未决，本轮新增两项：

- **D10 `STYLE.md:61` 与 `.semgrep/rust-deny-patterns.yml:105-109` 的口径差**。`STYLE.md:61` 说 `fish`/`dash`/`zsh`「不得出现在**任何文件**」，但同文件 `:5` 又禁止 bash/sh 脚本，而仓库自己在 `fmt.yml:44` 用 `bash -c`。是禁止范围要扩展到 `.yml`/`.nix`/`.md` 并把 `bash`/`sh` 补进规则，还是 `:61` 只针对 rust/kotlin 源码？两种改法差别很大，需要你定口径。
- **D11 `TESTING.md:9` 禁 `#[ignore]` / 禁跳过，与 N7（动画宏基准在零时长下测不出东西）、N19（aarch64 上 lavapipe 路径不存在）冲突**。当前用「静默地测不出东西」来规避环境问题，与「不得隐藏错误」直接抵触。正确做法是让环境失败（`TESTING.md:12`），但这需要先修 `flake.nix` 的 N19 和 `test-emulator.nu` 的 N7。

---

## 八、修复顺序建议

按「一行改动能消除一个永久性错误」与「不修就持续丢数据」排序：

1. **N0-1**（`join` 改 `join_with_timeout`）—— 复用已有实现，一处改动消除永久挂起。
2. **N0-4 + P0-4**（`SecondStageRunner` 的 `success` 与编排器的分支）—— 两处，堵住「dpkg 半配置被报成成功」。
3. **N1**（把 `take_kitty_placements` 移出 `RENDER_STATE`）—— 一处改动同时关闭 P0-1、P0-2 与本轮 N1 的性能塌陷，是整个渲染锁序问题的单点解。
4. **N0-2**（搜索单次前向扫描）—— 同时修掉 O(n²) 冻结、P1-12 列号越界、每行读 FFI 三重问题。
5. **N0-3**（`last_frame` 带会话 id）、**N2**（`grid_size` 同源）、**N3**（`dump_grid` 停顿）—— 渲染正确性三连。
6. **N4**（接线 `onCopyRequested`）—— 用户可见的缺失反馈，接线即可。
7. **P1-6 / P1-8 / P1-9 / P1-10** —— 四个「信号被吞掉」的缺陷，每个几行，各自产生一个永久错误状态。
8. **N6 + N11**（DocumentsProvider 有界遍历 + 移除 `AnrWatchDog`）—— 消除丢会话路径。
9. **N5**（`pty_write` 改写）—— 需先确认删除后依赖内核 `ONLCR` 的实测行为。
10. **N7/N8/N9/N10/N25/N26** —— 门禁与工具链正确性，需授权保护文件。
11. **P0-5 / P0-3 / P2-17 / P2-20** 及其余 P2/P3 与死代码清单。
12. **E1 / N7**（发布链路）—— 在 `build-apk.nu` 删两个变体与 `push: tags` 缺失修好之前，打 tag 产出的是空 release。

---

## 九、收敛状态

- 本轮**不是**「无新问题」的一轮。相对第 5 轮新增 **4 个 P0、11 个 P1、25 个 P2/P3**、**30+ 处死代码**。
- 「连续四次无新问题」的计数**归零**，从第 6 轮重新开始。
- 前五轮「已收敛」的结论作废：它的成因是自动化工具全绿被当成了充分证据，而三个审读轴里没有一轴真正逐行读过 VT 线程析构路径、回滚区搜索算法、`BootstrapOrchestrator` 的返回值消费方、以及 CI 工作目录的实际内容。
- 建议下一轮直接以本文档第八节的 1–7 项为起点，核实每项修完后**新增**的失败模式（尤其是第 3 项的锁序重排与第 4 项的搜索重写），再进入第 7 轮。
