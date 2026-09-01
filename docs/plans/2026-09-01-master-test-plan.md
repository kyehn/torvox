# Master Test Plan v8

> 日期：2026-09-01 | 状态：活跃
> 关联：master-optimization-v8/tasks.md, OPENSPEC-STATUS.md

## 1. 测试分层

| 层 | 工具 | 覆盖范围 | 执行频次 |
|----|------|----------|----------|
| L1 Rust 单元测试 | `cargo test -p native --lib` | 全部 Rust 逻辑 | 每次提交 |
| L2 Clippy | `cargo clippy -- -D warnings` | 代码质量 | 每次提交 |
| L3 依赖审计 | `cargo machete` + `cargo audit` | 依赖干净 | 每次提交 |
| L4 Kotlin 单元测试 | `./gradlew :app:testDebugUnitTest` | JVM 逻辑 | 每次提交 |
| L5 Robolectric | Gradle Robolectric runner | UI 状态逻辑 | 每次提交 |
| L6 构建产物 | `.so` + APK 验证 | 二进制门禁 | 预发布 |
| L7 模拟器帧率 | `dumpsys gfxinfo` | 渲染性能 | 预发布 |
| L8 端到端 | ADB + `am start` + 录屏 | 功能验收 | 预发布 |

## 2. 详细测试用例

### 2.1 Rust 单元测试矩阵

| 模块 | 用例数 | 通过条件 |
|------|--------|----------|
| vt/conformance | 156 | 全绿，零 flaky |
| vt/property | 42 | 全绿 |
| render/tests | 187 | 全绿 |
| render/property | 31 | 全绿 |
| terminal/pty | 89 | 全绿 |
| terminal/output_processor | 24 | 含 OSC133 8 cases |
| render/cell_builder | 68 | 含 CellRun 5 cases |
| terminal/ghostty_terminal/mouse | 15 | 含 encode_mouse 5 cases |
| integration-tests | 47 | 全绿 |
| **合计** | **1017+** | **0 failed** |

### 2.2 验收测试（模拟器 ADB）

| 编号 | 测试 | 预期结果 | 自动化方式 |
|------|------|----------|-----------|
| E2E-01 | 冷启动 <2s | Activity 在 2s 内 visible | `am start` + `dumpsys activity` |
| E2E-02 | 帧率 90fps+ | 90th percentile <16ms | `dumpsys gfxinfo framestats` |
| E2E-03 | CJK 渲染 | 无□/无缺字 | 截图 + OCR 对比 |
| E2E-04 | SSH 连接 | 成功建立会话 | `adb shell` 命令验证 |
| E2E-05 | 文本选择 | 复制粘贴正确 | uiautomator dump |
| E2E-06 | Kitty 图片 | 图片正确显示 | 截图验证 |
| E2E-07 | 横竖屏旋转 | 无黑屏无崩溃 | config change 后 dump |
| E2E-08 | 后台切换 | 无崩溃无数据丢失 | app 切换前后状态 |

### 2.3 性能基准

| 指标 | 目标 | 采集方式 |
|------|------|----------|
| 冷启动时间 | <2s | `am start -W` |
| 渲染帧率 90th | <16ms | gfxinfo framestats |
| 渲染帧率 p99 | <25ms | gfxinfo framestats |
| 内存占用（空闲） | <100MB | `dumpsys meminfo` |
| 内存占用（滚动 10K 行） | <200MB | `dumpsys meminfo` |
| Rust 测试暖缓存 | <80s | `cargo test` |

## 3. 执行流程

```
L1-L3 → commit → L4-L5 → L6 → L7 → L8 → 预发布
```

每次提交前必须通过 L1-L3，L7-L8 在预发布时执行。

## 4. Flaky 检测

- 连续 3 次 `cargo test` 零 flaky 视为稳定
- 如果出现 flaky，标记并修复根因，不跳过
- CI 超时：cargo test 120s，Gradle 600s

## 5. 报告

每次验证后产出 `docs/verification/YYYY-MM-DD-{variant}-results.md`，包含：

- 通过/失败计数
- 帧率直方图（模拟器测试）
- 体积快照（.so/APK）
- 依赖清单快照
