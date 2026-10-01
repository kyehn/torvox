# 第 12 轮全面审查（输入/IME 链路 + 门禁有效性）

审查日期：2026-09-30
基线提交：`3de0b2d`
方法：第 11 轮换到缓存/脏跟踪与 UI/安装器状态机。本轮再换两个新维度：
(1) **输入 / IME / 编码 / 事件投递端到端链路**，
(2) **测试套件、构建配置与门禁有效性** —— 后者针对的是「11 个 P0 存在而五道自动化门禁全绿」这一事实。
维度 (2) 的每条结论都用 `semgrep` 实机跑过对照实验，不是读配置推断。
本轮只审查，**未改动任何源码**（实验在 `/tmp` 中进行并已删除，仓库 `git status` 干净）。

---

## 一、本轮的四个 P0（三个已实机复现）

> 维护注：N0-16（新增`.semgrepignore`恢复单测扫描）、N0-15（选区钳位改绝对空间）、N0-17（七条规则改 generic）、N0-18（删 pattern-not-inside 并补 exclude）已修复并验证，对应小节删除；其余编号保持不变。

## 二、新的 P1

### N1-24 输入法抑制窗口吞掉 `commitText` 并清零组字基线

`TerminalSurface.kt:641-645` 与 `:681-685`：

```kotlin
override fun setComposingText(text: CharSequence?, newCursorPosition: Int): Boolean {
    if (isPaused || System.nanoTime() < suppressUntilNanos) {
        composingBuffer = ""
        return true
    }
```

`suppressUntilNanos` 的注释（`:1316-1320`）自述用途是**抑制「轻击清除」**，
但这两个输入法回调也查询它，而它被武装两次：
抽屉关闭后 **350 ms**（`:1321`，常量 `DRAWER_CLOSE_TAP_GRACE_NANOS = 350_000_000L` 在 `:1203`）
与两次焦点切换各 `SUPPRESS_GRACE_PERIOD_NS`（`:2098`、`:2104`）。

**两个独立故障**：

1. **丢字**：窗口内到达的 `commitText` 被 `return true` 丢弃。
   返回 `true` 等于告诉输入法「已处理」，输入法**不会重发**，该字符永久丢失。
2. **重复文本**：若已转发过组字（`composingBuffer = "ni"`），
   随后某次中间更新被抑制并把 `composingBuffer` 清零，
   下一次更新以空基线做 diff，追加完整串 → 终端出现 `ni` + `nihao`。

这正是 `TerminalSurface.kt:2101-2102` 注释声称要避免的「重复文本、多余空格或静默丢字」。

**修法**：把触摸抑制与输入法抑制拆开。输入法侧在被抑制时**返回 `false`**
（让输入法重试）且**不清 `composingBuffer`**；或把待处理的
`commitText`/`setComposingText` 入队，窗口过期后重放。

> 维护注：N1-25（硬件按键经共用贴底点）已修复并验证，对应小节删除；编号保持不变。

### N1-26 选区菜单的粘贴在主线程逐块同步写 PTY，绕过 `InputBatchBuffer`

`TerminalViewModel.kt:655-664`：

```kotlin
fun executePaste(text: String): Int {
    var offset = 0
    for (chunk in PasteChunker().chunks(text)) {
        runtime.writeToPty(chunk.toByteArray())
        offset += chunk.length
    }
```

`PasteChunker` 上限 4 000 字符（`PasteChunker.kt`），故 100 万字符的剪贴板产生 250 块，
每块都是主线程上的 JNI 调用，阻塞在会话互斥锁 + `write_all` 上。
违反 `TESTING.md:33-34`（输入法/粘贴无卡顿）与 `DESIGN.md:20`（高性能）。

与长按粘贴路径不一致：`TerminalSurface.kt:2244-2246` 走 `InputBatchBuffer`。
两条粘贴路径因此有不同的顺序域（见 N1-27）与不同的丢字行为。

**修法**：`executePaste` 改走 `InputBatchBuffer.write`（与长按粘贴同一出口）。

### N1-27 输入法的「追加」与「退格」走两个不同的发送域，次序无保证

`TerminalSurface.kt:665-671`（追加）走异步批缓冲：

```kotlin
if (edit.append.isNotEmpty()) {
    encodeAndSend(edit.append, ctrlActive = false, altActive = false)   // → inputBatchBuffer.write
}
```

而 `:660-664`（退格）走同步直写：

```kotlin
if (edit.backspaces > 0) {
    viewModel?.writeToPty(ByteArray(edit.backspaces) { BACKSPACE_BYTE })
}
```

`InputBatchBuffer.send` 经 `sender.execute`（`InputBatchBuffer.kt:89`）异步派发，
且 `flushSink` 在**执行时**才解析 `sessions[activeSessionId]`；
`writeToPty` 是主线程同步调用。两者之间没有共享锁或序号。

当追加量超过 `COMPOSITION_COMMIT_MAX_BYTES`（`InputBatchBuffer.kt:42`）
时会被**刻意**推迟到下一个 Choreographer 帧 —— 也就是说
「退格（已到达）+ 追加（仍在队列里）」的顺序完全可能反转。

**用户表现**：CJK 输入法组字回退后再前进，终端出现残留或多余字符。

**修法**：一次输入法事务的全部字节走同一出口 —— 让 `setComposingText` /
`deleteSurroundingText` 的退格字节也经 `InputBatchBuffer.write`，
或给 `InputBatchBuffer` 增加有序的 `writeAll(List<ByteArray>)`。

### N1-28 `ImePopupPixelInstrumentedTest` 的「上移前后」断言实际比较的是**弹出后的两帧**

`android/app/src/androidTest/java/terminal/emulator/ui/ImePopupPixelInstrumentedTest.kt:319-322`：

```kotlin
// 上移前后底部像素完全相同：贴输入法上沿的缝线行必须一致。
val seamTop = before.height - imeHeight - 12
val seamDiff = countDifferingPixels(movedFrame, settled, seamTop, before.height - imeHeight)
assertTrue("底部缝线像素必须完全相同 (差分=$seamDiff)", seamDiff == 0)
```

`movedFrame` 采自 `:307`（**弹出之后**），`settled` 采自 `:316`（再过 1 s）。
`before`（`:273`，弹出前）只被用来计算 `seamTop`，**从未参与像素比较**。
两帧之间界面静止，`diff == 0` 平凡成立，无法发现弹出引入的缝线/吞底缺陷。

`docs/specification/TESTING.md:34` 原文：「上移后终端与上移前终端的**底部像素完全相同**」。

注：同一文件的闪烁断言（`:317`）用 `movedFrame` vs `settled` 是**正确**的 ——
闪烁本就该比较弹出后的两帧，不能与 `before` 比。

**修法**：另采一组弹出前帧（`beforeSeam`），与 `settled` 比较缝线行。

### N1-29 `ktlint` 与 `ktfmt` 已配置但从未被任何门禁调用

- `android/build.gradle.kts:16` 声明 `id("org.jlleitschuh.gradle.ktlint") version "14.2.0" apply false`，
  `:57-61` 对 app 模块 `apply(plugin = …)` 并配置 `KtlintExtension`。
- `android/app/build.gradle.kts:10` 声明 `id("com.ncorti.ktfmt.gradle")`。

**实机检索**（`grep -rn "ktlint\|ktfmt" .github/workflows scripts/*.nu flake.nix`）→ **零命中**。
唯一的 Gradle 入口是 `.github/workflows/fmt.yml` 的 `spotlessApply`
（spotless 内嵌 ktlint 1.8.0 引擎，是**另一个 task**）
与 `scripts/check-gradle.nu:6` 的 `detekt spotlessCheck …`。
`ktlintCheck` 与 `ktfmtCheck` 从未被请求。

`BUILD.md:23`「不得保留未使用依赖，所有依赖使用最新稳定版本」——
两个插件是纯粹的配置负债，且它们的规则集与 spotless 冲突时会产生困惑。

**修法**：二选一 —— 从两个 `build.gradle.kts` 删除（受保护文件，需授权），
或在 `check-gradle.nu:6` 补 `ktlintCheck ktfmtCheck`。

---

## 三、新的 P2

| 编号 | 位置 | 问题 |
| --- | --- | --- |
| N2-47 | `scripts/check-rust.nu:11` | 只跑 `--bench cell_builder --bench vt_typing`，`cjk_resolve` 不进门禁，尽管其前提（系统存在 DejaVu/Liberation 等宽字体）在 `nix develop` 下成立。`AGENTS.md` 要求 `std::hint::black_box` 的三个 bench 中有一个永不执行。 |
| N2-48 | `TerminalSurface.kt:636` | `TerminalInputEncoder.encodeCommittedText(bracketedPaste = false)` 硬编码 false，`\e[200~…\e[201~` 机制**从不发出**。向启用 bracketed paste 的程序（vim、现代 shell）粘贴会逐行执行而非作为一次输入。参数与实现是死代码。 |
| N2-49 | `ffi.rs:828-832` | 主端 `O_NONBLOCK` 下 `write_all`（`pty.rs:80-91`）遇部分写后 `EAGAIN` → 剩余字节**静默丢弃且无日志**。注释只授权「丢弃整块输入」（与 xterm 一致），未覆盖「部分写后截断」。 |
| N2-50 | `TerminalRuntime.kt:2911` | `writeToPty` 在**执行时**才解析目标会话，故跨会话切换的大块粘贴会把尾部写进新会话。对比 OSC-52 应答路径在请求时即捕获 `Arc<Session>`（`ffi.rs:1181-1194`）—— 同一代码库内两种正确性标准。 |
| N2-51 | `PasteChunker.kt:29` | `text.take(maxChars)` 按 UTF-16 索引截断，可把代理对劈开（尾部变孤立代理项，`toByteArray(UTF_8)` 输出 `?`）。分块边界（`:34-36`）是代理安全的，整体截断不是。另 `replace("\n","\r")` 会把 CRLF 变成 `\r\r`。 |
| N2-52 | `android/detekt.yml:26-34` | `SwallowedException` / `TooGenericExceptionCaught` / `TooGenericExceptionThrown` / `InstanceOfCheckForException` / `MagicNumber` 全部 `active: false`。`STYLE.md:37` 只允许抑制「参数数量、行数、嵌套层数、缺失文档」等纯风格项；`AGENTS.md` 明确「禁止魔数」。吞异常正是 `TestUtils.kt:80-86`、`NavigationSteps.kt` 九处 `getOrDefault(false)` 保持不可见的原因。 |
| N2-53 | `app/build.gradle.kts:86-89` + `src/test/java/android/util/Log.kt:11-53` | `unitTests.isReturnDefaultValues = true` 叠加一个恒返回 0 的 `Log` 桩，使「Android SDK 未被打桩」这件事不可见。检索全部 57 个单测文件对 `Log.` 的引用为 0 命中 —— 该桩不换来任何东西，却让未打桩调用静默返回默认值而非大声失败。违反 `TESTING.md:12`。 |
| N2-54 | `native/src/terminal/ghostty_terminal/snapshot_cache_unit_tests.rs:3-18` | 三个同义反复测试。被测函数（`internal.rs:110-112`）就是 `grid_dirty \|\| !has_cache`，每个测试复述其中一个子句。把实现改成 `grid_dirty && !has_cache` 只有一个测试会失败，且没有任何测试触及缓存路径本身。违反 `TESTING.md:14`。 |
| N2-55 | `cucumber/steps/TerminalLaunchSteps.kt:37-42` | 步骤 `它渲染在 Compose 布局上层` 只做 `assertIsDisplayed()`，与同场景前一个步骤（`CommonSteps.kt:26-35`）逐字等价。没有任何 z 序 / `boundsInRoot` / 兄弟顺序断言 —— 把 SurfaceView 挪到 Compose 布局**后面**该场景仍绿。对应 `terminal-launch.feature:12-16`（REQ_ANDR_004）。 |
| N2-56 | `cucumber/steps/SearchSteps.kt:55-59`、`:87-90` | 场景 `输入法不遮挡搜索栏`（REQ_SEARCH_002）从不验证输入法是否真的弹出（`softKeyboardOpens()` 只点一下 + `waitForIdle()`），最终断言的 `SearchTextField.assertIsDisplayed()` 已被 Given 与上一个 Then 断言过。对照 `benchmark/…/InteractionAnimationBenchmark.kt:72` 的 `check(isImeShown(device))`。 |
| N2-57 | `cucumber/steps/SessionSteps.kt:97-100` | 步骤捕获的 `buttonText` 参数被丢弃，改用固定 testTag。feature 传的是 `"新建会话"`，把可见文案改成 `Add Session` 场景仍绿。 |
| N2-58 | `benchmark/…/InteractionAnimationBenchmark.kt:46-47` | `waitForControlKey(device)` 返回可空，`controlKey?.click()` 在找不到 CTRL 时**静默跳过**，测得 0 帧仍算成功。同文件 `:72` 的 IME 分支恰恰有 `check(isImeShown(device))` 并附注释说明原因。 |
| N2-59 | `scripts/test-emulator.nu`（全文 18 行） | `TextSearchEndToEndTest.kt:223-225` 与 `:451-454` 两处注释声称本脚本会对截图调用 `rapidocr`，脚本内无任何 `rapidocr` 调用（`flake.nix:144` 只在 shellHook 下载模型）。`TESTING.md:19/26/34` 依赖 OCR 验证。 |
| N2-60 | `scripts/setup-emulator.nu` | 全仓零引用（`build.yml:57-69` 用 `reactivecircus/android-emulator-runner@v2` 代替）。且命名违反 `STYLE.md:8`（kebab-case vs 要求的 snake_case），`:65`、`:69` 调用 `sdkmanager` 违反 `BUILD.md:6`。死脚本。 |
| N2-61 | `native/src/terminal/mock_pty.rs:51-64` | `MockPtyHandle::inject_output` / `drain_written` 在 `mock_pty.rs` 之外零调用方；因 `terminal/mod.rs:26` 的 `pub use` 而无法被 clippy 判死。死测试脚手架（`STYLE.md:63`）。 |
| N2-62 | `ffi.rs:921-936` | `writeKey` 的 `has_text` 分支不可达：两个调用方都传 `text = null`（`Bridge.kt:540,551`），`ffi.rs:938-942` 注释描述的「IME 可打印字符回退入口」未接线。 |
| N2-63 | `Bridge.kt:534-555` | 默认 `KeyboardMode.Raw`（`TYPE_NULL`）路径下，`unicodeChar == 0` 的非字母可打印键被静默丢弃：`:538` 的条件因 `TerminalInputEncoder.kt:86` 在 `unicodeChar <= 0` 时返回 `null` 而不可达，`:546` 的 A–Z 兜底不覆盖空格/数字/标点。 |
| N2-64 | `internal.rs:1619-1625` | `grapheme_extra` 只有 7 槽，`.take(7)` 静默丢弃第 8 个及之后的码点。极罕见且仅影响渲染，列为 P2。 |

---

## 四、本轮的正面结论（经核实为健康）

1. **原生 BDD 套件真的在跑且真的会失败**：7 feature / 12 scenario / 58 step 全通过；
   `cucumber-0.23.0/src/lib.rs:288-292` 的 `run_and_exit` 在失败时退出 101（已实测验证，
   推翻了「cucumber 失败也退 0」的假设）。
2. **Cucumber `ObjectFactory` 注册正确**：
   `META-INF/services/io.cucumber.core.backend.ObjectFactory` 指向
   `SimpleHiltObjectFactory`，`app/build.gradle.kts:47-49` 用
   `CucumberAndroidJUnitRunner` + `notClass` 使 `@CucumberOptions(features = ["features"])`
   解析到 `assets/features/`。
3. **全仓无 `@Ignore` / `@Disabled` / `#[ignore]` / `Assume.*`**（穷尽检索）——
   `TESTING.md:9/11` 当前成立。N0-16 是「未来会被静默破坏」而非「现在已被破坏」。
4. **`native/src/render/tests.rs:397-399`、`:2170-2172` 在缺少 GPU 适配器时大声 panic**，
   正是 `TESTING.md:12` 要求的「失败而不是跳过或忽略」。
5. **bench 断言测试有真实阈值**：`ghostty_terminal/tests.rs:1275-1280`（打字 < 6ms/键）、
   `:1314-1319`（批量 > 4000 单元/秒）、`render/tests.rs:1913-1917`（上传 > 350 MB/s），
   且随 `cargo test --workspace`（`check-rust.nu:8`）执行。
6. **三个 bench 都用 `std::hint::black_box`**，且 `no-criterion-black-box` 规则实际生效。
7. **无任何 `=x.y.z` 精确版本依赖**（三个 `build.gradle.kts` + 两个 `Cargo.toml` 穷尽检索），
   `DESIGN.md:5` 满足。
8. **`cargo machete --with-metadata` 在 CI 中运行**（`check-rust.nu:6`），
   `proptest` 与 `shuttle` 均被真实使用。
9. **`no-hardcoded-paths` 规则有效**（推翻「正则匹配不到 `/data/<pkg>/files`」的假设：
   `/data/[a-zA-Z0-9._]+/files` 在第二个 `/data` 偏移处命中，实测有发现）。
10. **`BUILD.md:18-22` 配置全部匹配**：`VERSION_17`、`compileSdk=37`、`minSdk=33`、
    `targetSdk=28`、`versionCode=2000`、`versionName="0.1.0"`、`applicationId="com.termux"`、
    `abiFilters` 仅 `arm64-v8a` + `x86_64`、debug 与 release **均**用 `aosp-testkey.p12`
    且无 debug 签名回退、`rust-version = "1.98"`、`edition = "2024"`、两个 manifest 均无 `[features]`。
11. **基线配置文件真实且已跟踪**：`baseline-prof.txt` 与 `startup-prof.txt` 均在 git 中。
12. **OSC-52 无注入面**：`output_processor.rs:128-142` 拒绝选择名中的 `;`、BEL、ESC
    并限长 64 字节；`session.rs:584-602` 用 STANDARD base64 发出；
    `emit()` 还要求两个 `;` 才应答，畸形请求原样透传。
13. **OSC-52 请求↔应答的会话配对正确**：`ffi.rs:1173-1199` 在注册表内克隆 `Arc<Session>`，
    即便用户中途切会话也写回原会话；`NEXT_REQUEST_ID` 单调递增，会话销毁后 id 不复用。
14. **CJK 退格按码点一对一**：`deleteSurroundingText` 钳到
    `maxOf(composingBuffer.length, 4096)`，用 `codePointBefore`/`charCount` 保持
    `composingBuffer` 对齐，每删一个码点恰发一个 `0x08` —— 不会切开宽字符也不会重复删除。
15. **选区安装已保序**：`installed_bounds`（`internal.rs:2002-2014`）先按 `Forward` 排序，
    且 ghostty 的 `Selection.order`（`Selection.zig:206-223`）对乱序输入有处理，
    倒置区间不会使上游 panic；列在 `absolute_point`（`internal.rs:1983`）钳位。
16. **`InputBatchBuffer` 内部的大写入次序正确**（`InputBatchBuffer.kt:48-67`）：
    超容量块先把常驻字节排到 `toSend[0]` 再排自己。
17. **`close()` 用 `shutdown()` 而非 `shutdownNow()`**（`InputBatchBuffer.kt:107-120`），
    末次 flush 仍会执行；`drainLocked` 在 `lock` 内原子拷出并清空，部分 flush 不会丢内容。
18. **按键上下对称**：`Bridge.processKeyEvent` 对非 `ACTION_DOWN` 返回 `false`（`Bridge.kt:508`），
    `TerminalSurface.onKeyUp`（`:2396-2402`）刻意交给 `super` —— 无卡死修饰键。
19. **组字文本确实被送进 VT/PTY**（经 `TerminalSurface.kt:660-671` 的增量 diff），
    不只显示在输入法候选窗；`commitText` 对 `composingBuffer` 去重（`:705`），
    故 `setComposingText` + `commitText(同文本)` 不会双插。
20. **无有损 UTF-8 往返**：`feedPty`/`feedTerminal` 取原始 `jbyteArray`（`ffi.rs:796-816`），
    `TerminalInputEncoder.encodeCommittedText` 按码点迭代。
21. **CI 确实在跑三道脚本**：`check.yml:40-43` 依次执行 `check-rust.nu`、
    `build-android-libs.nu`、`check-gradle.nu`。N0-16/17/18 的性质是
    **规则惰性**，不是**门禁未跑**。

---

## 五、需要用户裁决的问题

1. **`.semgrep/*` 是保护文件**，N0-17（改 7 条规则的
   `languages`）、N0-18（删一行 `pattern-not-inside`）、N2-52（`detekt.yml` 重新启用
   4 条吞异常规则）全部需要明确授权。是否授权？其中 N0-17 与 N0-18 各只改 1 行 / 1 处配置。
2. **`ktlint` / `ktfmt`（N1-29）**：删除插件（需改 `build.gradle.kts`，受保护）
   还是接入 `check-gradle.nu`（需改 `scripts/*.nu`，受保护）？
3. **粘滞 SCROLL 的产品语义**（N1-25）：Termux 是「再按一次解除」还是「任意输入即解除」？
   本仓 `onUserInputForScrollSnap` 的实现与注释倾向前者不成立，但这是产品决策。
4. **bracketed paste（N2-48）是否在范围内**？`docs/specification/` 未提及。
   在范围内则两条粘贴路径都要加 `\e[200~…\e[201~`；不在范围内则删掉死参数。
5. **输入法抑制窗口的语义**（N1-24）：抽屉关闭动画期间**是否应当**吞掉输入法输入？
   若否（推荐），则需拆开触摸抑制与输入法抑制。
6. **`AnrWatchDog`（5s 主线程无响应即杀进程）与 `PROHIBITED.md:10` / `DESIGN.md:192`
   的不相容** —— 第 9 轮 D3、第 11 轮 Q6 两次未决，仍待裁决。

---

## 六、修复顺序建议

1. **N0-15 选区行钳位** —— 纯 Kotlin、纯正确性、命中 DESIGN 核心功能、错误重复四处。
2. **N0-17 / N0-18 两条门禁** —— 先取得授权；每条 1 行至 1 处配置，
   收益是恢复 `TESTING.md:9/11/12` 与 `STYLE.md:60/61`、`BUILD.md:13` 的**唯一**执行者。
3. **N1-24 / N1-27 输入法丢字与乱序** —— 两条都在中文输入下可直接观察到。
4. **N1-25 硬件键盘 SCROLL** —— 一行调用。
5. **N1-26 粘贴主线程阻塞**、**N1-28 像素断言名不符实**、**N1-29 ktlint 死配置**。
6. **N2-52 / N2-53 重新打开被全局关闭的正确性规则**（需授权）。
7. **N2-54 / N2-55 / N2-56 / N2-57 / N2-58 同义反复或名不符实的测试** ——
   `TESTING.md:8/14` 要求删除或重写。
8. 其余 N2 项。

---

## 七、收敛状态

- 本轮**不是**「无新问题」的一轮：新增 **4 个 P0、6 个 P1、18 个 P2**，
  外加 **21 组经核实的正面结论**（其中 5 条是对既有假设的**推翻**）。
- 「连续五次无新问题」计数**第一次归零**，从第 12 轮重新开始。
- 第 6–12 轮七轮全部产出新 P0，规律完全一致：**换一个审读维度就立刻在该维度未被覆盖的位置产出 P0**。
  本轮换到**输入/IME 链路**与**门禁有效性** —— 前者产出 N0-15/N1-24/N1-27，
  后者产出 N0-16/N0-17/N0-18。
- 累计（第 6–12 轮）：**15 个 P0、39 个 P1、约 95 个 P2/P3**。
- **本轮首次出现「门禁本身的缺陷」这一类 P0**（N0-16/17/18）。
  前十一轮的结论一直是「自动化全绿但有 P0」，本轮定位到**其中三个原因就是门禁规则本身是死的**。
  这比新增若干运行时缺陷更重要：它解释了此前十一轮缺陷密度高的结构性成因。
- 仍然建议先修复再复审。理由不变，且本轮新增一条：
  在 `no-prohibited-shells` / `no-allow-in-prod` 这类规则恢复之前，
  「CI 全绿」这一信号本身不具备信息量，复审的收敛判据也因此被削弱。
- 本轮任务限定「不实际修改代码」，上述内容以文档形式留存，等待授权后按第六节顺序执行。
