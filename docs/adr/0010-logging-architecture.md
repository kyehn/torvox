# 10. Logging Architecture

## Status

Accepted

Date: 2026-09-05

## Requirement IDs

NFR-025, NFR-028

## Context

Torvox is a Rust-native Android terminal emulator with logging needs
across multiple subsystems: JNI FFI entry/exit, PTY session lifecycle,
GPU rendering pipeline, font loading, MCP server operations, and Ghostty
terminal processing. Logs must be visible in Android's `logcat` for
debugging while avoiding performance overhead in the render hot path.

The original torvox relied on ad-hoc `println!` and `eprintln!` calls
that were invisible in logcat and unstructured. The current codebase uses
the `log` crate facade extensively (200+ log call sites across `native/src/`)
but the routing infrastructure was added incrementally without a unifying
architecture document.

Key challenges:

- **Logcat message limit**: Android's `__android_log_write` truncates
  messages at ~4 KB. Long messages (stack traces, shader compilation
  errors) must be chunked.
- **Module-level filtering**: Noisy subsystems (`wgpu_hal`, `naga`,
  `wgpu_core`) produce excessive debug output that obscures torvox's own
  logs.
- **Platform divergence**: The `log` crate is used on Android (→ logcat),
  but desktop builds need a different backend.

## Decision

### 1. Log facade: `log` crate

All Rust code uses the `log` crate macros (`log::info!`, `log::debug!`,
etc.) for logging. This is a compile-time facade — no backend is selected
at compile time. The backend is registered at runtime.

### 2. Android backend: custom `log::Log` implementation

`native/src/android/logging.rs` implements the `log::Log` trait directly,
routing all log output to Android's logcat via the NDK function
`__android_log_write`. This replaces any default log backend and gives
full control over formatting and filtering.

Initialization happens exactly once, in `NativeBridge::initLogger()` (the
JNI entry point called by Kotlin at app startup). The `log::set_logger`
and `log::set_max_level` calls happen here — before any other Rust code
runs.

### 3. Logcat message chunking

`native/src/log_chunk.rs` splits log messages exceeding the logcat
capacity into multiple `__android_log_write` calls. The chunking uses a
configurable `LOGGER_SAFETY_MARGIN` (4 bytes) to account for logcat's
internal NUL terminator and priority prefix overhead. Each chunk is
prefixed with a sequence indicator for reassembly.

### 4. Module-level filtering

`native/src/android/mod.rs` implements `module_filtered()` to suppress
debug-level output from noisy upstream crates:

| Module | Debug-level filter | Rationale |
|--------|-------------------|-----------|
| `wgpu_hal` | Suppress (allow Error+) | Verbose hardware abstraction |
| `wgpu_core` | Allow Debug | Useful for GPU state debugging |
| `naga` | Allow Debug | Shader compilation diagnostics |
| All other | Allow Debug | Torvox's own subsystems |

This filter is applied in the `Log::enabled()` method so filtered log
calls never reach the formatting path.

### 5. Log level hierarchy

- **Error**: Fatal conditions — GPU initialization failure, render
  pipeline crashes, panic backtraces.
- **Warn**: Degraded but recoverable — font fallback, render warmup
  failures, MCP accept errors.
- **Info**: Lifecycle events — session create/destroy, surface
  attach/detach, MCP server start/stop.
- **Debug**: Operational detail — font selection, cell builder metrics,
  render frame timing.
- **Trace**: Hot path detail — texture acquisition, buffer writes (opt-in
  only in debug builds).

### 6. Panic routing

`android/logging.rs` installs a custom `std::panic::set_hook` that
routes panic information (message + backtrace) through the log system
(`log::error!`). This ensures panics appear in logcat rather than being
silently lost on Android.

### 7. Desktop logging

On non-Android targets (integration tests, desktop development), the
standard `env_logger` or `env_logger`-compatible backend is used via the
`log` crate's no-op default. The `android/logging.rs` code is
`#[cfg(target_os = "android")]` gated.

## Alternatives Considered

### `tracing` crate
- **Rejected**: The `tracing` crate adds structured logging, spans, and
  subscribers — significant complexity for a project that primarily needs
  simple log-level filtering and logcat output. The `log` crate's
  simplicity matches torvox's needs and avoids pulling in a large
  dependency tree.

### `android_logger` crate
- **Rejected**: The `android_logger` crate provides logcat routing but
  does not support message chunking or module-level filtering. Torvox's
  custom implementation handles both.

### `tracing-android` bridge
- **Rejected**: Would require migrating all 200+ `log::*` call sites to
  `tracing::*` for marginal benefit.

## Consequences

### Positive

- Single log facade (`log` crate) across all subsystems — no mixing of
  `println!`, `eprintln!`, and `log::*`
- Logcat messages are never truncated — chunking handles long messages
- Noisy upstream crates are filtered without affecting torvox's own logs
- Panic backtraces appear in logcat automatically
- Zero runtime cost when log level is below the enabled threshold

### Negative

- Custom `log::Log` implementation must be maintained (vs. using a
  published crate)
- Module-level filter list is static — changing it requires a code change
  (could be made configurable via settings in the future)
- The `log` crate does not support structured logging fields — all
  context is embedded in the format string

## Compliance

- Single `log` facade across all subsystems (enforced by `cargo clippy` —
  no `println!`/`eprintln!` allowed in production code)
- Logcat output verified via `adb logcat` in emulator tests
- Noisy crate filters maintained in `android_logger.rs` (static list)
