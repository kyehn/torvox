# 9. Security Model

## Status

Accepted

Date: 2026-09-05

## Requirement IDs

NFR-009, NFR-014, NFR-019, FR-013

## Context

Torvox is an Android terminal emulator that operates with elevated
privileges relative to a typical app. It spawns PTY child processes (shell
sessions), manages a GPU rendering pipeline, and exposes an MCP socket for
AI agent integration. Each of these capabilities introduces attack surface
that must be bounded.

The threat model is shaped by the Android application sandbox:

- **Process isolation**: Each app runs in its own Linux process with a
  unique UID. The kernel enforces filesystem, memory, and IPC boundaries.
- **NDK/native code**: Torvox's Rust native library (`libtorvox`) runs in
  the app's process with the app's permissions. It has no special system
  privileges beyond what Android grants the app.
- **PTY child processes**: Shell sessions inherit the app's UID and
  sandbox. They cannot escalate beyond app-scoped resources.
- **MCP socket**: An Android-unix-abstract socket provides local IPC for
  AI agents. Only processes sharing the app's UID (or root) can connect.

Prior to this decision, no explicit security documentation existed. The
architecture implicitly relied on Android's sandbox, but several
defense-in-depth mechanisms were added without a unifying document.

## Decision

### 1. Trust boundary: Android app sandbox

The primary security boundary is Android's per-app UID sandbox. All
components — Rust native library, PTY children, MCP server — operate
within this boundary. No code path in Torvox requires or requests
permissions beyond standard app capabilities.

### 2. MCP socket access control (SO_PEERCRED)

The MCP Unix socket (`android:unix:/abstract/torvox_mcp`) uses a
peer-credential check on every accepted connection:

```rust
fn peer_uid_allowed(peer_uid: u32, own_uid: u32) -> bool {
    peer_uid == own_uid || peer_uid == 0
}
```

Connections from any UID other than the app's own UID or root are
immediately closed. This mirrors the `termux` `LocalServerSocket` policy.
The check is defense-in-depth: the abstract socket namespace is already
protected by Android's sandbox, but the SO_PEERCRED check prevents any
process with filesystem or namespace access from gaining MCP privileges.

### 3. PTY session isolation

PTY sessions (`terminal/pty.rs`) use standard POSIX `fork()`/`exec()`
within the app's process:

- Child processes inherit the app's UID — no privilege escalation.
- Each session gets its own PTY pair; sessions cannot read each other's
  I/O.
- `setsid()` + `TIOCSCTTY` ensure the child process has its own session
  and controlling terminal.
- File descriptor cleanup closes inherited FDs except the slave PTY.

### 4. JNI FFI boundary

JNI functions in `native/src/android/ffi.rs` are the bridge between Kotlin and Rust.
Security-relevant practices:

- Every `unsafe` block carries a `// SAFETY:` comment documenting the
  invariant.
- Raw pointers received from JNI are treated as opaque handles (e.g.,
  `ANativeWindow*` is never dereferenced directly in Rust; it is passed
  to wgpu which manages its own lifetime).
- Input validation happens at the JNI entry point (null checks, bounds
  checks on byte arrays).
- `#[unsafe(no_mangle)]` functions follow JNI naming conventions — no
  arbitrary C ABI exports.

### 5. Clipboard access (OSC 52)

Clipboard operations (read/write) are gated by Android permission checks
on the Kotlin side. The Rust MCP `clipboard` tool triggers a JNI callback
to the host app, which enforces the permission. Clipboard content is
never persisted or logged.

### 6. No arbitrary code execution from MCP

MCP tools are statically registered at compile time. The MCP protocol
accepts structured JSON-RPC requests, but tool dispatch is a fixed match
on tool names — there is no mechanism for an MCP client to load or
execute arbitrary code.

## Alternatives Considered

### SELinux policies / app-level sandboxing

- **Not applicable**: Torvox runs as a standard Android app. Custom
  SELinux policies require system-level changes that are outside the
  scope of an app.

### Capability-based security (Linux capabilities)

- **Deferred**: Android already applies capability restrictions per-app.
  Adding Linux capabilities would require root or system partition
  access.

### Encrypting MCP socket traffic

- **Rejected**: The socket is local-only (abstract namespace on Android)
  and protected by UID checks. Encryption adds complexity without
  meaningful security benefit for local IPC.

## Consequences

### Positive

- Clear, documented trust boundaries for all subsystems
- Defense-in-depth on MCP socket via SO_PEERCRED (not relying solely on
  Android sandbox)
- All `unsafe` code is auditable with `// SAFETY:` comments
- No special permissions required — standard Android app permissions

### Negative

- PTY children share the app's UID — a vulnerable shell process could
  access app-private files (mitigated by Android's per-app filesystem
  isolation)
- Clipboard content passes through JNI callbacks without encryption
  (acceptable for local-only IPC)

## Compliance

- All `unsafe` blocks have `// SAFETY:` comments (enforced by code review)
- MCP socket permissions checked at runtime (SO_PEERCRED verification)
- `cargo clippy --all -- --deny warnings` catches unsafe code violations
- No special Android permissions required (no INTERNET, no READ_EXTERNAL_STORAGE)
