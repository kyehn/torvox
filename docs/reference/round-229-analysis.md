# Round 229 Gap Analysis: §3.5 + §3.6 — FINAL

## Summary

Based on comprehensive code search (`grep`, `read_file`, `code_index`)
and implementation through rounds 229–231:

| # | Gap | Status | Commit(s) |
|---|-----|--------|-----------|
| **§3.5** | | | |
| 3.5.1 | PTY 初始 winsize | ✅ IMPLEMENTED | `pty.rs:115-123` — 24×80 seed |
| 3.5.2 | fast-death 恢复 | ✅ IMPLEMENTED | `TerminalRuntime.kt:391-483` |
| 3.5.3 | scrollback 行数设置 | ✅ IMPLEMENTED | Settings→Bridge→NativeBridge→ffi→Session full chain |
| 3.5.4 | 输出导出到文件 | ✅ IMPLEMENTED | `TerminalViewModel.exportTerminalOutput()` + SAF |
| 3.5.5 | 多用户/主用户检查 | ✅ IMPLEMENTED | `BootstrapOrchestrator.kt:48-54` uid/100_000 |
| 3.5.6 | Bootstrap 离线安装 | ✅ IMPLEMENTED | `SettingsScreen` "Install from file" + `ViewModel.installOffline(uri)` |
| 3.5.7 | 符号链接重放 | ✅ IMPLEMENTED | `BootstrapInstaller.kt:143-148,240-248,321-390` |
| **§3.6** | | | |
| 3.6.1 | ArgumentTokenizer | ✅ IMPLEMENTED | `ArgumentTokenizerTest.kt` ~20 assertions |
| 3.6.2 | SO_PEERCRED | ✅ IMPLEMENTED | `mcp.rs:89-163` + 3 tests |
| 3.6.3 | MCP screenshot | ✅ IMPLEMENTED | `event.rs` + `ffi.rs` + `PollEvent.kt` + `Bridge.kt` + `TerminalRuntime.kt` |
| 3.6.4 | run_command 链路 | ✅ IMPLEMENTED | Rust + Kotlin tests both exist, exit code clamped 0-255 |

**All §3.5 and §3.6 gaps: RESOLVED**

**Excluded per user request**: hash强化 (保留现有), 会话持久化, MCP同意门控, 指纹锁, 能力开关env导出, 确认卡片

## MCP Screenshot Implementation Details

### Architecture
Event-based request/response following the established pattern (dialog, clipboard, run_command):

1. **MCP thread**: `screenshot_tool()` calls `set_screenshot_handler` → pushes `Event::Screenshot` → waits on oneshot `Receiver<(u32, u32, Vec<u8>)>`
2. **Render thread**: `pollEvent()` drains the event → `dispatchScreenshotRequest()` → `NativeBridge.captureFrame()` → GPU readback via `render_to_buffer()` → `NativeBridge.screenshotResult()` → resolves oneshot
3. **MCP thread**: receives RGBA pixels → base64-encodes → returns JSON `{width, height, format, data}`

### Files Changed
| File | Change |
|------|--------|
| `native/src/event.rs` | Added `Screenshot { session_id, request_id }` variant |
| `native/src/android/ffi.rs` | `SCREENSHOT_REQUEST_REGISTRY`, `register_screenshot_request`, `answer_screenshot_request`, `set_screenshot_handler` registration, `screenshotResult` JNI, `captureFrame` JNI |
| `android/.../bridge/PollEvent.kt` | Added `Screenshot` variant with `@SerialName("screenshot")` |
| `android/.../bridge/Bridge.kt` | Added `ScreenshotRequest`, `screenshots` in `PollResult`/`merge`/`parseEvent` |
| `android/.../bridge/NativeBridge.kt` | Added `screenshotResult` + `captureFrame` external declarations |
| `android/.../runtime/TerminalRuntime.kt` | `dispatchScreenshotRequest()` + exit branch reply |

### Key Design Decisions
- **Separate registry**: `SCREENSHOT_REQUEST_REGISTRY` for `(u32, u32, Vec<u8>)` return type (vs `REQUEST_REGISTRY` for `String`)
- **Single JNI call**: `captureFrame` returns `byte[]` with `[u32 LE width][u32 LE height][RGBA pixels]` — no two-phase protocol
- **Render thread**: `dispatchScreenshotRequest` runs on the render thread which owns the wgpu context, avoiding cross-thread GPU access
- **Exit handling**: Pending screenshot requests are answered with empty data on session exit to prevent 300s MCP hangs
