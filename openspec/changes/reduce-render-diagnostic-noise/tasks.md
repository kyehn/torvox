# Tasks

## 1. 原生侧逐帧呈现诊断

- [ ] 1.1 `slow_log` 新增带计数的限频闸门（累计窗口内事件数，放行时交出），
      验证：窗口内事件被压住且计数不丢，首条可见
- [ ] 1.2 `Renderer` 持有该闸门；`render_frame_with_plan` 的呈现诊断改为
      汇总输出，`bands` 改称 `dirty_bands`，
      验证：输出间隔不小于 500ms，每行带窗口内呈现帧数与末帧形态

## 2. 原生侧热路径落点

- [ ] 2.1 `setScrollOffset` 的落点日志降为 trace（与 `setScrollYPx` 一致），
      验证：默认日志级别下不再出现，注释同步更新

## 3. 宿主侧帧节奏窗口

- [ ] 3.1 `FrameTimingStats` 加 500ms 输出间隔下限：窗口满而间隔未到时，
      样本环形覆盖、报告延后给出，
      验证：相邻报告间隔不小于 500ms，统计样本不丢
- [ ] 3.2 补单测：间隔未到时报告延后、间隔到时给出、覆盖后统计取最新样本、
      窗口迟迟不结算时报缺陷
      验证：改坏下限的用例能判红

## 4. 仪器化用例同步

- [ ] 4.1 `ScrollBehaviorQuantifiedTest` 的呈现帧计数措辞改为汇总行语义，
      验证：判定口径（滚动期确有呈现）不变

## 5. 集成验证

- [ ] 5.1 Rust：`cargo fmt --all -- --check`、`cargo clippy --all-targets -- -D warnings`、
      `cargo test --lib` 全绿
- [ ] 5.2 JVM：`:app:testDebugUnitTest`、`:app:detekt`、`:app:spotlessCheck` 通过
- [ ] 5.3 规范：`openspec validate --all --strict` 全绿

## Workflow follow-up

- 归档变更并同步 `openspec/specs/logcat-hygiene/spec.md`。
