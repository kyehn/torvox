# 7. Session Lifecycle and Android Integration

## Status

Accepted

Date: 2026-07-27

## Requirement IDs

FR-03, FR-04, NFR-03

## Context

Android's Activity lifecycle requires the app to handle:
- **Activity recreation** (screen rotation, config change): Surface destroyed
  and recreated. The old `ANativeWindow` pointer becomes invalid.
- **Process death**: The OS can kill the app process. All Rust state and PTY
  processes die with it.
- **Background → Foreground**: The app must resume rendering without
  destroying the terminal state.

The original torvox handled this with a complex boltffi bridge that cached
the ANativeWindow pointer across thread boundaries and restored wgpu surfaces
on recreation.

Key facts established by research:

- `ANativeWindow*` always fits in an `i64` (pointer size ≤ 8 bytes on all
  Android ABIs)
- `ANativeWindow_fromSurface` / `ANativeWindow_release` use refcounting —
  each `fromSurface` call must be paired with a `release`
- `ANativeWindow` (via `ndk` crate's `NativeWindow`) is `Send + Sync`
- wgpu `Surface` is `Send + Sync` — can be created on UI thread and moved
  to render thread
- Surface recreation: old ANativeWindow → release → get new → create new
  wgpu surface

This ADR depends on ADR-0003 (JNI lifecycle events) and ADR-0004 (the
render thread that must receive the recreated surface).

## Decision

### Session ownership

Session lives in Rust (`native/`), not Kotlin. PTY fork/exec, the Ghostty
Terminal, and the render loop are all Rust-managed.

### Lifecycle protocol (Kotlin ↔ Rust)

```
Activity.onResume():
  Kotlin: attachWindow(ANativeWindow*) → JNI
  Rust: create wgpu::Surface from ANativeWindow
       → configure swapchain → resume render loop

Activity.onPause():
  Kotlin: detachWindow() → JNI
  Rust: stop render loop → release swapchain
       → ANativeWindow_release()
       → set Surface to None (keep Device + Terminal state)

Activity.onDestroy():
  Kotlin: destroySession(sessionId) → JNI
  Rust: kill PTY child process → drop Terminal → free resources

Process death + recovery:
  Rust: Ghostty Formatter API → GHOSTTY_FORMATTER_FORMAT_VT
       → save VT sequences to app-internal storage
  Restore: spawn fresh shell → pty_write(saved_sequences)
```

### Multiple sessions

Each session is identified by a `u64` handle returned from
`createSession()`:

```
createSession() → u64  (Rust allocates SessionThread + PTY)
destroySession(u64)     (Rust cleanup)
switchSession(u64)      (RenderThread switches active surface)
```

### Session persistence (process death)

- On significant output or periodically: serialize visible grid + scrollback
  via Ghostty Formatter (`GHOSTTY_FORMATTER_FORMAT_VT`) and save to file
- On app restart: check for saved file → read → spawn shell → replay
  VT sequences → shell prompt appears at bottom
- Clean saved file after successful restoration

## Alternatives Considered

### Session in Kotlin side
- **Rejected**: Since GPU rendering is in Rust, the rendering thread needs
  the session to produce cell data. Splitting session management across
  the JNI boundary adds complexity without benefit.

### Keep session alive across Activity destroy (foreground service)
- **Deferred**: This is a future concern when torvox needs multi-tasking.
  For v1, sessions survive rotation/configuration changes (Activity
  recreation) but not process death. A foreground service can be added
  later.

## Consequences

### Positive

- Session lifecycle mirrors render lifecycle — both in Rust
- Single source of truth for all session state
- Process death recovery via Ghostty's own formatter (not rkyv)

### Negative

- Kotlin must always pass the ANativeWindow pointer through JNI on every
  `onResume` (but this is unavoidable — Android doesn't provide it
  automatically)
- Process death recovery restores only visual state, not the shell process
  itself (same limitation as every terminal emulator)

## Compliance

- `attachWindow` / `detachWindow` must be called in matching pairs
- Session handles are opaque `u64` values — never raw pointers across JNI
- The `ANativeWindow_release` call is verified with `ATrace` markers

## Status Note (Jul 2026, updated Aug 2026)

This decision is partially implemented. Session lifecycle is managed in Rust via `SessionRegistry` (ffi.rs). ANativeWindow surface handoff uses a command channel (`SurfaceCommand` queue). JNI is used instead of boltffi — the boltffi/rkyv mentions in the original text are superseded by ADR-0003 and ADR-0006 implementations. The Kotlin-side `LaunchedEffect` polling loop and session persistence via Ghostty Formatter remain pending as of Aug 2026.
