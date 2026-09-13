# torvox 终端修复交付 — Environment & Ops Facts

> 先读索引，再按 ledger Current slice 引用的行取用。不要整 file 重读。
> Red line: 密钥与真实数据内容永不进 repo、日志与提交。

## Context index

| ID | Use when | Read | Verify |
| --- | --- | --- | --- |
| C-01 | 规范与红线 | docs/specification/BUILD.md,DESIGN.md,STYLE.md,TESTING.md 全文；AGENTS.md | 无（只读约束） |
| C-02 | lint 门禁 | android/app/lint.xml；android/app/build.gradle.kts | nix develop --command bash -c './android/gradlew -p android :app:lintDebug --stacktrace'，tee /tmp/lintDebug-<date>.log |
| C-03 | DocumentsProvider | android/app/src/main/AndroidManifest.xml:50-60；TerminalDocumentsProvider.kt；DocumentMutations.kt；DocumentQueries.kt | provider 单测 + SAF 实测 |
| C-04 | IME 与尺寸抖动 | ui/TerminalScreen.kt:492-523,804-818；ui/TerminalSurface.kt:372-532,2244-2270,2915-3004；native/src/android/ffi.rs attachWindow/setFontSizeInPlace；native/src/render/context.rs | logcat 无抖动 attach + 截图 |
| C-05 | 颜色与丢字 | native/src/render/cell_builder.rs:594-802；render/font（atlas/shaping/pipeline） | cargo test -p native + 截图/OCR |
| C-06 | 滚动与手势 | ui/TerminalSurface.kt:1743-1884,2672-2867；ui/ModifierBar.kt | 模拟器手势测试 + 截图 |
| C-07 | 黑屏与启动 | MainActivity.kt:330-350；ui/TerminalScreen.kt:234-267；runtime/TerminalRuntime.kt；TerminalApp.kt；monitor/BootGuard.kt | 后台前台循环 + 冷启动耗时 |
| C-08 | Rust 门禁 | Cargo.toml；native/Cargo.toml | nix develop --command cargo clippy --workspace；nix develop --command cargo test --workspace（首轮核实确切命令） |
| C-09 | GHA 与 kotlin 门禁 | .github/（只读）；android/detekt.yml；scripts/*.nu（只读可执行） | gh run list；detekt 确切命令首轮核实 |
| C-10 | 模拟器门禁 | android/app/src/androidTest；scripts/*.nu（只读） | 确切命令首轮核实并落 ledger |

Always-hot: `ledger.md` status/current slice + directives above the watermark.
On-reference only: 上表行、repo 标准、证据 artifact、archives。

## Build / test

- 环境一律 `nix develop` 进入；禁止 sdkmanager/包管理器装工具；禁止 cargo zigbuild，Android 构建用 cargo ndk。
- lint：见 C-02。高成本输出一律 tee 到 /tmp 并 tail 关键行。
- Rust/Kotlin/GHA/模拟器确切命令：R1 在 nix develop 内核实后写回本节（in place 更新，不追加历史）。

## Standards / conventions (the shared yardstick)

- 以 docs/specification/ 与用户提示为实际标准；openspec/specs 仅参考。
- STYLE.md：Nushell 禁 `||`；Kotlin 用完整描述性名称；简体中文注释文档；不实现未声明功能。
- AGENTS.md 编码节：禁止魔数；禁止缩写；生产代码禁 `#[allow]`；禁硬编码 /data/*/files 数据路径；Rust 用 std::hint::black_box；Kotlin 用 SharingStarted.WhileSubscribed(TIMEOUT_MILLIS) 具名常量。
- 库 crate 禁 anyhow（用 thiserror 2）；核心终端数据路径禁 unsafe；禁 bash/sh 脚本；禁逐单元格 Canvas.drawText。

## Credentials / secrets policy

签名用 android/app/aosp-testkey.p12（仓库既有）；密钥内容永不打印、存储、记录、提交。

## Data policy

模拟器与真机日志只保留脱敏聚合证据；真实用户数据不进 repo 与证据。

## Cost / resource notes (optional)

lint/模拟器是耗时门禁（分钟级），跑时并行做不依赖它的定位工作；诊断重跑同样计入耗时。
