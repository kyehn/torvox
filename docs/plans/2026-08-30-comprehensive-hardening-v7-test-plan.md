# 测试计划: comprehensive-hardening-v7

> 日期: 2026-08-30 | 关联: `2026-08-30-comprehensive-hardening-v7-detailed.md` §4  
> 基准: `TESTING.md`（仅公共 API、确定性同步、具体行为断言）、`docs/reference/02-三向对比` 的测试方法论（LiveTest/无 TTY 确定性帧）

## 1. 测试策略

三层：Rust 单元（确定性、1000+）、Kotlin JVM（Robolectric）、模拟器集成（Espresso/UIAutomator/Maestro）。每层独立可重复执行，合计覆盖后端确定性 + UI 可靠性 + 90fps 性能。

## 2. Rust 单元测试（`nix develop --command cargo test --lib`）

### 2.1 output_processor（新增 7, 现有 12）

```
test_bel_detection
test_no_bel
test_pty_write_raises_new_output_flag
test_idle_keeps_new_output_flag_clear
test_empty_chunk_does_not_raise_new_output_flag
test_shell_integration_prompt_start
test_shell_integration_command_start
test_shell_integration_st_terminator
test_osc52_clipboard_read_forwarded
test_no_shell_integration
test_shell_integration_command_executed
test_shell_integration_exit_code
test_shell_integration_empty_osc
test_shell_integration_unknown_marker
test_shell_integration_incomplete_sequence
test_shell_integration_mixed_terminators
test_shell_integration_detects_marker_in_surrounding_text
test_last_command_output_captures_between_b_and_c
test_last_command_output_cross_chunk_and_st_terminator
test_last_command_output_take_clears
test_last_command_output_reset_on_new_prompt
# 新增（v7 语义段）：
test_osc133_semantic_prompt_start_col
test_osc133_semantic_command_input_col
test_osc133_semantic_command_output_col
test_osc133_semantic_finished_exit_code
test_osc133_semantic_multiline
test_osc133_semantic_take_clears
test_osc133_semantic_reset_on_A
```

### 2.2 ghostty_terminal（新增 2, 现有 3）

```
test_encode_mouse_event_gated_off_without_tracking_mode
test_encode_mouse_event_sgr_press
test_encode_mouse_event_wheel
# 新增：
test_encode_mouse_event_bounds_negative_clamp
test_encode_mouse_event_drag_sequence
```

### 2.3 cell_builder（新增 4, 现有 8）

```
test_cell_run_single_format         # 5 同色 → 1 run
test_cell_run_mixed_format          # 变色 → 2 runs
test_cell_run_newline_breaks_run    # 换行不跨行
test_cell_run_empty_row             # 空行 0 runs
test_compute_dirty_bands_single
test_compute_dirty_bands_multiple
test_compute_dirty_bands_all_clean
test_compute_dirty_bands_all_dirty
test_cached_instances_band_slice
test_cached_instances_is_compatible
test_build_instances_cached_reuses_clean_rows
```

### 2.4 渲染/会话

```
test_wgpu_backend_lavapipe_on_emulator   # is_emulator 分支（条件编译文档）
test_session_spawn_resize_pixel_sync     # winsize 像素同步（新增）
test_mcp_argument_tokenizer_split        # shell-words 切分
test_mcp_so_peercred_uid_check           # peer uid == app uid
```

## 3. Kotlin 单元测试（`./gradlew :app:testDebugUnitTest`）

### 3.1 TerminalAccessibilityTest

```
test_visibleLines_firstRow_calculation
test_visibleLines_trims_trailing_whitespace
test_visibleLines_drops_blank_rows
test_visibleLines_scrollOffset_negative_clamp
test_visibleLines_scrollOffset_overflow_clamp
test_contentDescription_joined_with_newline
test_contentDescription_truncated_at_MAX_DESCRIPTION
test_contentDescription_updates_on_output
test_current_wraps_to_first
test_next_wraps_around
test_previous_wraps_around
test_current_remains_visible_after_scroll
test_debouncedUpdater_only_last_within_window_emitted
test_debouncedUpdater_identical_text_not_rescheduled
```

### 3.2 BridgeTest / PollEventTest

```
test_encodeMouseEvent_translates_to_native
test_encodeMouseEvent_destroyed_session_returns_empty
test_pollEvent_drains_bell_clipboard_exit
test_parseEvent_bell
test_parseEvent_clipboard
```

## 4. 集成测试（模拟器 API 35, SwiftShader, `adb`）

| # | 场景 | 步骤 | 预期 | 自动化 |
|---|------|------|------|--------|
| I1 | vim 鼠标 | `echo "set mouse=a" > ~/.vimrc; vim file` → `adb shell input tap 200 400` → `adb shell input swipe 200 400 200 500 100` | 光标移动/选择，logcat 含 `encode_mouse_event` SGR | Espresso + adb |
| I2 | 非 mouse mode 零事件 | normal shell `adb shell input tap` | PTY 无 SGR | logcat 断言 |
| I3 | TalkBack 朗读 | 开启 TalkBack → `echo hello` → `adb shell uiautomator dump /sdcard/dump.xml; adb pull` | dump 含 contentDescription hello | UIAutomator |
| I4 | TalkBack Next/Previous | 同 I3 → 右滑/左滑（Next/Previous action） | 逐行朗读包裹 | UIAutomator |
| I5 | OSC133 getLast | `printf '\x1b]133;B\x07echo hi\x1b]133;C\x07hi\n\x1b]133;D;0\x07'` → `Bridge.getLastCommandOutput()` | 返回 echo hi | Instrumented |
| I6 | 多行 OSC133 | `echo -e "a\nb"` 带标记 → getLast | 含换行 | 同上 |
| I7 | CellRun | 同格式长行渲染一帧 → 计数 | runs=1 | 单测已覆盖，集成仅帧率 |
| I8 | 90fps | `dumpsys gfxinfo reset` → `yes head -n 1000` 5s → `dumpsys gfxinfo framestats` | 90th <11ms | gfxinfo 解析脚本 |
| I9 | winsize 同步 | 横竖屏旋转 → `stty size` | rows/cols 变 | adb shell |
| I10 | Bootstrap sha | 篡改 zip → 安装 | 失败重试 | Instrumented |
| I11 | MCP peercred | 非 app uid 连 socket | 拒 | mock 或 su |

## 5. 性能测试

| 指标 | 基线 | 目标 | 采集 |
|------|------|------|------|
| Rust 单测暖缓存 | 76s | <120s CI | `time cargo test --lib` |
| Kotlin 单测 | ~20s | <30s | gradle |
| 模拟器 90th 帧 | 未测 | <11ms | gfxinfo |
| mouse encode 延迟 | N/A | <0.1ms | `criterion` bench |
| contentDescription 更新 | N/A | <1ms | Trace |
| CellRun 合并率 | 0 | >50% | 单测 |

## 6. 测试环境

- Rust: `nix develop`（fenix stable, zig 0.16, mesa lavapipe, VK_ICD_FILENAMES）
- Kotlin: JDK 21, AGP 8.x, Gradle 8.x, parallel/caching/configuration-cache
- 模拟器: API 35 x86_64, SwiftShader, 2GB, emulator-5554
- 真机（可选）: Pixel 7, Vulkan
- CI: GitHub Actions ubuntu-latest, `android-emulator-runner@v2`

## 7. 测试数据

- VT 序列: `native/src/terminal/test_data/`（现有）+ 新增 `osc133_test_data/`（A/B/C/D 带 ST/BEL 双终结）
- Mouse: `mouse_event_test_data/`（SGR 序列样本）
- 字体: `flake.nix` maple-mono + noto-fonts-cjk-sans

## 8. 执行命令

```sh
# Rust（确定性后端）
nix develop --command cargo test --lib -- --test-threads=8
nix develop --command cargo clippy -- -D warnings
nix develop --command cargo bench --bench cell_builder  # 可选

# Kotlin JVM
./gradlew :app:testDebugUnitTest
./gradlew :app:detekt

# 模拟器（需先 assemble + install）
./gradlew :app:assembleDebug
adb install -r android/app/build/outputs/apk/debug/app-debug.apk
adb shell am start -n com.termux/.MainActivity
./gradlew :app:connectedDebugAndroidTest   # 全部 instrumented
adb shell dumpsys gfxinfo com.termux framestats  # 90fps
```

## 9. 验收与回归

- 每项新增测试必须先失败后通过（TDD 红-绿）
- 现有 50+ instrumented 测试必须全绿（PTY spawn/resize、选择/复制/搜索、键栏/主题/bootstrap）
- 覆盖率：新增代码行覆盖 90%+（`cargo llvm-cov` 可选）
- 缺陷回归：每个修复对应单测，记录于 `docs/verification/`
