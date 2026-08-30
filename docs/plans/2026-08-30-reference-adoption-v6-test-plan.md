# 测试计划: reference-adoption-v6

## 测试策略

### 1. 单元测试 (Rust)

#### mouse_encoder.rs
```
test_mouse_mode_none_returns_none
test_mouse_mode_x10_press
test_mouse_mode_button_press
test_mouse_mode_any_drag
test_mouse_mode_sgr_press
test_mouse_mode_sgr_release
test_mouse_mode_sgr_scroll_up
test_mouse_mode_sgr_scroll_down
test_mouse_bounds_negative_clamp
test_mouse_bounds_overflow_clamp
test_mouse_encode_returns_valid_utf8
```

#### output_processor.rs (OSC 133)
```
test_osc133_prompt_start
test_osc133_command_input
test_osc133_command_output
test_osc133_command_finished_with_exit_code
test_osc133_multiline_command
test_osc133_get_last_output
test_osc133_multiple_commands
```

#### cell_builder.rs (CellRun)
```
test_cell_run_single_format
test_cell_run_mixed_format
test_cell_run_newline_breaks_run
test_cell_run_empty_row
```

### 2. 单元测试 (Kotlin)

#### TerminalAccessibilityTest
```
testContentDescription_updates_on_output
testContentDescription_includes_visible_text
testAnnounce_on_bell
testAnnounce_on_title_change
```

### 3. 集成测试

#### MouseEncodingIntegrationTest
```
test_vim_responds_to_mouse_click
test_non_mouse_mode_ignores_events
```

#### AccessibilityIntegrationTest
testTalkBack_reads_terminal_content

### 4. 模拟器测试

| 场景 | 步骤 | 预期 |
|------|------|------|
| vim 鼠标 | 启动 vim → 点击 | 光标移动 |
| htm 鼠标 | 启动 htm → 点击 | 选中项目 |
| TalkBack | 开启 TalkBack → 输入 | 朗读内容 |
| OSC 133 | 运行 `echo test` → getLastCommandOutput | "test" |

### 5. 性能测试

| 指标 | 基线 | 目标 |
|------|------|------|
| CellRun 合并率 | 0% | >50% (同格式文本) |
| JNI 调用次数 | N | <N/2 (同格式文本) |
| contentDescription 更新耗时 | N/A | <1ms |
| mouse encoding 延迟 | N/A | <0.1ms |

## 测试环境

- 模拟器: API 35, x86_64, SwiftShader
- 真机: Pixel 7 (可选)
- CI: GitHub Actions (ubuntu-latest)

## 测试数据

- VT 序列样本: `native/src/terminal/test_data/`
- OSC 133 序列: 新增 `osc133_test_data/`
- Mouse 事件: 新增 `mouse_event_test_data/`
