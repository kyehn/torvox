# Design: reference-adoption-v6

## Architecture

### Mouse Encoding Flow

```text
TerminalSurface (Kotlin)
    │ touch/scroll event
    ▼
TerminalViewModel.sendMouseEvent(x, y, action, button)
    │ JNI call
    ▼
ffi.rs::sendMouseEvent → Session::send_mouse_event
    │
    ▼
MouseEncoder::encode(x, y, action, button, cell_w, cell_h)
    │ ghostty_mouse_encoder C FFI
    ▼
PTY write (escape sequence)
```

### Accessibility Overlay Flow

```text
TerminalRuntime render loop
    │ frame rendered
    ▼
onFrameRendered callback
    │ extract visible text
    ▼
TerminalSurface.setContentDescription(visibleText)
    │
    ▼
TalkBack reads contentDescription
```

### OSC 133 Semantic Flow

```text
PTY output
    │
    ▼
GhosttyTerminal::process_output
    │
    ▼
OutputProcessor::handle_osc133(A/B/C/D, col)
    │ update SemanticSegments
    ▼
JNI getLastCommandOutput → Kotlin
```

## Data Structures

### MouseEncoder (Rust)

```rust
pub struct MouseEncoder {
    encoder: *mut ghostty_mouse_encoder,
    mode: MouseMode,
}

pub enum MouseMode {
    None,          // no mouse reporting
    X10,           // mode 1000
    Button,        // mode 1002
    Any,           // mode 1003
    SGR,           // mode 1006
}

pub enum MouseAction { Press, Release, Drag, ScrollUp, ScrollDown }
pub enum MouseButton { Left, Right, Middle, None }
```

### SemanticSegment (Rust)

```rust
pub struct SemanticSegment {
    pub start_col: u16,
    pub end_col: u16,
    pub segment_type: SemanticType,
    pub exit_code: Option<i32>,
}

pub enum SemanticType {
    Prompt,
    CommandInput,
    CommandOutput,
    CommandFinished,
}
```

### CellRun (Rust)

```rust
pub struct CellRun {
    pub start_col: u16,
    pub length: u16,
    pub fg_color: [f32; 4],
    pub bg_color: [f32; 4],
    pub flags: u32,
}
```

## Dependencies

| Dependency | Version | Purpose |
|-----------|---------|---------|
| libghostty-vt | existing | mouse encoder C FFI |
| No new deps | - | all features use existing infrastructure |

## Thread Safety

- MouseEncoder: per-session, called from render thread
- SemanticSegments: guarded by session lock
- CellRun: built during render, no cross-thread sharing

## Error Handling

- Mouse encoding failure → log warning, skip event (non-fatal)
- OSC 133 parse error → log warning, continue (non-fatal)
- Accessibility update failure → swallow (accessibility is best-effort)
