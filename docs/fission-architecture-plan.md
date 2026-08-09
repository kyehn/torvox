# Fission-Inspired Architecture Improvements for Torvox

## Research Summary

Based on deep analysis of Fission's architecture (application framework with terminal shell),
this document maps 4 key patterns to torvox's wgpu-based Android terminal emulator.

### Fission Patterns Analyzed

| Pattern | Fission Source | Torvox Applicability |
|---------|---------------|---------------------|
| LiveTest Protocol | fission-test-driver/src/lib.rs | TCP JSON test control for emulator |
| Deterministic Frame | fission-shell-terminal/src/frame.rs | CPU-side CellFrame for CI screenshots |
| Invalidation Cache | fission-shell-winit/src/pipeline.rs | Cursor/Cell/Geometry/Full invalidation |
| Platform Traits | Host trait + Memory/Unsupported | ClipboardHost, EventBridge trait-based |

---

## Pattern 1: Platform Capability Traits

### Fission Pattern
```
trait ClipboardHost { fn get_text(); fn set_text(); }
├─ JniClipboardHost (Android JNI)
├─ MemoryClipboardHost (tests)
└─ UnsupportedClipboard (stub)
```

### Torvox Adaptation

**File: `native/src/platform/mod.rs`** (new)

```rust
/// Platform abstraction for clipboard operations.
/// Replaces direct JNI calls with testable trait-based dispatch.
pub trait ClipboardHost: Send + Sync {
    fn get_text(&self) -> Option<String>;
    fn set_text(&self, text: &str);
}

pub struct JniClipboardHost {
    session_id: u64,
    // JNI env captured at construction
}

pub struct MemoryClipboardHost {
    text: std::sync::Mutex<Option<String>>,
}

impl ClipboardHost for MemoryClipboardHost {
    fn get_text(&self) -> Option<String> {
        self.text.lock().unwrap().clone()
    }
    fn set_text(&self, text: &str) {
        *self.text.lock().unwrap() = Some(text.to_string());
    }
}
```

**File: `native/src/platform/clipboard.rs`** (new)

```rust
pub trait ClipboardProvider: Send + Sync {
    fn read(&self, selection: ClipboardSelection) -> Option<String>;
    fn write(&self, text: &str);
}

pub enum ClipboardSelection { Clipboard, Primary }

// Production impl delegates to EVENT_QUEUE + session clipboard arcs
pub struct AndroidClipboardProvider { session_id: u64 }

// Test impl stores in-memory
pub struct MockClipboardProvider {
    clipboard: Mutex<Option<String>>,
    primary: Mutex<Option<String>>,
}
```

### Benefits
- Tests can use `MockClipboardProvider` without JNI
- FFI layer becomes thinner (just trait registration)
- Supports future clipboard history, shared clipboard, etc.

---

## Pattern 2: Invalidation Classification

### Fission Pattern
```rust
struct InvalidationSet {
    build: bool,    // Widget tree changed
    layout: bool,   // Geometry changed
    paint: bool,    // Visual appearance changed
    composite: bool // Opacity/transform changed
}
```

### Torvox Adaptation (simplified for terminal grid)

```rust
/// Terminal-specific invalidation levels (ordered by cost).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum InvalidationLevel {
    /// Only cursor blinked/moved — re-render cursor area only
    CursorOnly = 0,
    /// Cell content changed — rebuild cell instances for dirty rows
    Cells = 1,
    /// Grid geometry changed (resize, scroll) — full rebuild
    Geometry = 2,
    /// Font/theme/atlas changed — rebuild everything including atlas
    Full = 3,
}

/// Tracks what changed between frames to skip redundant work.
pub struct FrameInvalidation {
    level: InvalidationLevel,
    dirty_rows: BitVec,  // which rows need rebuild
    cursor_moved: bool,
    selection_changed: bool,
}
```

### Integration with existing `render_cell_data()`

Currently `render_cell_data()` rebuilds ALL instances every frame. With invalidation:
- `CursorOnly`: Only rebuild cursor area (~2-4 instances)
- `Cells`: Rebuild only dirty rows
- `Geometry`/`Full`: Full rebuild

---

## Pattern 3: Deterministic Frame Capture

### Fission Pattern
```
TerminalFrame = pure data grid (width × height × TerminalCell)
  → write_frame_png() via ab_glyph (CPU rasterization)
  → No GPU surface readback needed
```

### Torvox Adaptation

```rust
/// CPU-side terminal frame for CI testing (no GPU needed).
pub struct CpuFrame {
    pub width: u32,
    pub height: u32,
    pub cells: Vec<CpuCell>,
    pub cursor: Option<CpuCursor>,
}

pub struct CpuCell {
    pub codepoint: char,
    pub fg: [u8; 4],   // RGBA
    pub bg: [u8; 4],
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
}

/// Build from CellData (shares computation with GPU path, skips vertex encoding)
impl CpuFrame {
    pub fn from_cell_data(cells: &[CellData], rows: u32, cols: u32, cursor: &CursorInfo) -> Self {
        // Convert CellData f32 colors → u8 RGBA, map flags
    }
    
    /// Render to PNG using ab_glyph (CPU rasterization)
    pub fn to_png(&self, font: &ab_glyph::FontRef, cell_w: u32, cell_h: u32) -> Vec<u8> {
        // Similar to Fission's write_frame_png
    }
    
    /// Search for text content in the frame
    pub fn find_text(&self, needle: &str) -> Vec<TextHit> {
        // Row-by-row text search for assertions
    }
}
```

---

## Pattern 4: LiveTest Protocol

### Fission Pattern
```
TestCommand (JSON) → HTTP POST /cmd → TestResponse (JSON)
├─ Tap { x, y }
├─ TypeText { text }
├─ Screenshot { path }
├─ WaitForText { text, timeout_ms }
├─ GetText {} → Vec<TextItem>
└─ Quit {}
```

### Torvox Adaptation (TCP + JSON over Unix socket)

```rust
/// LiveTest protocol commands (serde JSON).
#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "cmd")]
pub enum TestCommand {
    // Terminal input
    WritePty { data: String },     // Feed raw bytes to PTY
    WriteKey { key: String, modifiers: u8 },
    
    // Screen query
    GetText {},                     // Extract visible text from frame
    GetCursor {},                   // Get cursor position
    
    // Wait conditions
    WaitForText { text: String, timeout_ms: u64 },
    WaitForCursor { row: u32, col: u32, timeout_ms: u64 },
    
    // Screenshot
    Screenshot { path: String },    // CpuFrame → PNG
    CompareScreenshot { golden: String, threshold: f64 },
    
    // Lifecycle
    Resize { rows: u32, cols: u32 },
    Wait { ms: u64 },
    Quit {},
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "status")]
pub enum TestResponse {
    Ok {},
    Text { items: Vec<TextItem> },
    Cursor { row: u32, col: u32, visible: bool },
    Screenshot { png_base64: String, width: u32, height: u32 },
    Comparison { passed: bool, changed_percent: f64 },
    Error { message: String },
}
```

### Server Implementation

```rust
pub struct LiveTestServer {
    listener: TcpListener,
    session_id: u64,
    // Shared reference to session + CpuFrame builder
}

impl LiveTestServer {
    pub fn start(port: u16, session_id: u64) -> Result<Self> { ... }
    
    fn handle_command(&self, cmd: TestCommand) -> TestResponse {
        match cmd {
            TestCommand::GetText { } => {
                let frame = self.build_cpu_frame();
                let items = frame.extract_text_items();
                TestResponse::Text { items }
            }
            TestCommand::WaitForText { text, timeout_ms } => {
                // Poll with exponential backoff
                let deadline = Instant::now() + Duration::from_millis(timeout_ms);
                loop {
                    let frame = self.build_cpu_frame();
                    if frame.find_text(&text).len() > 0 {
                        return TestResponse::Ok {};
                    }
                    if Instant::now() >= deadline {
                        return TestResponse::Error { message: format!("timeout waiting for '{}'", text) };
                    }
                    thread::sleep(Duration::from_millis(50));
                }
            }
            // ...
        }
    }
}
```

---

## File Structure

```
native/src/platform/
├── mod.rs              // Platform trait definitions
├── clipboard.rs        // ClipboardHost, MockClipboardProvider
└── capabilities.rs     // Runtime capability detection

native/src/render/
├── invalidation.rs     // InvalidationLevel, FrameInvalidation
├── cpu_frame.rs        // CpuFrame, CpuCell (CPU-side rendering)
└── (existing files unchanged)

native/src/test/
├── mod.rs              // Test infrastructure module
├── live_test.rs        // LiveTestServer, TestCommand, TestResponse
└── golden.rs           // Golden image comparison utilities
```

---

## Testing Strategy

| Layer | Tool | What It Tests | Speed |
|-------|------|--------------|-------|
| L1: Rust unit | cargo test | Platform traits, InvalidationLevel, CpuFrame, LiveTest | ⚡ Fast |
| L2: Roborazzi | recordRoborazziDebug | Compose UI rendering correctness | ⚡ Fast |
| L3: Compose UI | ComposeTestRule | ModifierBar, TerminalScreen interactions | Medium |
| L4: Maestro | maestro CLI | End-to-end user flows on emulator | Slow |
| L5: UIAutomator | uiautomator2 | System-level UI interactions | Slow |
| L6: Espresso | EspressoTestRule | View-level assertions on emulator | Slow |

---

## Implementation Order

1. **Phase 1**: Platform traits (clipboard.rs, capabilities.rs) — no dependencies
2. **Phase 2**: Invalidation (invalidation.rs) — enhances existing render loop
3. **Phase 3**: CpuFrame (cpu_frame.rs) — depends on CellData, CellBuilder
4. **Phase 4**: LiveTest (live_test.rs) — depends on CpuFrame + Platform traits
5. **Phase 5-8**: Tests — depends on all above

## Risks & Mitigations

| Risk | Mitigation |
|------|-----------|
| CpuFrame font rasterization too slow for CI | Use ab_glyph (same as Fission), cache glyph cache |
| LiveTest TCP latency on emulator | Use Unix domain socket when available, TCP fallback |
| Invalidation adds complexity to hot path | Gate behind feature flag, default to full rebuild initially |
| MemoryClipboardProvider leaks in tests | Use Drop impl, or Arc-based lifecycle |
