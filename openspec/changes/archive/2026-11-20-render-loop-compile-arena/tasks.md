# 任务

## 一、渲染循环按职责拆分

- [x] 抽出唤醒门控（`awaitRenderWake`）、选区推送（`pushSelection`）、视口滚动推送
      （`pushScroll`）、渲染发布（`renderFrame`）、表面失效处置
      （`handleSurfaceInvalidated`）
- [x] 抽出帧计时（`recordFrameTiming`）、新输出消费（`consumeNewOutput`）、渲染失败
      处置（`handleRenderFailure`）、渲染恢复（`noteRenderRecovery`）、输出泵送
      （`pumpOutput`）、诊断发布（`publishDiagnostics`）、循环计时（`recordLoopTiming`）
- [x] 跨帧状态收进 `RenderLoopInstrumentation`／`ViewportSyncState`／`LoopFrameMarks`
      三个每线程对象（逐帧复用，无每帧分配）
- [x] 保留渲染循环外层的 `runBlocking`：拆分后判据已达成（实测 0 条），协程包装
      不是 arena 越线的原因，去掉它只会额外改动关闭路径的中断语义

## 二、规范

- [x] 校正「`Compiler allocated` 的数字」口径：编译期 arena 工作内存，非编译产物字节数
- [x] 补记 2026-11-15 那次处置未生效的证据（2026-11-20 日志同一方法仍 4232KB／4972KB）
- [x] 新增「渲染循环的待编译方法必须保持小体量」要求（含 arena 越线与口径两个场景）
- [x] 登记 2026-11-20 12-45-04 日志的聚合归因（应用进程 W/E 70 条逐条处置、首帧与
      设备丢失时序、编译 arena 六条）

## 三、验证

- [x] `android/gradlew :app:compileDebugKotlin :app:detekt`：通过
- [x] `android/gradlew :app:spotlessCheck :app:testDebugUnitTest`：通过
- [x] 模拟器 release 构建：`logcat` 中本应用的 `Compiler allocated` 行数为 0
      （改动前为 4520KB／4739KB 两行）
- [x] 模拟器 debug 构建：同上（改动前为 5787KB／6019KB 两行；现仅剩平台方法
      `android.view.ViewRootImpl.performTraversals` 一条，不在判据内）
- [x] 模拟器实机截图确认终端正常出帧
- [x] `openspec validate --all --strict`：通过
- [x] `semgrep scan --error --config .semgrep/rules.yml`：0 findings
- [x] `markdownlint-cli2 "**/*.md"`：0 issues
