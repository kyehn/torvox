# 验证工具调研：Miri、ASan、Turbine（2026）

## 摘要

结论先行：

- **loom**：不采用。loom 要在测试构建里替换生产原子量，切换只能靠 crate
  feature 或自定义 `--cfg`：前者违反「不得定义任何 crate features」，后者只是
  绕过 Semgrep 门禁。相关不变量改由标准库线程用例直接覆盖生产类型。
- **Miri**：已可运行，但只能覆盖纯 Rust 单测。库加载时的 PNG 解码器
  `.init_array` 会调用 FFI，Miri 必须跳过它（`cfg(miri)`）。目前验证面为
  `terminal::output_processor` 的 25 条测试。
- **ASan**：已在本机（x86_64 host）跑通全部 952 条 `--lib` 测试。Android
  真机 ASan 需要 nightly、`libclang_rt.asan` 与 `wrap.sh`，且要改受保护的
  `scripts/`，本轮未做。
- **Turbine**：仓里早已在用（`SettingsRepositoryTest`），不需要新增依赖。

## 运行环境

| 工具 | 入口 | 说明 |
| --- | --- | --- |
| 默认 stable | `nix develop` | Android 交叉编译、日常测试 |
| nightly + Miri | `nix develop .#nightly` | 只装主机目标；复用默认 shell 的构建输入 |
| loom（不采用） | — | 需 crate feature 切换原子类型，规范禁止 |
| Miri | `MIRIFLAGS=-Zmiri-disable-isolation cargo miri test --lib terminal::output_processor` | 用 nightly shell |
| ASan（host） | 见下 | 用 nightly shell |

`rust-toolchain.toml` 固定 stable，但 nix shell 直接暴露 fenix 的 `cargo`，
不经过 rustup，故 nightly shell 内不会回落到 stable。

## loom（不采用）

loom 的建模要求在测试构建里把生产原子量换成 `loom::sync::atomic::AtomicBool`，
切换只能靠 crate feature 或自定义 `--cfg`：前者违反
`docs/specification/STYLE.md` 的「不得定义任何 crate features」，后者只是绕过
Semgrep 门禁，两者都不采纳。

被建模的不变量是**一次置位恰好被一个消费者取走**。该性质由
`AtomicBool::swap` 的原子性直接保证，现由
`terminal::output_processor::new_output_flag_is_consumed_by_exactly_one_consumer`
在生产类型上直接覆盖；事件队列侧的并发语义仍由 `tests/concurrency.rs`
（shuttle）穷举。

**未建模的**：渲染循环的驻留点 `RenderWake`（代数计数 + `parking_lot`
`Condvar`）。它用「代数变化才等待」消除丢唤醒：`wait` 在持锁状态下比较代数，
不等则立即返回；等待与通知之间不存在丢信号的窗口。这不是已知缺陷，而是
loom 够不着的原语（loom 不支持 `parking_lot` 的 `Condvar`）；要建模得先把
等待原语抽成可替换接口，属于重构而非测试。

## Miri

库的 `PNG_DECODER_INIT` 位于 `.init_array`，进程启动即调用
`libghostty_vt::set_png_decoder`。Miri 不能执行 FFI，所以在
`native/src/terminal/ghostty_terminal/mod.rs` 用 `not(miri)` 跳过这次安装；
纯 Rust 测试不触及 PNG 解码，跳过不影响验证面。

```sh
nix develop .#nightly --command sh -c \
  'cd native && MIRIFLAGS=-Zmiri-disable-isolation \
   cargo miri test --lib terminal::output_processor'
```

`-Zmiri-disable-isolation` 是 proptest 取 `getcwd` 所需。实测 25 条测试约 8
分钟——Miri 是解释执行，别放进常规 CI 的每提交路径。

**边界**：`ghostty_terminal`、`render`（wgpu）、`android::ffi`（JNI）都在
FFI 另一侧，Miri 验证不到；这些边界归 ASan 与真机仪器测试。

## ASan（host）

不能让 `RUSTFLAGS` 全局带 `-Zsanitizer=address`：proc-macro 与构建脚本会被
一起插桩，rustc 直接拒绝。用显式 target 加目标专属 rustflags，把插桩限制在
目标产物：

```sh
nix develop .#nightly --command sh -c \
  'cd native && \
   CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_RUSTFLAGS="-Zsanitizer=address" \
   ASAN_OPTIONS=detect_leaks=0 \
   cargo test --target x86_64-unknown-linux-gnu --lib'
```

实测全部 952 条测试通过（约 65 秒）。`detect_leaks=0` 是必要的：测试进程里
有意保留的全局状态会被 LeakSanitizer 当成泄漏。

**未做**：Android 真机 ASan。它需要
`libclang_rt.asan-aarch64-android.so` 随 APK 打包、`wrap.sh` 设置
`LD_PRELOAD`、以及 nightly 交叉编译；构建入口在受保护的
`scripts/build-android-libs.nu`，且必须真机验证，本轮没有条件做。

## Turbine

`android/app/build.gradle.kts` 早已声明 `app.cash.turbine:turbine:1.2.1`，
`SettingsRepositoryTest` 已用 `.test { awaitItem() }` 断言 DataStore 的发射
序列。无需新增依赖或改造测试。

## 已有但未在本轮扩展

- `tests/concurrency.rs` 用 **shuttle** 做随机调度探索，覆盖事件队列的并发不变量。
- `proptest` 覆盖分块/等价类；`criterion` 承担基准。
- 解析器 fuzz（`SYMLINKS.txt`、`fonts.xml`、shell entry 切分）可接
  `cargo-fuzz`，但当前 proptest 已覆盖主要边界，优先级低于真机 ASan。
- TSan：Rust 侧需 nightly 且 Android 支持差，不建议。

## 后续建议

1. Android 真机 ASan：先取得 `scripts/` 修改同意，再做一次真机仪器测试。
2. 新增原子协议时，在对应模块补一条直接覆盖生产类型的不变量用例；不要为
   `std::sync::Mutex` 队列补并发模型（模型里只是顺序逻辑）。
