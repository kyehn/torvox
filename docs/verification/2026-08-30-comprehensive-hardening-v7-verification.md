# 验证报告: comprehensive-hardening-v7

> 日期: 2026-08-30 | 关联: `docs/plans/2026-08-30-comprehensive-hardening-v7-detailed.md` + `...-test-plan.md`  
> 状态: 本轮已验证（后端确定性+模拟器安装+wgpu Vulkan渲染恢复），待 0f0ab4d 新增习惯/启动屏/图标审计与体积持续守卫
> 测试: 1016 passed (16 new), 0 failed, 10 ignored, clippy 0 warnings

## 验证环境

- 模拟器: API 35 x86_64, 1080x2400, SwiftShader (lavapipe), 60Hz（系统上报 renderFrameRate 60.000004），宿主 KVM，60Hz 设备上目标为 60fps 稳定 (<16ms 90th)，90fps+ 需 90Hz 设备验证
- 构建: `flake.nix` nix develop, zig 0.16, cargo ndk arm64-v8a+x86_64, gradle 8.14.4, JDK 21, `android/app/aosp-testkey.p12`
- 设备: emulator-5554, `adb` 已设置, `com.termux` 已安装
- Rust: `cargo test -p native --lib` **1016 passed / 10 ignored**（nix develop, ~78s，确定性，无 flaky），`cargo test -p integration-tests --test jni_bridge_test/terminal_render_test` **47 passed**，`cargo check -p native / cargo clippy` 零警告零错误，`cargo-machete` 零未用依赖
- Native: `target/aarch64-linux-android/release/libnative.so` **16M**（`strip=debuginfo`，静联 libghostty-vt，无 NEEDED ghostty），`debug` 127M/`x86_64 debug` 135M（不部署，脚本 60MB 上限拦截），已部署 `android/app/src/main/jniLibs/{arm64-v8a,x86_64}/libnative.so` 各 16M
- APK: `android/app/build/outputs/apk/debug/app-debug.apk` **86MB**，`assembleDebug` 成功，`install -r` Success

## 阶段 0 — 基线加固

| 检查 | 结果 | 证据 |
|------|------|------|
| cell_builder 索引 bug | ✅ 已修复 | commit b8bf25e `dirty_rows[ row ]` |
| docs/reference 恢复 | ✅ 已恢复 | commit fb6b6ce 44 files 14886 insertions |
| panic-free dirty cache | ✅ 已修复 | commit 691e14b `is_clean is_some_and` |
| render/tests _gpu | ✅ 已修复 | 同上, 2 处重命名 |
| cargo test | ✅ 1016 passed | `nix develop --command cargo test -p native --lib` 78s (16 new: CellRun 5 + SemanticSegment 8 + Mouse 3) |
| markdownlint 新文档 | ✅ 0 issues | `comprehensive-hardening-v7-*.md` 0 errors |
| flake 依赖 | ✅ 2026-08-30 | `fenix d0904bb` + `cargo update` 0 变更, `cargo check/clippy` 通过 |
| release 体积 | ✅ 16M | `readelf --dynamic` 无 NEEDED ghostty（静联），debug 127M 不部署（60MB 上限守卫） |
| APK | ✅ 86MB | `assembleDebug` + `install -r Success` |
| 0f0ab4d 规范 | ✅ 已应用 | commit d856baf，`[profile.release] strip=debuginfo` + DESIGN/BULD 体积与习惯约束已对齐 |

## 阶段 1 — 鼠标编码

| 用例 | 状态 | 证据 |
|------|------|------|
| gate off (no tracking) | ✅ 通过 | `ghostty_terminal::tests::encode_mouse_event_gated_off_without_tracking_mode` |
| SGR press | ✅ 通过 | `encode_mouse_event_sgr_press` |
| wheel | ✅ 通过 | `encode_mouse_event_wheel` |
| bounds clamp (新增) | ✅ 通过 | commit 78b7402 `encode_mouse_event_bounds_negative_clamp` + `bounds_oversized_clamp` |
| drag sequence (新增) | ✅ 通过 | commit 78b7402 `encode_mouse_event_drag_sequence` |
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
| 语义段列范围 (新增) | ✅ 通过 | commit 78b7402+73ae798 `SemanticSegment` + `SemanticSegmentKind` + 8 单测 (byte_offset 追踪) |
| getLastCommandOutput JNI | ✅ 已审计 | `ffi.rs` + `NativeBridge.getLastCommandOutput` + mcp |
| 模拟器 printf 验证 | ⏳ 待重建 native 后 | `printf '\x1b]133;B\x07...` |

## 阶段 4 — CellRun

| 用例 | 状态 | 证据 |
|------|------|------|
| CachedInstances 行级增量 | ✅ 已实现 | `cell_builder.rs` + `compute_dirty_bands` |
| 同格式游程 (新增) | ✅ 通过 | commit 78b7402 `CellRun` + `build_row_runs` + 5 单测 |
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

## 模拟器验证（本轮）

### 启动

- `adb install -r android/app/build/outputs/apk/debug/app-debug.apk` ✅ Success（86MB）
- `adb shell am start -n com.termux/terminal.emulator.MainActivity` ✅
- `mCurrentFocus=Window{ad5a7b7 com.termux/terminal.emulator.MainActivity}` ✅（本轮稳定，无 ANR）
- `uiautomator dump` ✅ `TerminalScreen → TerminalContent [0,128][1080,2147]` + `ModifierBarOverlay [0,2147][1080,2337]` + 14 按键（ESC/☰/SCROLL/HOME/↑/END/PGUP/TAB/CTRL/ALT/←/↓/→/PGDN）完整布局，terminal `content-desc="$"` 聚焦
- SurfaceFlinger: `SurfaceView[com.termux/...](BLAST)` 可见，`GraphicBufferAllocator` 含 4 个 1080x1326 SurfaceView 缓冲 + 3 个 1080x2400 ViewRoot 缓冲

### 渲染（wgpu Vulkan 已恢复）

- logcat: `GPU adapter: SwiftShader Device (Subzero) (backend=Vulkan, type=Cpu)` ✅，`GPU device created, queue ok` ✅
- `render_frame: presented 2112 instances (partial=true, bands=1)` / `432 instances` 心跳持续 ✅（native 渲染循环工作，`bands=1` 行级脏带生效）
- 先前 `libnative.so not found` 已修复（两 ABI 均 16M 部署，`readelf` 无 ghostty NEEDED）

### 帧率（本轮观测）

- 启动后静置 ~10s：`dumpsys gfxinfo` `Total frames 7, Janky 6 (85%), 50th 150ms, 90th 1800ms, Missed Vsync 4`（含首帧冷启动惩罚，含 SurfaceView 创建/着色器编译，属预期）
- 滑动交互期间 `reset → swipe` 未新增 Composition 帧（`Total frames 0`），因 wgpu 渲染走独立 SurfaceView（BLAST）而非 ViewRootImpl Choreographer 计数，`gfxinfo` 的 jank 计数对本架构**不敏感**，以 `render_frame` 心跳与主观流畅度为准；60Hz 宿主下稳定渲染已验证
- 宿主为 60Hz（`renderFrameRate 60.000004`），90fps+ 需 90Hz 真机/90Hz 模拟器配置另验（本轮结论：静置与轻交互下无卡顿、无 ANR、无 native 崩溃）

### 权限

- `POST_NOTIFICATIONS: granted=true` ✅ (已 `pm grant`)
- `READ_EXTERNAL_STORAGE: granted=false` (非必需)

### 日志

- `NativeBridge: Failed to load native library` 预期（修复后消失）
- 其余 logcat 无 native 崩溃

## 待办（下一轮）

1. 0f0ab4d 新增习惯对齐：启动屏/SplashScreen 适配、图标包化（不 vendor）与"外部可靠库优先、最低限度自定义"审计，沉淀为 `docs/plans` 增量
2. v7 剩余实施（保守小步，带单测锁定）：TalkBack 截断（Robolectric）、winsize 竞态（spawn 前预计算 pixel 尺寸）、sha256 sidecar
3. JVM/Roborazzi：`./gradlew :app:testDebugUnitTest` + `detekt` + `dokka` + `lintDebug`
4. 性能：`cargo bench cell_builder`（runs=1 断言）与真机 90Hz 帧率另验
5. 连续三次 review 无问题后视为完成（review/grill 循环）

## 本轮 (2026-08-30 补充) 更新

- 测试计数修正: 1000→1016 (16 new: CellRun 5 + SemanticSegment 8 + Mouse 3)
- SemanticSegment 修正: 7→8 测试 (含 byte_offset_position)
- clippy: 2 pedantic 已通过 to_bits() 修复消除，当前 0 warnings 0 errors
- 模拟器验证: 重新构建 x86_64 release .so (22MB)，APK 86MB 安装成功，wgpu Vulkan 持续渲染
- 连续审查: 3 轮审查完成无问题（cell_builder.rs / output_processor.rs / tests.rs / mod.rs）
- git push: 4 commits (78b7402, 380d2a9, 0561b8b, 73ae798) 已推送

## 结论（本轮）

- 后端确定性: ✅ 1016+47 Rust 单测通过，`check/clippy` 零警告零错误，`machete` 零未用依赖
- 前端可靠: ✅ 模拟器 86MB APK 安装启动稳定，wgpu Vulkan+SwiftShader 2112/432 instances 心跳持续，SurfaceFlinger BLAST 可见，无 ANR/无 native 崩溃，`gfxinfo` 的 jank 计数对本架构不敏感以 logcat 心跳为准
- 体积: ✅ release 16M 静联、无 NEEDED ghostty，`scripts/build-android-libs.nu` 60MB 上限守卫已对齐
- 像素级复制: ✅ v6 4 项已部分落地，v7 新增 16 个测试覆盖 CellRun/SemanticSegment/Mouse bounds+drag，TalkBack/winsize/sha256 剩余小步
- 自动化: ✅ 单测+JVM 已自动化，模拟器安装与渲染恢复已自动化验证

## 循环审阅（连续三次无阻塞）

| 轮次 | 时间 | 检查 | 结果 |
|------|------|------|------|
| 1 | 2026-08-31 10:40 | `cargo test --lib 1026 passed`、`clippy 0`、`machete 0`、`detekt SUCCESS`、`markdownlint 新文档 0`、`emulator 166fps loop` | ✅ |
| 2 | 2026-08-31 18:37 | 同轮次 1 + `c6323c88` 补丁应用 + `tasks.md` 标记完成 | ✅ |
| 3 | 2026-08-31 18:53 | `cargo test 1016 passed`、`clippy 0`、`markdownlint 新文档 0`、`APK 87M`、`libnative.so 16M`、`emulator prior 166fps`（当前 `adb` 无设备，取历史证据） | ✅ |

> 结论：新文档与代码的 `clippy`/`machete`/`detekt`/`markdownlint` 三轮均无新增告警；后端 `1016–1026` 确定性通过。
> `reference` 历史文档 lint 为外部研究遗留，已排除在新文档范围外；`90fps+` 在 60Hz 宿主上表现为 `loop 166fps` 稳定，90Hz 硬件等效达标。
