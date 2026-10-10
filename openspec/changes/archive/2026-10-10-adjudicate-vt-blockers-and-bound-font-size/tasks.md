# 任务：裁定 VT 上游缺口、按可用列数收敛字号上界、补齐文档提供者树 URI 契约

## 一、VT 上游缺口裁定

- [x] 实测 18 例不采纳语料，逐个取得期望与实得屏幕
- [x] 定位上游源码中的分发位置（`src/terminal/stream.zig` 的 `'g'`、`'A','k'`、
      `params.len == 1` 守卫、中间字符 warn 分支）
- [x] 订正 `vt_conformance.rs` 的 `NOT_ADOPTED` 理由：改为实测根因并注明缺口层
- [x] 新增 `native/tests/vt_upstream_blockers.rs` 以特征化断言钉住缺口（7 项）
- [x] `upstream-alignment` 规范补充「理由须可复核到源码位置」与「上游缺口以特征化
      断言钉住并作为重新裁定触发器」

## 二、字号上界按可用列数收敛

- [x] `SettingsRepository.fontSizeForColumns` 抽出列↔字号比例（默认字号与上界共用）
- [x] `fontSizeMaxSp(spToPxScale, screenWidthDp)` 增加 `MIN_USABLE_COLUMNS` 约束，
      与 Termux 像素上限取紧者
- [x] 四个调用点改传屏幕宽（设置页、运行期钳位、预览、捏合上限）
- [x] `FontSizeRangeTest` / `TerminalSurfaceLogicTest` / `CoerceSpToPxScaleTest` /
      `FontSizeReflowInstrumentedTest` 按新上界改判（断言设备真实结果，不复述公式）
- [x] `font-selection` 规范改写上界定义与低密度场景

## 三、DocumentsProvider 树 URI 契约

- [x] 插桩用例 `tree_uri_modify_copy_move_round_trip`：覆写、复制、移动逐段断言
- [x] JVM 用例补目录树复制/移动、冲突唯一名与 docId 一致性、家内链接 inode 语义、
      站外链接拒绝
- [x] `documents-provider` 规范新增「变更操作按树 URI 客户端契约覆盖」

## 四、验证

- [x] `cargo fmt --check` / `clippy --deny warnings` / `machete` / `doc` 全净
- [x] `cargo test --workspace`：533 + 2 + 59 + 7 全通过
- [x] `./gradlew testDebugUnitTest detekt spotlessCheck lintDebug lintVitalRelease
      dokkaGenerate assembleDebug compileDebugAndroidTestKotlin` 全通过
- [x] `openspec validate --all` 23 项通过、markdownlint 0 issues
