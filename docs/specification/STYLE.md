# Style Guide

## Shell Scripts

All shell scripts use Nushell (`.nu`). No bash or sh.

- Shebang: `#!/usr/bin/env -S nix develop --command nu`
- `snake_case` naming

### Deterministic script rules

The environment is deterministic: SDK paths, tool availability, and system state are fixed at runtime. Scripts must match this.

- No `do --ignore-errors`: let non-zero exits propagate. Use `try/catch` ONLY when the failure IS an expected state (e.g., `adb` with no device), never for error masking.
- No `err> /dev/null` or `e>| null`: stderr is diagnostic output. If a command's error message is noise, the command is wrong.
- No `which X` for tool lookup: tools guaranteed by `nix develop`. Hardcode SDK root paths when they are fixed in CI.

### Forbidden patterns

- No `else { print }` fallback blocks — errors propagate naturally
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
- No `nu scripts/xxx.nu` inside nu scripts — use `./scripts/xxx.nu` (shebang)
- No `rustup target add` or cross-compilation targets in check scripts — only workspace tests

### Style rules

- Expand all variables to descriptive names: no `s`, `p`, `w`, `h`, `t`, `e` single-letter variables
- Functions and variables: full words, no abbreviations (`config` not `cfg`, `background` not `bg`, `application` not `app`)
- Nushell: use `is-not-empty` / `is-empty` instead of `| length > 0` / `| length == 0`

## Nix

All environment management via Nix. No system shell builds.

- Always: `nix develop`
- No abbreviated variable names
- ShellHook is the primary mechanism; checks and formatter defined in flake.nix

## GitHub Actions

- Action versions: default branch (`@main` or `@master`), not tags
- Exception: `reactivecircus/android-emulator-runner@v2` — `@main` has no compiled node_modules
- No step `name`
- Merge adjacent `run` steps into multi-line blocks
- `||` only for explicit error handling, never for error swallowing
- kebab-case job naming

## General

- No abbreviated variable names
- Inline intermediate variables when possible
- One document per topic, no duplication
