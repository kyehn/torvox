# Round 229 Gap Analysis: §3.5 + §3.6

## Summary

Based on comprehensive code search (`grep`, `read_file`, `code_index`):

| # | Gap | Status | Action |
|---|-----|--------|--------|
| **§3.5** | | | |
| 3.5.1 | PTY 初始 winsize | ✅ IMPLEMENTED | `pty.rs:115-123` — spawn with winsize before first read |
| 3.5.2 | fast-death 恢复 | ✅ IMPLEMENTED | `TerminalRuntime.kt:391-483` — exponential backoff, user-cancel |
| 3.5.3 | scrollback 行数设置 | ⚠️ PARTIAL | Settings UI exists, native side NOT wired (DEFAULT_SCROLLBACK_LINES=50000 hardcoded) |
| 3.5.4 | 输出导出到文件 | ❌ NOT IMPLEMENTED | No export functionality |
| 3.5.5 | 多用户/主用户检查 | ❌ NOT IMPLEMENTED | No userId check in installer |
| 3.5.6 | Bootstrap 离线安装 | ⚠️ PARTIAL | Path-based install works, no SAF file picker UI |
| 3.5.7 | 符号链接重放 | ✅ IMPLEMENTED | `BootstrapInstaller.kt:143-148,240-248,321-390` |
| **§3.6** | | | |
| 3.6.1 | ArgumentTokenizer | ✅ IMPLEMENTED | `ArgumentTokenizerTest.kt` ~20 assertions |
| 3.6.2 | SO_PEERCRED | ✅ IMPLEMENTED | `mcp.rs:89-163` + 3 tests |
| 3.6.3 | MCP screenshot | ⚠️ PARTIAL | Rust tool exists, FFI+Kotlin NOT wired |
| 3.6.4 | run_command 链路 | ✅ IMPLEMENTED | Rust + Kotlin tests both exist |

**Excluded per user request**: hash强化 (保留现有), 会话持久化, MCP同意门控, 指纹锁, 能力开关env导出, 确认卡片

## Implementation Plan (5 items)

### T1: scrollback 行数接线 (P0)
- `Bridge.kt` → pass `scrollbackLines` from SettingsRepository
- `NativeBridge.kt` → JNI `initSession` accepts scrollback parameter
- `ffi.rs` → pass to `Session::spawn` via `RunConfig`
- `session.rs` → use `scrollback_lines` from `ThemeConfig` or `RunConfig`
- Tests: unit test for parameter propagation

### T2: 输出导出 (P1)
- `TerminalViewModel.kt` → `exportToFile()` using `GridSnapshot`
- `ffi.rs` → `dump_grid_text` already exists, wire to Kotlin
- `TerminalScreen.kt` → export action in overflow menu
- SAF: `ACTION_CREATE_DOCUMENT` → write text to chosen file
- Tests: unit test for export content

### T3: 多用户检查 (P2)
- `BootstrapOrchestrator.kt` → pre-install userId == 0 check
- `BootstrapInstaller.kt` → fail with clear error if not primary user
- Tests: unit test for check logic

### T4: 离线安装 SAF (P1)
- `SettingsScreen.kt` → "Install from file" button
- `MainActivity.kt` → `ACTION_OPEN_DOCUMENT` for `.zip`
- Route to existing `installBootstrapFromPath`
- Tests: unit test for intent routing

### T5: MCP screenshot 接线 (P1)
- `ffi.rs` → register screenshot handler calling into Kotlin
- `TerminalViewModel.kt` → implement screenshot capture via Compose Canvas
- `mcp.rs` → verify handler callback chain
- Tests: unit test for screenshot tool
