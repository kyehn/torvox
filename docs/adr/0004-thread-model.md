# 0004 — Thread Model

- **Status**: Accepted
- **Date**: 2026-07-27
- **Requirement IDs**: NFR-01, NFR-03, FR-02

## Context

`libghostty-rs` explicitly declares all its types (`Terminal`, `RenderState`,
`RowIterator`, `CellIterator`) as **`!Send + !Sync`** — the underlying C API
uses thread-local state and is not guarded by mutexes.

This is a hard constraint: each `Terminal` + `RenderState` pair must be
created, accessed, and destroyed on a single thread. It cannot be sent to
another thread or shared by reference. This constraint directly drives the
per-session-thread design (see also ADR-0002 §Decision — because the
terminal state source of truth is Ghostty's C API, the `!Send` constraint
is unavoidable).

Meanwhile, `wgpu::Device`, `wgpu::Queue`, and `wgpu::Surface` are all
**`Send + Sync`** on Android — they can be created on one thread and used on
another freely.

The render path needs to produce flat `CellData` arrays from Ghostty state
and upload them to GPU memory.

## Decision

**One thread per terminal session** produces flat cell arrays. A **shared
render thread** consumes them and drives wgpu.

```
┌─────────────────────┐  flat bytemuck     ┌──────────────────────┐
│ SessionThread #1    │  CellData[]        │  Render Thread       │
│ ┌─────────────────┐ │ ───channel──▶      │ ┌──────────────────┐ │
│ │Terminal (!Send)  │ │                    │ │wgpu Device (Send)│ │
│ │RenderState       │ │                    │ │wgpu Queue        │ │
│ │RowIterator       │ │                    │ │wgpu Surface[N]  │ │
│ │CellIterator      │ │                    │ └──────────────────┘ │
│ │→ flatten CellData│ │                    │  process per frame   │
│ └────────┬─────────┘ │                    │  queue_write_buffer  │
│          │ raw bytes │                    │  draw_indexed        │
└──────────┼───────────┘                    └──────────────────────┘
           │
┌──────────▼───────────┐
│ PTY Reader Thread    │
│ (per session)        │
└──────────────────────┘
```

### Thread roles

| Thread | Count | Owner of | Communicates to |
|--------|-------|----------|-----------------|
| PTY Reader | 1/session | raw PTY fd | SessionThread via `flume` channel (raw bytes) |
| Session | 1/session | `Terminal`, `RenderState`, `RowIterator`, `CellIterator` | RenderThread via `flume` channel (`flat CellData[]`); JNI/main via global event queue |
| Render | 1 global | `wgpu::Device`, `wgpu::Queue`, `wgpu::Surface`, render scheduling | GPU |
| JNI/Main | 1 global | `JNIEnv`, lifecycle, `pollEvent()` | SessionThread via command channel; global event queue via Mutex |
| MCP | 1 per request | — | SessionThread via `flume` snapshot channel (read-only) |

### Event queue (Ghostty callbacks → Kotlin)

Ghostty fires synchronous callbacks for title changes, bell, clipboard
updates, and process exit. These callbacks cannot make JNI calls directly
(the callback runs on an arbitrary C stack frame with no attached `JNIEnv`).

Instead, each callback pushes an `Event` into a **global `Mutex<Vec<Event>>`**:

```rust
static EVENTS: Mutex<Vec<Event>> = Mutex::new(vec![]);

// Called from Ghostty callback (any thread)
// The Ghostty callback receives a context pointer; torvox stores the
// session handle there so it can be recovered.
fn on_title_change(ctx: *mut c_void, title: &str) {
    let session_id: u64 = unsafe { *(ctx as *const u64) };
    EVENTS.lock().unwrap().push(Event::Title {
        session_id,
        title: title.to_string(),
    });
}
```

Kotlin calls `pollEvent()` on the JNI/main thread, which drains the queue:

```rust
#[no_mangle]
pub extern "system" fn Java_io_term_NativeLib_pollEvent(
    mut env: JNIEnv, _class: JClass
) -> jobject {
    let event = EVENTS.lock().unwrap().pop();
    // convert to Java object or return null
}
```

**Why global Mutex, not per-session channels**: Events are rare (TITLE/BELL
at <1 Hz, CLIPBOARD at <1/min). The global Mutex adds ~20 ns lock latency
on non-contended access — negligible for events that occur once per second.
Per-session channels would add complexity (each session must create + tear
down its own event receiver, and the JNI thread must poll N channels) with
no measurable benefit.

### Surface handoff (JNI thread → Render thread)

When `attachWindow(session_id, ptr)` is called on the JNI/main thread:

1. JNI thread: `ANativeWindow_fromSurface(env, surface)` → `NonNull<c_void>`
2. JNI thread: sends the pointer through a **command channel** to the
   Render thread (a dedicated `flume::Sender<SurfaceCommand>`)
3. JNI thread: returns immediately — does NOT create a wgpu Surface
4. Render thread: receives `SurfaceCommand::NewWindow(ptr)` on its next
   frame loop
5. Render thread: creates `AndroidNdkWindowHandle` → `create_surface_unsafe`
   → configures swapchain → resumes rendering

This design avoids thread-safety questions: wgpu Surface creation and
swapchain configuration happen on the same thread that uses them. The JNI
thread only handles the ANativeWindow refcount.

### Multi-session render scheduling

Only the **active (foreground) session** is rendered each frame. Inactive
sessions keep their Ghostty state up to date (they still process PTY output)
but do not produce CellData arrays or wgpu draw calls.

When the user switches tabs:
1. Kotlin calls `switchSession(session_id)` → JNI command channel
2. Render thread: flushes the current frame → marks old surface as inactive
3. SessionThread for the new session is already producing CellData (it was
   always running). One extra frame to switch surface targets.

### Data flow per frame

1. PTY Reader reads `buf` → sends to SessionThread
2. SessionThread: `ghostty_terminal_vt_write(buf)` → `RenderState::update` →
   `RowIterator` loop → `CellIterator` loop → collect into `Vec<CellData>`
   where `CellData` is a `#[repr(C)]` bytemuck `Pod` struct
3. Send `Vec<CellData>` over bounded channel to RenderThread
4. RenderThread: `staging_belt.write_buffer(vertex_buf, &cell_data)` →
   `encoder.draw_indexed()` → `queue.submit()` → `surface.present()`

## Alternatives Considered

### Lock-based sharing (Arc<Mutex<Terminal>>)
- **Rejected**: libghostty-rs `!Send` makes this impossible — cannot wrap
  a `!Send` type in `Arc<Mutex>`. Even if we could, the thread that creates
  the Terminal must be the only one accessing it.

### Single-thread everything
- **Rejected**: PTY reads are blocking. A single thread can't read PTY,
  render at 60 fps, and handle JNI calls without blocking one of them.

### Move PTY reader into SessionThread (avoid channel)
- **Rejected**: Blocking `read()` in the same thread as Ghostty processing
  starves rendering when there's no PTY output. Separation lets the
  SessionThread process at frame rate while PTY reader blocks independently.

## Consequences

### Positive

- Resolves the `!Send` constraint cleanly — each `Terminal` stays on its
  creation thread
- Channel-based communication is bounded, lock-free, and Android-friendly
- Flat `CellData` arrays are trivially `Send` (bytemuck Pod)
- Render thread can be shared across N sessions with no Ghostty dependency

### Negative

- Two channels per session: PTY bytes + CellData (minimal overhead, each
  is a single `send` per frame). The channel crate used is `flume` (already
  in the workspace).
- SessionThread is idle when there's no PTY output (bounded channel blocks
  on send when render thread is busy)
- Render thread must handle surface recreation for each session
  independently (ADR-0007)
