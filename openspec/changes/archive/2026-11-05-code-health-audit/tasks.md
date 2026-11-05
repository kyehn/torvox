# 任务

## 一、重复率

- [x] 用固定参数（`--min-lines 12 --min-tokens 60`）扫描 `native/src`、`native/benches`
      与 `android`，给出扫描前基线：Rust 4 处 / 0.18%，Kotlin 9 处 / 0.36%。
- [x] 收敛 Kotlin 三处 shell 探针会话夹具为 `UxTestUtils.withShellProbeSession`。
- [x] 收敛 Kotlin 两处输入法可见性与「收起并确认隐藏」为
      `TestUtils.isImeVisible` / `hideImeAndConfirm`，并让
      `hideImeAndAwaitHidden` 复用同一实现。
- [x] 收敛 Rust 基准里断言与采样各写一份的 `CellInstanceConfig`。
- [x] 收敛 Rust 两个会话回归钉共用的「用户写入持住主端写入锁」夹具为
      `session/tests/mod.rs` 的 `hold_pty_write_lock`；前景/背景「精确 u8」断言的
      换算合并为 `exact_rgba`。
- [x] 复扫确认：Rust 0.03%（余 1 处）、Kotlin 0.25%（余 6 处 import 块）。
- [x] 保留项逐条写明理由（import 块、图标 XML、两个判别不同不变式的回归钉）。

## 二、外部依赖与 vendor

- [x] 逐个核对 27 个直接依赖的 crates.io 累计下载、近 90 天下载与最近发布日期，
      写进 `audit.md`。
- [x] 定下采信口径（累计 ≥ 100 万、近 90 天 ≥ 10 万、12 个月内有发布），并逐条
      判定；全部通过，`raw-window-handle` 作为生态基线类型豁免并写明理由。
- [x] 核对自实现部分是否该换成依赖：终端协议、字形簇、PNG 解码、字体集合、缓存、
      并发与序列化均已有 crate 或上游承担，无值得替换的自实现通用设施。
- [x] 核对 vendor：仓内无 `vendor/`、无 `.gitmodules`、无上游 Zig 源码副本；
      构建期拉取的上游源码不入库。

## 三、规约

- [x] `code-health` ADDED「重复实现必须收敛为共享夹具」。
- [x] `code-health` ADDED「直接依赖必须处于活跃维护状态」。

## 四、验证

- [x] `cargo clippy --offline --all-targets -- --deny warnings` 通过。
- [x] `cargo test --offline` 通过（801 passed）。
- [x] `cargo bench --offline --bench cell_builder -- --quick` 通过
      （floor 0.358ms / 上限 20ms）。
- [x] `./gradlew :app:compileDebugAndroidTestKotlin :app:spotlessCheck` 通过。
- [x] `./gradlew detekt` 通过。
- [x] `openspec validate --all --strict` 通过。
- [x] `markdownlint-cli2` 无 issue。
