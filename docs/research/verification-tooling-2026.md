# 验证工具调研：loom、Miri、ASan、Turbine（2026）

## 摘要

结论先行：

- **loom**：已落地一条真实协议测试（`new_output` 单次置位恰好消费一次），
  `--features loom` 构建，生产构建零开销。更高价值的并发风险在渲染循环的
  check-then-park，需要先有可注入的 parker 才能建模。
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
| loom | `cargo test --features loom --lib loom_tests` | 用 nightly shell |
| Miri | `MIRIFLAGS=-Zmiri-disable-isolation cargo miri test --lib terminal::output_processor` | 用 nightly shell |
| ASan（host） | 见下 | 用 nightly shell |

`rust-toolchain.toml` 固定 stable，但 nix shell 直接暴露 fenix 的 `cargo`，
不经过 rustup，故 nightly shell 内不会回落到 stable。

## loom

`OutputFlag` 在 `feature = "loom"` 下换成 `loom::sync::atomic::AtomicBool`，
共享句柄换成 `loom::sync::Arc`；其余构建就是标准库类型。

```sh
nix develop .#nightly --command sh -c \
  'cd native && cargo test --features loom --lib loom_tests'
```

只建模一个不变量：**一次置位必须恰好被一个消费者取走**。这覆盖
`swap(false, AcqRel)` 的原子性；生产代码只有一个写者（PTY 线程）和若干消费者
（渲染/诊断），不存在第二个 `store(false)`，所以这个不变量是完整的。

**没有建模的**：渲染循环的 check-then-park 丢唤醒。那条路径跨 `parking_lot`
的 `Condvar`/parker 与帧邮箱，loom 不支持这些原语；要验必须先把 park 抽成
可替换的接口，属于重构而非测试。

建模时的坑（写 harness 前必读）：C++ 内存模型允许**同线程** `store` 后的
`load` 读到并发线程**更晚**的 store（coherence 只要求不早于自己的 store）。
因此“store 后立刻 load 必须读到自己的值”是错误断言，loom 会如实报出那个
合法交错。断言只能写真正由 happens-before 保证的性质。

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

- `tests/concurrency.rs` 用 **shuttle** 做随机调度探索；与 loom 的穷举互补。
- `proptest` 覆盖分块/等价类；`criterion` 承担基准。
- 解析器 fuzz（`SYMLINKS.txt`、`fonts.xml`、shell entry 切分）可接
  `cargo-fuzz`，但当前 proptest 已覆盖主要边界，优先级低于真机 ASan。
- TSan：Rust 侧需 nightly 且 Android 支持差，不建议。

## 后续建议

1. Android 真机 ASan：先取得 `scripts/` 修改同意，再做一次真机仪器测试。
2. 若渲染掉帧/卡死复现，先把 park 抽成接口，再把 check-then-park 纳入 loom。
3. 新增原子协议时，按 `output_processor` 的模式补一条 loom 测试；不要给
   `std::sync::Mutex` 队列补 loom（模型里只是顺序逻辑）。
