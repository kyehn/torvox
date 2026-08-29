# Proposal: terminal-ux-v5-polish — 4 大用户感知缺陷深度诊断与像素级抛光

## Why

用户 2026-08-29 报告 4 项 P0 缺陷（CJK 字体错乱/模糊/极慢、IME 跳跃闪烁、切应用黑闪、滚动方向错误与卡顿）与 `polish-v4` 文档“已闭环”声称不一致。代码审计发现：4 项核心修复已进入 `main`（CJK 32 惩罚/token 边界/多数投票/LRU、IME hybrid 48ms、`surfaceCreated` 0 守卫、滚动 `floor`），但缺乏端到端可复现证据，且存在 2 处可验证瑕疵——

1. **构建产物缺失**：`app-release.apk 7.3M` 无 `.so`（`jniLibs` 空），渲染黑帧导致“宋体模糊”体感。
2. **IME 重组开销**：`animateDpAsState spring` 每帧触发 `TerminalScreen` recomposition，`offset` 虽放置阶段但组合已执行，SwiftShader 上贡献 jank（曾 Missed 74）。

本提案以**测量驱动**重做验证与精修：不改 CJK 核心逻辑（已正确），IME 去 spring 零重组，补构建门禁与日志审计自动化，模拟器 90fps+ 可复现，后端确定性 99%+ pass。

## What Changes

### 1. `cjk-font-fidelity` — 构建门禁 + 日志审计（P0，不改核心）

- 补 `scripts/build-android-libs.nu` → `jniLibs/arm64-v8a,x86_64/libnative.so` 20M±，`readelf --dynamic` 无 `ghostty` NEEDED 门禁，`scripts/build-apk.nu` 产出 90M+ APK 含 `.so`
- 日志审计自动化：`FALLBACK_CANDIDATE -15 vs -47`、`FALLBACK_HIT Noto Sans CJK`、`FONT_RASTERIZE_ASCII` 校验脚本

### 2. `ime-smooth-follow` — 零重组动画（P0，唯一代码改动）

- `TerminalScreen.kt`：去 `animateDpAsState spring`，动画期 `offset` 改为**内联读取** `WindowInsets.ime.getBottom` 于 placement lambda 内，零组合、零重组；系统 `WindowInsetsAnimation` 已插值，无需二次 spring
- 保留 `LaunchedEffect delay 48ms (16ms×3)` settled 判定，`isImeSettled` 切换时单次 `padding` + `surface.onImeSettled` → 单次 `applyGridResize` → `reconfigure_swapchain` 原地

### 3. `app-switch-continuity` — 审计与产物验证（P0）

- 审计 `surfaceCreated 0-size` 守卫、`ON_RESUME` 双清、`context.rs RECONFIGURE_SWAPCHAIN`，`release_surface` 时序
- 以 `jniLibs` 填充后的 90M APK 为验证基线，HOME 往返 3 次 0 黑

### 4. `scroll-physics` — 证据补齐（P0）

- 方向保持 Termux parity（`floor` 对称已落地），补 `ScrollBehaviorQuantifiedTest` 复跑与 `seq 1 400` 手势录屏，以可视化证据判定“反向”体感

### 5. 依赖与文档（P1）

- `cargo update` patch 小版，`flake.nix fenix stable` 校验，`android/build.gradle.kts` compose BOM 2026.08.00 / kotlin 2.4.10 审计
- 产出 `docs/plans/2026-08-29-*.md`、`docs/verification/2026-08-29-*.md`，刷新 `openspec/changes/terminal-ux-v5-polish/*`

## Capabilities

### New Capabilities

- `cjk-font-fidelity` — 构建门禁 + 日志审计闭环，设置首项 == 渲染首项可自动化断言
- `ime-smooth-follow-zero-recomp` — 零重组 pan，Missed 0

### Modified Capabilities

- `font-fallback` — 门禁化
- `ime-animation` — 48ms settled 保留，去 spring
- `surface-lifecycle` — 产物验证
- `gesture-scroll` — 证据化

## Impact

- 4 项 P0 以**测量为准**闭环，`jniLibs` 填充后 APK 可直接在模拟器像素级验证
- 无破坏性 API，`font_info` JSON 保持
- 风险：去 spring 后 stepped 报告如显阶梯可加 `derivedStateOf` 插值（系统动画已插值，实测如无阶梯则保持零插值）

## Alternatives Considered

- 重写 CJK 核心：否，已正确，改动风险大于收益
- 退回 `adjustResize`：否，`adjustNothing + WindowInsetsAnimation` 为官方推荐，`adjustResize` 缩放压扁更重
