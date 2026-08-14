# 3. Bridge and Rendering Strategy

## Status

Accepted

Date: 2026-07-27

## Requirement IDs

FR-02, NFR-01, NFR-04

## Context

The original torvox used **boltffi** (a multi-language FFI binding generator)
to serialize the entire terminal grid as a wire format and pass it to Kotlin
via JNA. This created a heavy data path:

```
Ghostty C → Rust GridSnapshot → boltffi wire_encode → [u8] →
JNA → Kotlin WireReader deserialize → Kotlin data class → Compose TreeUI
```

boltffi is designed for occasional cross-language calls with small payloads.
Torvox was calling it **60 times per second** with ~43 KB payloads (1,920
cells × 16 bytes + wire overhead). Additionally, the Kotlin side manually
implemented `WireReader`/`WireWriter` for every type — negating boltffi's
"automatic binding" value.

The adversarial review and warp-mobile-android proof project both confirm:
**GPU rendering should stay entirely in the native layer**, with Kotlin only
receiving high-level events.

This decision depends on ADR-0002 (Ghostty is the source of truth for grid
data — we cannot bypass boltffi without that foundation) and informs
ADR-0004 (the thread model must provide a queue that the JNI poller can
access without blocking the session thread).

## Decision

**Render entirely in Rust via wgpu. Kotlin receives only lightweight events
through direct JNI.**

### Data path

```
Ghostty C → CellIterator → flat bytemuck CellData[]
→ queue.write_buffer() → render_pass.draw(0..4, 0..instances) → surface.present()
```

No grid data crosses the FFI boundary. Kotlin interacts only via:

| Direction | Data | Mechanism |
|-----------|------|-----------|
| Rust→Kotlin | title, bell, clipboard, exit code | JNI `pollEvent()` (poll-based, not callback) |
| Kotlin→Rust | session lifecycle | JNI `initSession(executable, args)`, `destroySession()`, `switchSession(id)` |
| Kotlin→Rust | keyboard input | JNI `writeKey(keycode, modifiers)` |
| Kotlin→Rust | PTY bytes | JNI `feedPty(bytes)` |
| Kotlin→Rust | lifecycle events | JNI `attachWindow(ANativeWindow)`, `detachWindow()` |
| Kotlin→Rust | resize | JNI `resize(cols, rows)` |
| Kotlin→Rust | session enumeration | JNI `getSessionCount()`, `listSessions()` |

### boltffi removal

- All `boltffi` and `JNA` dependencies are removed
- The `bridge/` directory (4.6 KLOC) is deleted
- The Rust code uses direct `extern "system"` JNI functions via the `jni`
  crate
- Kotlin side uses `external fun` declarations (~50 lines total)

## Alternatives Considered

### JNI flat int[] array (ghostty-android-terminal pattern)
- **Rejected**: Slightly simpler than boltffi but still sends grid data to
  Kotlin. If Kotlin doesn't need grid data (GPU renders in Rust), there's no
  reason to send it.

### UniFFI
- **Rejected**: ~625 ns/call overhead × 1,920 cells ≈ 1.2 ms/frame just for
  FFI. Intolerable for 60 fps rendering. UniFFI is designed for occasional
  API calls, not real-time data.

## Consequences

### Positive

- Zero serialization overhead for grid data
- JNI overhead limited to ~5 light calls per frame (events, lifecycle)
- Compose TreeUI complexities removed — Android UI is just a `TextureView`
  hosting the wgpu swapchain (Vulkan where available, OpenGL ES fallback)
- APK size decreases (no boltffi runtime, no JNA)

### Negative

- All rendering code must be in Rust — cannot leverage Android's built-in
  HWUI text rendering
- Input handling (IME, composing text) still requires Kotlin→Rust JNI calls
- Ghostty callbacks (title change, clipboard) must be buffered in a Rust
  queue and polled from Kotlin — cannot use synchronous callbacks across JNI

## Compliance

- No `boltffi` or `jna` dependencies in `Cargo.toml`
- No grid cell data in JNI function signatures — only ints, strings, and
  byte arrays for PTY data
- `native/src/android/ffi.rs` contains all `#[no_mangle] extern "system"` exports,
  each under 20 lines

## Status Note (Jul 2026)

This decision has been fully implemented. The boltffi/JNA bridge was replaced with direct JNI (commit ffc7713). All GPU rendering stays in Rust behind JNI. Kotlin receives only JNI events (title/bell/clipboard/exit) via `pollEvent()`. The `jni_bridge.rs` handles ANativeWindow surface extraction. No terminal grid data crosses the FFI boundary.
