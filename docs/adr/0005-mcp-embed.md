# 0005 — MCP Architecture

- **Status**: Accepted
- **Date**: 2026-07-27
- **Requirement IDs**: FR-06, NFR-05

## Context

The original `mcp-server/` was a ~3.1 KLOC standalone Rust binary:
- CLI argument parsing (`clap`)
- JSON-RPC protocol implementation (serde + manual dispatch)
- Three store modes (NoOp, Mock, Live)
- Independent process lifecycle

Analysis showed that >90% of the code was protocol/CLI infrastructure.
The actual value — reading terminal state and writing to PTY on behalf of
an AI agent — requires only:

- Receiving a JSON-RPC 2.0 request from stdin or a Unix socket
- Reading `RenderState` data (visible grid, cursor, selection)
- Writing a command string to the PTY
- Returning the response

Haven's architecture confirms the better pattern: MCP tools should be
tightly coupled to terminal state, not a remote API.

## Decision

MCP is an **embedded module** in `native/src/mcp.rs`:

```rust
// native/src/mcp.rs — ~723 lines
pub fn build_router() -> McpRouter { ... }
pub fn start() { ... }
pub async fn run_stdio() -> Result<()> { ... }
```

- Uses **tower-mcp 0.14** (standard MCP protocol SDK) — proper `tools/list` + `tools/call` with JSON Schema via `schemars`
- Communication via **Unix socket** (embedded, app-launched) or **stdio** (standalone for AI coding agent CLIs)
- No CLI parser — configuration via environment variables
- No independent process — runs inside the app process
- The MCP handler reads session state through a **snapshot channel** rather
  than directly accessing the `!Send` Terminal. Each SessionThread
  periodically pushes a read-only session snapshot (visible text, cursor
  position, scrollback) into a lock-free `flume` channel. The MCP thread
  reads from this channel — see ADR-0004 (thread model) and ADR-0002 (data
  source).
- No three-store modes — one real mode

## Alternatives Considered

### Standalone server (status quo)
- **Rejected**: 3.1 KLOC for what's effectively a PTY proxy. The standalone
  process would need IPC to access terminal state anyway, negating the
  architectural independence.

### Remove entirely
- **Rejected**: AI integration is a stated differentiator for torvox
  (see AGENTS.md). Embedding keeps the capability while removing the cost.

## Consequences

### Positive

- ~2.5 KLOC removed from the codebase
- Zero serialization overhead — the MCP handler reads Ghostty state directly
- No process management, IPC, or CLI parsing complexity
- MCP is compiled in by default (`default = ["mcp"]`) so the JNI exports
  always exist; it can still be compiled out with `--no-default-features`
  for a smaller APK. The server is runtime-disabled until the user enables
  it in settings.

### Negative

- External AI agents still need a socket or pipe to connect (the app process
  must expose a Unix socket or use stdin/stdout from a parent)
- The MCP thread reads session state from a snapshot channel, not directly:
  the data is slightly stale (at most one frame old) but this is acceptable
  for AI agent latency
- JSON-RPC library is still needed (minimal: serde_json + basic dispatch)

## Compliance

- Always compiled in (implemented with mcp feature gate; test-util implies mcp)
- No `clap` or equivalent CLI dependency
- MCP module must not depend on any crate not already in the dependency tree

## Status Note (Jul 2026, updated Aug 2026)

This decision was **fully implemented** in Phase 7 of the re-architecture:

- MCP server lives at `native/src/mcp.rs` (~723 lines)
- Uses **tower-mcp 0.14** (proper MCP protocol, not hand-rolled JSON-RPC) instead of the originally proposed hand-written dispatch
- Provides **two transports**: `StdioTransport` (for AI coding agent CLIs) and `UnixSocketTransport` (for embedded use)
- Supports **8 standard MCP tools**: `terminal_info`, `clipboard_get`, `clipboard_set`, `notify`, `toast`, `open_url`, `pick_file`, `dialog`
- Implemented with mcp feature gate (test-util implies mcp) — ~60KB binary overhead when enabled
- Reads session state via snapshot channel (as originally designed)
- Removed: `clap` CLI, three store modes, standalone binary
