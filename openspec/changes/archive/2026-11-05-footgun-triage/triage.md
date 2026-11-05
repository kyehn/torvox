# 扫描命中判定与 Rust 复核

本留档把两件事写死：**扫描器的每一处命中都有判定**，**本会话的 Rust 改动经复核**。
判定结论都可复核：命中位置是文件与行号，判据来自陷阱文档，结论来自现场代码。

## 一、footgun-scan 扫描范围与结果

- 工具：`footgun-scan`（`scripts/scan.py`，32 个模式）。
- 范围：`--all android/app/src/main`（81 个文件）与
  `--all android/app/src/androidTest`（80 个文件）。
- 结果：主源码集 10 处命中（全部为 `look`），仪器测试源码集 0 处命中。
- 说明：10 处命中全部是「形状」提示，是否构成缺陷按各自的陷阱文档判定。

## 二、逐条判定

### 2.1 `job-relaunched-without-cancel`（3 处）

陷阱：作业字段重新启动前必须取消，否则旧协程继续跑。

| 位置 | 字段 | 赋值点 | 判定 |
| --- | --- | --- | --- |
| `TerminalViewModel.kt:986` | `fontLoadJob` | `viewModelScope.launch(TerminalDispatchers.inputOutput)` | 驳回 |
| `monitor/RenderWatchDog.kt:48` | `watchJob` | `scope.launch { watchLoop(ownGeneration) }` | 驳回 |
| `runtime/TerminalRuntime.kt:1392` | `renderMonitorJob` | `scope.launch { … }` | 驳回 |

判定依据（逐字段核实赋值点唯一性）：

- `fontLoadJob`：`TerminalViewModel.kt:985` 的 `fontListLoaded || fontLoadJob?.isActive == true`
  早退与赋值在同一条语句序列上，活跃作业存在时不会二次启动。
- `watchJob`：`RenderWatchDog.kt:45-49` 把「检查是否活跃」与「启动」放在**同一个**
  `synchronized(fireLock)` 临界区内；两个并发调用者（切会话与 `checkSessions` 的重启）
  不会各跑一个 2s 轮询循环。`stop()`（52-65）推进代次并清空字段。
- `renderMonitorJob`：`TerminalRuntime.kt:1390-1398` 同样是「检查 + 启动」同临界区，
  `stopRenderMonitor()`（1401-1406）取消并置空。

三者都是「cancel-before-relaunch」的等价形态（活跃即不重启），不是「赋值前不取消」的
泄漏形态。陷阱文档指认的泄漏形态在此不成立。

### 2.2 `native-load`（1 处）

位置：`bridge/NativeBridge.kt:44`（`System.loadLibrary("native")`）。

判定：**驳回（陷阱形态不适用）**。陷阱讲的是 JNA：同名系统副本可能被抢先加载，
因此必须打印「实际打开的文件路径」。本仓走 Android 的 JNI `System.loadLibrary`：
类加载器的搜索路径只含应用自身的 native 库目录，不存在系统同名副本兜底加载的场景，
因而没有「实际打开路径」可打。失败路径已打印 `UnsatisfiedLinkError` 全文（47-49 行），
其中含平台给出的 ABI 缺失提示——这正是本仓需要的那条诊断。

### 2.3 `run-blocking`（2 处）

陷阱：`runBlocking` 走到主线程会卡住 UI。

| 位置 | 判定 | 依据 |
| --- | --- | --- |
| `installer/BootstrapInstallService.kt:59` | 驳回 | `install()` 由 `onStartCommand` 里显式 `Thread { … }.start()`（37-48 行）调用，全程在后台线程 |
| `runtime/TerminalRuntime.kt:1637` | 驳回 | `runBlocking` 在渲染线程的 `Thread { … }` 体（1634 行）内，不在主线程 |

两处都不是「构造路径上的阻塞资源读取」——陷阱指认的最贵形态（属性初始化器里
`runBlocking`）在此不成立。

### 2.4 `pointer-input-unit`（2 处）

陷阱：`pointerInput(Unit)` 会把首次组合时的捕获值固定下来。

| 位置 | 判定 | 依据 |
| --- | --- | --- |
| `ui/ModifierBar.kt:732` | 驳回 | 键位手势的全部回调都经 `rememberUpdatedState` 读取最新值（728-730 行的 `currentOnClick` / `currentOnRepeat` / `currentSecondaryAction`），块里没有裸捕获的可变标志 |
| `ui/SettingsScreen.kt:125` | 驳回 | 块体是 `detectTapGestures(onTap = {})`，不读任何状态 |

`ModifierBar.kt` 用 `Unit` 作键是**有意为之**，且注释（725-727 行）写明了理由：改用
lambda 实例作键会在长按期间的任何重组上取消并重启手势协程，静默杀死自动重复。
回调的「始终是最新值」由 `rememberUpdatedState` 保证——这正是陷阱文档认可的等价形态
（标志若是块自己持有的 `mutableStateOf`，`Unit` 本就安全）。

### 2.5 `slider-value-range`（2 处）

陷阱：滑块的 `valueRange` 必须是「控件会被交给的所有值」的断言；越界值只钳拇指不钳
读数，首次触摸会把区间端点写回。

| 位置 | 判定 | 依据 |
| --- | --- | --- |
| `ui/SettingsScreen.kt:544`（`FontSizeSlider`） | 驳回 | 传入滑块的值是 `sliderFontSize`，初值经 `SettingsRepository.clampFontSize` 收敛（285-288 行），并在区间收紧时由 `LaunchedEffect(fontSizeUpperBound)` 再次收敛（294-297 行） |
| `ui/SettingsComponents.kt:122`（通用 `SettingsSliderRow`） | 驳回 | 该组件只透传调用方给的区间；唯一调用方即上条，值已在区间内 |

本仓已经把这条陷阱写进了实现：`SettingsRepository.clampFontSize`（193-194 行）的
文档注释逐字描述了该陷阱的场景（「宽屏存下的 96sp 旋转到 360dp 竖屏后上界只剩
30sp……首次触摸会按指针位置换算出区间端点写回」），并规定「调节条、捏合预览、落盘值
与设置页展示都经这一处」。扫描器命中的是**已经处理过**的形态。

## 三、本会话 Rust 改动的复核

复核范围：提交 `82863168`（`refactor(test): 收敛仪器测试与 Rust 用例的重复夹具`）
的 Rust 侧改动——`native/src/terminal/session/tests/{mod,clipboard,write_backpressure}.rs`、
`native/src/terminal/test_helpers.rs`、`native/benches/cell_builder.rs`。

工具：`cargo fmt --all -- --check`（通过）、`cargo clippy --offline --all-targets
-- -D warnings`（通过）、`cargo clippy --all-targets --all-features -- -W
clippy::pedantic`（本批改动所在文件仅 5 处告警，全部是改动前就有的
`cast_possible_truncation`，位置为 `cell_builder.rs:50-51` 与
`test_helpers.rs:77,78,162`）、`cargo test --offline`（801 passed）。

判定：本仓是**应用**（cdylib + lib，非发布的库 crate），故按「简洁优先」而非「库的
API 严格性」判定。

### Must fix

#### 1. 共享夹具丢掉了持锁线程的 panic 断言（已修）

**规则**：测试断言不得在收敛时被弱化；析构函数中不得 panic（rust-review：
"Destructors that panic, block, or can fail"；Effective Rust：析构期间 panic 会在
展开中二次 panic 而 abort）。

**问题**：两份拷贝原本都以 `user_write.join().expect("用户写入线程 panicked")` 收尾。
收敛成守卫后，join 发生在 `Drop` 里，被降级成静默 `let _ = holder.join()`：持锁线程
panic 时用例继续跑并可能判绿，测试从「钉住行为」退化为「看起来跑过」。

**修法**（`native/src/terminal/session/tests/mod.rs`）：

```rust
impl PtyWriteLockHold {
    /// 放行持锁线程并回收它；持锁线程 panic 即判红。
    fn release(mut self) {
        self.signal_and_join(true);
    }

    fn signal_and_join(&mut self, strict: bool) {
        if let Some(release) = self.release.take() {
            let _ = release.send(());
        }
        if let Some(holder) = self.holder.take() {
            if strict {
                holder.join().expect("用户写入线程 panicked");
            } else {
                let _ = holder.join();
            }
        }
    }
}

impl Drop for PtyWriteLockHold {
    fn drop(&mut self) {
        self.signal_and_join(false);
    }
}
```

两个调用点改为显式 `hold.release()`：判红回到显式路径，`Drop` 只作兜底（忘记放行
不再拖死进程，兜底路径也不 panic）。

### Should fix

无。

### Consider

1. **`Option` 字段只为析构里 `take()`**（`session/tests/mod.rs`）。替代方案是
   `ManuallyDrop` 或自引用结构体，复杂度更高；`Option` 是惯用解，保留。
2. **`hold_pty_write_lock(&Arc<Mutex<Session>>, …)` 收的是 `&Arc`**。调用方之后仍要用
   同一个 `Arc`（`session.lock()`），按引用传入是必要的；clippy 无异议。
3. **`isImeVisible(activity)` / `hideImeAndConfirm(activity, …)` 用 `&Activity` 而非
   `AndroidComposeTestRule` 扩展函数**。两者需要的只是 activity，扩展函数会多绑一个
   用不上的接收者。
4. **仓库整体仍有约 335 条 `clippy::pedantic` 告警**，分布在本批改动之外
   （多为 `cast_possible_truncation` 与文档类）。`[lints.clippy]` 表目前只放行两项
   （`too_many_arguments`、`type_complexity`）。收紧 pedantic 需要逐条甄别，不是本轮
   的范围；此处只记录事实，不改门禁。

### 已做得好

- 共享夹具把「持住的是**主端写入锁**（`pty_write_order`）而不是会话锁」这一决定成败
  的差别写进了注释与代码结构（`session/tests/mod.rs:101-106`），收拢后不会再有人
  把它改成会话锁——那样两条钉就都失去判别力。
- 守卫同时具备「显式放行判红」与「析构兜底不拖死进程」两段式，取掉了两个极端。
- Kotlin 侧把「输入法可见性是窗口级状态」这一口径统一到窗口 decor，并把「两处口径
  分叉会让『before 截图必须无键盘』的前提自相矛盾」写进注释
  （`TestUtils.kt` 的 `isImeVisible`），使后来者不会随手改回读 Surface 的 inset。

## 四、结论

- 扫描命中 10 处，逐条判定后**均不构成缺陷**：3 处作业泄漏形态有等价的活跃检查，
  1 处陷阱形态（JNA）不适用于 JNI，2 处阻塞在后台线程，2 处 `pointerInput` 键的选择
  有据且回调走 `rememberUpdatedState`，2 处滑块区间已由 `clampFontSize` 收敛。
- Rust 复核发现 1 处真实回退（丢掉的 join 断言），已修并验证。
