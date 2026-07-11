# Style Guide

## Shell Scripts

All shell scripts use Nushell (`.nu`). No bash or sh.

- Shebang: `#!/usr/bin/env -S nix develop --command nu`
- `snake_case` naming

### Deterministic script rules

The environment is deterministic: SDK paths, tool availability, and system state are fixed at runtime. Scripts must match this.

- Single source of truth: don't check the same condition two ways (`ps` AND `adb` to detect emulator). Pick one command and trust its output.
- No `ps | where name =~ ...` process-table scanning: use the tool's native status output (`adb get-state`, `adb shell getprop`).
- No `do --ignore-errors`: let non-zero exits propagate. Use `try/catch` ONLY when the failure IS an expected state (e.g., `adb` with no device), never for error masking.
- No `err> /dev/null` or `e>| null`: stderr is diagnostic output. If a command's error message is noise, the command is wrong.
- No `which X` for tool lookup: tools guaranteed by `nix develop`. Hardcode SDK root paths when they are fixed in CI.
- No process-liveness collateral checks in wait loops: check only the signal you care about (`boot_completed`), not whether the process table entry still exists.
- No `job spawn` for work that must complete before proceeding — `job spawn` is fire-and-forget. Use it only for background processes whose completion is polled separately.

### Forbidden patterns

- No abbreviated CLI flags: use `--target` not `-t`, `--package` not `-p`, `--replace-existing` not `-r`, `--downgrade` not `-d`, `--overwrite` not `-o`, `--force` not `-f`, `--recursive` not `-r`, `--dereference` not `-L`, `--directory` not `-d`, `--strip` not `-p`, `--list` not `-l`, `--deny` not `-D`, `--maxdepth` not `-maxdepth`, `--name` not `-name`, `--type` not `-type`, `--not-path` not `-not -path`, `--dynamic` not `-d`, `--parents` not `-p`, `--in-place` not `-i`, `--raw` not `-f` (for save)
- No `else { print }` fallback blocks — errors propagate naturally
- No `try {} catch {}` for commands that should simply fail
- No `| ignore` to suppress expected failures — use explicit error handling
- No `print "=== step_name ==="` step labels — output should only be results
- No useless output: no `print "Done!"`, `print "Boot verified"`, etc.
- No redundant `which X | length` checks when shebang already enters nix develop
- No intermediate variable aliases that are used once (`let sdkmanager = ...`, `let adb = ...`) — use the path directly
- No env var shadowing: don't set `$env.AVD_DIR = $avd_home` — use `$env.ANDROID_AVD_HOME` directly
- No `if ($dir | path exists)` for directories that MUST exist — let commands fail with clear errors
- For directories that SHOULD exist: check explicitly and exit with non-zero if missing
- No intermediate variables like `let start = ... let elapsed = ...` that add no clarity
- No `$env.ANDROID_HOME/platform-tools/adb` or hardcoded path bin usage — `adb`, `emulator`, `sdkmanager`, `avdmanager` come from nix devShell (`android-tools` package)
- No `nu scripts/xxx.nu` inside nu scripts — use `./scripts/xxx.nu` (shebang) or `nix develop --command "nu scripts/xxx.nu"`
- No multi-level single directories for vendored sources — clone directly to `vendor/<name>/` not `vendor/<name>/src/`
- No `rustup target add` or cross-compilation targets in check scripts — only workspace tests
- No silent `if ($dir | path exists)` for maestro/flows directories — let `ls` fail naturally if missing
- No useless intermediate variables that add no clarity (e.g., `let android_sdk = $env.ANDROID_HOME` used once)
- Hardcoded numeric thresholds must use named constants (e.g., `let minimum_apk_size_bytes = 5_000_000`)
- Script parameters for tunables (timeout, retries, ports) — never hardcode inside function body
- `match` expressions preferred over `if/else` chains for value mapping

### Style rules

- Expand all variables to descriptive names: no `s`, `p`, `w`, `h`, `t`, `e` single-letter variables
- Functions and variables: full words, no abbreviations (`config` not `cfg`, `background` not `bg`, `application` not `app`)
- Nushell: use `is-not-empty` / `is-empty` instead of `| length > 0` / `| length == 0`

## Nix

All environment management via Nix. No system shell builds.

- Always: `nix develop`, `nix develop --command "cargo build"`, `nix fmt`
- No abbreviated variable names
- ShellHook is the primary mechanism; checks and formatter defined in flake.nix

## GitHub Actions

- Action versions: default branch (`@main` or `@master`), not tags
- Exception: `reactivecircus/android-emulator-runner@v2` — `@main` has no compiled node_modules
- Exception: `emulator -avd` — no `--avd` long form exists in Android Emulator
- Exception: `adb install -r` — `--replace-existing` unsupported by API 35 PackageManager
- No step `name`
- Merge adjacent `run` steps into multi-line blocks
- `||` only for explicit error handling, never for error swallowing
- kebab-case job naming

## General

- No abbreviated variable names
- Inline intermediate variables when possible
- One document per topic, no duplication

## Implementation Notes (current code reality)

These are post-overhaul facts that code in this repository must match. They are the
contract for new work; violating them regresses a fixed bug.

### Keyboard encoder

- `torvox-terminal` encodes keys with libghostty-vt's `key::Encoder` +
  `key::Event`. These are allocated **once per `GhosttyTerminal` worker** and
  **reused** across keystrokes (`ghostty_terminal.rs`). Per-keypress allocation
  is a regression (loses per-encoder state).
- Encoder modes are re-synced before every key via
  `encoder.set_options_from_terminal(&terminal)` (honors DECCKM, Kitty keyboard
  protocol, alt-esc, modifyOtherKeys, keypad app mode).
- Field contract (per `key/event.h`): `utf8` = produced text **without**
  Ctrl/Alt; `unshifted_codepoint` = base key with **no** modifiers. The two are
  **distinct** values. C0 controls (`U+0000..U+001F`, `U+007F`) must be passed
  as `None` (null) so the encoder uses the logical key. When SHIFT only changed
  the printed character (`unicode_char != unshifted_char`), SHIFT is stripped
  from the modifier set.
- The Kotlin bridge supplies `unshifted_char`; falling back to `unicode_char`
  for both fields is only allowed when `unshifted_char` is absent.

### IME mode architecture

- `KeyboardMode` (`android/app/.../ui/KeyboardMode.kt`) is a sealed interface
  with `Secure`, `Standard`, `Raw`, and `Custom` variants.
- `Secure` uses `TYPE_TEXT_VARIATION_VISIBLE_PASSWORD |
  TYPE_TEXT_FLAG_NO_SUGGESTIONS` (CJK/voice/swipe composition still works). It
  must **not** use `TYPE_NULL` (that kills IME composition — a known regression).
- The IME input-connection path (`BaseInputConnection`) must handle
  `setComposingText` / `finishComposingText` so composing input method editors don't lose
  deltas.
- Input-mode signals (alt-screen / DECCKM / bracketed-paste) are exposed in the
  snapshot `meta` array; wiring `meta[14]` to restart the IME on alt-screen is
  the recommended hardening (do not add a blocking contract that other stages
  own).

### OSC 7 current working directory handling

- OSC 7 (`OSC_CWD = 7`) is in `HANDLED_OSC` in `osc_handler.rs` and is
  intercepted before it reaches the VT engine. `dispatch_osc7` emits
  `OscEvent::Cwd`, which the session stores in `Session::cwd`.
- The doc comment in `osc_handler.rs` listing `7 — current working directory`
  must stay in sync with `HANDLED_OSC`. Do not re-add the old dead
  `OscEvent::Cwd(_) => {}` no-op in `session.rs`.

### PTY flags hygiene

- `pty.rs` sets `setsid()` + controlling terminal, enables `IUTF8`, clears
  `IXON`/`IXOFF`, sets `ws_xpixel`/`ws_ypixel`, and closes stray file descriptors (bounded
  scan). Keep these; they match termux-app's known-correct practice.
- `Drop` for `PtyPair` sends `SIGHUP`, waits `GRACEFUL_SHUTDOWN_TIMEOUT_MS`,
  then `SIGKILL`s and `waitpid`s (reaping the child, no zombie). Do not
  reintroduce `into_raw_fd` / `mem::forget(self)` — that leaks the child.
- `unsafe` is confined to `pty.rs` fork/exec and the gui-android FFI, each with
  a `// SAFETY:` comment. `torvox-core` and `torvox-renderer` remain
  `#![forbid(unsafe_code)]`.
