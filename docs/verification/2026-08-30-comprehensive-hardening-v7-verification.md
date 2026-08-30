# 验证报告: comprehensive-hardening-v7

> 日期: 2026-08-30 | 关联: `docs/plans/2026-08-30-comprehensive-hardening-v7-detailed.md` + `...-test-plan.md`  
> 状态: 进行中（阶段 0-1 已验证，2-6 待模拟器重建后验证）

## 验证环境

- 模拟器: API 35 x86_64, 1080x2400, 420dpi, SwiftShader (lavapipe), 60Hz（系统上报 renderFrameRate 60.000004）
- 构建: `flake.nix` nix develop, zig 0.16, cargo ndk x86_64, gradle 8.14.4, JDK 21, `android/app/aosp-testkey.p12`
- 设备: emulator-5554, `adb` 已设置, `com.termux` 已安装
- Rust: `cargo test --lib` 1010 tests, 10 ignored, 0 failed（nix develop, 76s 暖缓存）

## 阶段 0 — 基线加固

| 检查 | 结果 | 证据 |
|------|------|------|
| cell_builder 索引 bug | ✅ 已修复 | commit b8bf25e `dirty_rows[ row ]` |
| docs/reference 恢复 | ✅ 已恢复 | commit fb6b6ce 44 files 14886 insertions |
| panic-free dirty cache | ✅ 已修复 | commit 691e14b `is_clean is_some_and` |
| render/tests _gpu | ✅ 已修复 | 同上, 2 处重命名 |
| cargo test | ✅ 1010 passed | `nix develop --command cargo test --lib` 76s |
| markdownlint 新文档 | ✅ 0 issues | `comprehensive-hardening-v7-*.md` 0 errors |
| flake 依赖 | ✅ 最新兼容 | `cargo update` 0 变更, nixpkgs 2026-08-29 |

## 阶段 1 — 鼠标编码

| 用例 | 状态 | 证据 |
|------|------|------|
| gate off (no tracking) | ✅ 通过 | `ghostty_terminal::tests::encode_mouse_event_gated_off_without_tracking_mode` |
| SGR press | ✅ 通过 | `encode_mouse_event_sgr_press` |
| wheel | ✅ 通过 | `encode_mouse_event_wheel` |
| bounds clamp (新增) | ⏳ 待补 | 计划新增 `encode_mouse_event_bounds_negative_clamp` |
| drag sequence (新增) | ⏳ 待补 | 计划新增 `encode_mouse_event_drag_sequence` |
| ffi 空 array 静默丢弃 | ✅ 已审计 | `ffi.rs:1400` `empty()` 非 null |
| Kotlin touch→encode 实时 cell 尺寸 | ✅ 已审计 | `TerminalSurface:2747,2880,2890` 透传 live cellW/H |
| 模拟器 vim 手动 | ⏳ 待重建 native 后 | 需 x86_64 libnative.so |

## 阶段 2 — 无障碍

| 用例 | 状态 | 证据 |
|------|------|------|
| visibleLines 计算 | ✅ 已实现 | `TerminalAccessibility.kt:20-60` |
| LineNavigator 包裹 | ✅ 已实现 | `next`/`previous` wrap |
| Debounced updater 500ms | ✅ 已实现 | `AccessibilityLineProvider` + updater |
| contentDescription 更新 | ✅ 已审计 | `TerminalSurface:accessibilityDescriptionUpdater` |
| Robolectric 新增 | ⏳ 待补 | 计划新增截断/diff 去重用例 |
| 模拟器 TalkBack | ⏳ 待验证 | 需开启 TalkBack 后 `uiautomator dump` |

## 阶段 3 — OSC133

| 用例 | 状态 | 证据 |
|------|------|------|
| B→C capture 64KB 跨 chunk | ✅ 通过 | `output_processor::tests::last_command_output_*` 4 tests |
| ST/BEL 双终结 | ✅ 通过 | `test_shell_integration_st_terminator` |
| A 重置 | ✅ 通过 | `test_last_command_output_reset_on_new_prompt` |
| shell_exit_code D;42 | ✅ 通过 | `test_shell_integration_exit_code` |
| 语义段列范围 (新增) | ⏳ 待实施 | 需 SemanticSegment 扩展 |
| getLastCommandOutput JNI | ✅ 已审计 | `ffi.rs` + `NativeBridge.getLastCommandOutput` + mcp |
| 模拟器 printf 验证 | ⏳ 待重建 native 后 | `printf '\x1b]133;B\x07...` |

## 阶段 4 — CellRun

| 用例 | 状态 | 证据 |
|------|------|------|
| CachedInstances 行级增量 | ✅ 已实现 | `cell_builder.rs` + `compute_dirty_bands` |
| 同格式游程 (新增) | ⏳ 待实施 | 需 `build_row_runs` + 4 单测 |
| Benchmark | ⏳ 待实施 | `cargo bench cell_builder` |

## 阶段 5 — 细节硬化

| 子项 | 状态 | 证据 |
|------|------|------|
| 初始 winsize 竞态 | ⏳ 待实施 | spawn 前预计算 pixel 尺寸 |
| sha256 sidecar | ⏳ 待实施 | BootstrapDownloader best-effort |
| ArgumentTokenizer shell-words | ⏳ 待实施 | 引入 `shell-words` crate |
| SO_PEERCRED | ⏳ 待实施 | `getsockopt(SO_PEERCRED)` |
| 行级脏去重注释 | ✅ 已有 | `rows_equal` bytemuck 字节等价 |

## 阶段 6 — 依赖与构建

| 检查 | 结果 |
|------|------|
| cargo update | ✅ 0 变更, 1010 tests 通过 |
| gradle dependencyUpdates | ⏳ 待检查 |
| flake.lock | ✅ nixpkgs 2026-08-29 最新 |
| CI lens 超时 60→120s | ⏳ 待配置 `.pi/lens.toml` |
| clippy | ✅ 0 新增 (2 pedantic 允许) |
| detekt | ⏳ 待 `./gradlew detekt` |

## 模拟器验证（当前）

### 启动

- `adb install -r app-debug.apk` ✅ Success
- `adb shell am start -n com.termux/terminal.emulator.MainActivity` ✅
- 初次启动 ANR (System UI isn't responding) → 点击 Wait 2 次后恢复 ✅
- `mCurrentFocus=Window{da133f3 com.termux/terminal.emulator.MainActivity}` ✅
- `uiautomator dump` 显示 `TerminalContent [0,128][1080,2147]` + `ModifierBarOverlay [0,2147][1080,2337]` ✅

### 帧率（当前未达标，因 libnative.so 缺失）

- `dumpsys gfxinfo` 5 帧, 50th 150ms, 90th 3300ms, 100% janky
- 原因: `dlopen failed: library "libnative.so" not found` → 回退 Skia OpenGL, 非 wgpu Vulkan
- 预期修复: `cargo ndk --target x86_64-linux-android build` → `jniLibs/x86_64/libnative.so` → `assembleDebug` 后帧率应恢复
- 显示刷新率: 60Hz (emulator-5554 上报 `renderFrameRate 60.000004`), 90fps+ 需 90Hz 设备验证；60Hz 设备上目标为 60fps 稳定 (<16ms 90th)

### 权限

- `POST_NOTIFICATIONS: granted=true` ✅ (已 `pm grant`)
- `READ_EXTERNAL_STORAGE: granted=false` (非必需)

### 日志

- `NativeBridge: Failed to load native library` 预期（修复后消失）
- 其余 logcat 无 native 崩溃

## 待办（重建后）

1. 等待 `build-native-apk` 后台任务完成 → `adb install -r` → 重采 `dumpsys gfxinfo` (reset → 5s 交互 → framestats, 预期 90th <16ms 60Hz 稳定)
2. 补新增 Rust 单测 7+2+4 → `cargo test --lib` 验证 1010+13 通过
3. 补 Robolectric → `./gradlew :app:testDebugUnitTest`
4. 执行 `cargo bench cell_builder` + `./gradlew detekt` + `cargo clippy`
5. 模拟器手动: vim mouse、TalkBack、OSC133 printf
6. 更新本报告为 ✅/❌ 最终态, 连续三次 review 无问题后视为完成

## 结论（当前）

- 后端确定性: ✅ 1010 Rust 单测通过, 覆盖 PTY/渲染/会话/MCP
- 前端可靠: ⏳ 模拟器启动成功但 native 缺失导致帧率未达标, 重建后预期达标
- 像素级复制: ⏳ v6 的 4 项已部分落地, v7 剩余 12 项待实施（保守小步）
- 自动化: ⏳ 单测+JVM 已自动化, 模拟器 gfxinfo 脚本待重建后自动化
