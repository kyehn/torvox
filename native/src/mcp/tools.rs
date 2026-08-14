//! MCP 工具工厂：12 个工具的定义与输入结构。
//!
//! 工具 handler 通过 `super::global_state()` 读取终端状态；本模块不持有
//! 服务器生命周期或路由（见 `mcp/mod.rs`）。

use base64::Engine as _;
use serde::Deserialize;
use serde_json::json;
use std::sync::atomic::Ordering;
use std::time::Duration;
use tower_mcp::{CallToolResult, Tool, ToolBuilder, schemars::JsonSchema};

use super::global_state;

/// Input for clipboard_set.
#[derive(JsonSchema, Deserialize)]
pub(crate) struct ClipboardSetInput {
    text: String,
}

/// Input for notify.
#[derive(JsonSchema, Deserialize)]
pub(crate) struct NotifyInput {
    #[serde(default)]
    title: String,
    #[serde(default)]
    body: String,
}

/// Input for toast.
#[derive(JsonSchema, Deserialize)]
pub(crate) struct ToastInput {
    text: String,
}

/// Input for open_url.
#[derive(JsonSchema, Deserialize)]
pub(crate) struct OpenUrlInput {
    url: String,
}

/// Input for send_signal.
#[derive(JsonSchema, Deserialize)]
pub(crate) struct SendSignalInput {
    signal: i32,
}

// ── Tool definitions ─────────────────────────────────────────────────────

pub(crate) fn terminal_info_tool() -> Tool {
    ToolBuilder::new("terminal_info")
        .title("Get terminal info")
        .description(
            "Get terminal dimensions (rows, columns), version info, the \
             active session's exit code (null while it is still running) \
             and its current working directory (cwd, null when unknown)",
        )
        .no_params_handler(|| async move {
            let state = global_state();
            let rows = state.0.terminal_rows.load(Ordering::Acquire);
            let cols = state.0.terminal_cols.load(Ordering::Acquire);
            // Spec d4: terminal_info MUST include the session exit_code.
            // Resolved live from the registry for the active session so an
            // exited session reports its real code (e.g. 3).
            let exit_code = {
                let session_id = state.0.active_session_id.load(Ordering::Acquire);
                if session_id == 0 {
                    None
                } else {
                    crate::android::ffi::session_exit_code(session_id)
                }
            };
            // cwd: resolved live from the active session via the
            // JNI-registered handler. Source is session.cwd() — OSC 7
            // (shell-tracked) when the shell emits it, otherwise the
            // terminal's /proc-derived cwd fallback (session.rs:752).
            // null when no session or the handler is not registered.
            let cwd = {
                let session_id = state.0.active_session_id.load(Ordering::Acquire);
                let handler = state.0.on_session_cwd.lock();
                if session_id == 0 {
                    None
                } else {
                    handler.as_ref().and_then(|f| f(session_id))
                }
            };
            let info = json!({
                "rows": rows,
                "columns": cols,
                "version": env!("CARGO_PKG_VERSION"),
                "exit_code": exit_code,
                "cwd": cwd,
            });
            Ok(CallToolResult::text(info.to_string()))
        })
        .build()
}

pub(crate) fn clipboard_get_tool() -> Tool {
    ToolBuilder::new("clipboard_get")
        .title("Get clipboard content")
        .description("Read the current system clipboard text")
        .no_params_handler(|| async move {
            let (session_id, pending) = {
                let state = global_state();
                let guard = state.0.on_clipboard_get.lock();
                let session_id = state.0.active_session_id.load(Ordering::Acquire);
                (session_id, guard.as_ref().map(|f| f()))
            }; // guard dropped before await
            match pending {
                Some((request_id, rx)) => {
                    match tokio::time::timeout(Duration::from_secs(300), rx).await {
                        // Empty text is a legitimate result (empty clipboard),
                        // not a failure. Only a dropped sender (session closed)
                        // or the timeout is reported as an error.
                        Ok(Ok(text)) => Ok(CallToolResult::text(text)),
                        _ => {
                            crate::android::ffi::cancel_request(session_id, request_id);
                            Ok(CallToolResult::error(
                                "Clipboard read cancelled or timed out",
                            ))
                        }
                    }
                }
                None => Ok(CallToolResult::error(
                    "Clipboard not available on this platform",
                )),
            }
        })
        .build()
}

pub(crate) fn clipboard_set_tool() -> Tool {
    ToolBuilder::new("clipboard_set")
        .title("Set clipboard content")
        .description("Write text to the system clipboard")
        .handler(|input: ClipboardSetInput| async move {
            let state = global_state();
            let guard = state.0.on_clipboard_set.lock();
            match guard.as_ref() {
                Some(f) => {
                    f(input.text);
                    Ok(CallToolResult::text("Clipboard updated"))
                }
                None => Ok(CallToolResult::error("Clipboard not available")),
            }
        })
        .build()
}

pub(crate) fn notify_tool() -> Tool {
    ToolBuilder::new("notify")
        .title("Send notification")
        .description("Show a system notification with title and body")
        .handler(|input: NotifyInput| async move {
            let state = global_state();
            let guard = state.0.on_notify.lock();
            match guard.as_ref() {
                Some(f) => {
                    let msg = if input.body.is_empty() {
                        input.title
                    } else {
                        format!("{}\n{}", input.title, input.body)
                    };
                    f(msg);
                    Ok(CallToolResult::text("Notification sent"))
                }
                None => Ok(CallToolResult::error("Notifications not available")),
            }
        })
        .build()
}

pub(crate) fn toast_tool() -> Tool {
    ToolBuilder::new("toast")
        .title("Show toast")
        .description("Show a brief toast message on screen")
        .handler(|input: ToastInput| async move {
            let state = global_state();
            let guard = state.0.on_toast.lock();
            match guard.as_ref() {
                Some(f) => {
                    f(input.text);
                    Ok(CallToolResult::text("Toast shown"))
                }
                None => Ok(CallToolResult::error("Toast not available")),
            }
        })
        .build()
}

pub(crate) fn open_url_tool() -> Tool {
    ToolBuilder::new("open_url")
        .title("Open URL")
        .description("Open a URL in the default browser")
        .handler(|input: OpenUrlInput| async move {
            let state = global_state();
            let guard = state.0.on_open_url.lock();
            match guard.as_ref() {
                Some(f) => {
                    f(input.url);
                    Ok(CallToolResult::text("URL opened"))
                }
                None => Ok(CallToolResult::error("URL opening not available")),
            }
        })
        .build()
}

pub(crate) fn send_signal_tool() -> Tool {
    ToolBuilder::new("send_signal")
        .title("Send signal to terminal")
        .description("Send a POSIX signal (by number) to the foreground process in the active terminal session. Common signals: 2 (SIGINT), 3 (SIGQUIT), 9 (SIGKILL), 15 (SIGTERM), 20 (SIGTSTP).")
        //  note: the handler synchronously takes the session
        // registry read lock + the session lock on the MCP worker thread
        // (no await inside). It blocks up to a pollEvent frame or a 50ms
        // focus-event query while the UI thread holds the session lock —
        // brief and deadlock-free (lock order is the global one), but it
        // DOES briefly block the MCP worker, unlike the pure snapshot
        // reads of the other tools.
        .handler(|input: SendSignalInput| async move {
            let state = global_state();
            let guard = state.0.on_send_signal.lock();
            match guard.as_ref() {
                Some(f) => {
                    let session_id = state.0.active_session_id.load(Ordering::Acquire);
                    let result = f(session_id, input.signal);
                    Ok(CallToolResult::text(result))
                }
                None => Ok(CallToolResult::error("send_signal not available")),
            }
        })
        .build()
}

pub(crate) fn last_command_output_tool() -> Tool {
    ToolBuilder::new("last_command_output")
        .title("Get last command output")
        .description(
            "Return the text output of the last completed shell command in the active terminal \
             session, captured via OSC 133 shell integration (termlib getLastCommandOutput \
             equivalent). Empty when the shell does not emit OSC 133 markers or no command \
             finished yet. Reading drains the buffer.",
        )
        .no_params_handler(|| async move {
            let state = global_state();
            let session_id = state.0.active_session_id.load(Ordering::Acquire);
            if session_id == 0 {
                return Ok(CallToolResult::error("no active session"));
            }
            match crate::android::ffi::session_last_command_output(session_id) {
                Some(output) => Ok(CallToolResult::text(output)),
                None => Ok(CallToolResult::text("")),
            }
        })
        .build()
}

pub(crate) fn pick_file_tool() -> Tool {
    #[derive(Deserialize, JsonSchema)]
    struct PickFileInput {
        /// Starting directory
        #[serde(default)]
        directory: String,
        /// File filter pattern (e.g. "*.txt")
        #[serde(default)]
        pattern: String,
    }

    ToolBuilder::new("pick_file")
        .title("Pick file")
        .description("Open a system file picker dialog")
        .handler(|input: PickFileInput| async move {
            let (session_id, rx) = {
                let state = global_state();
                let guard = state.0.on_pick_file.lock();
                let session_id = state.0.active_session_id.load(Ordering::Acquire);
                (
                    session_id,
                    guard
                        .as_ref()
                        .map(|callback| callback(session_id, input.directory, input.pattern)),
                )
            }; // guard + state drop before await
            match rx {
                Some((request_id, rx)) => {
                    match tokio::time::timeout(Duration::from_secs(300), rx).await {
                        Ok(Ok(path)) if !path.is_empty() => Ok(CallToolResult::text(path)),
                        _ => {
                            // Drop the pending registry entry so a
                            // never-answered picker cannot leak one Sender
                            // per call. The request_id is only known here,
                            // after the callback registered it.
                            crate::android::ffi::cancel_request(session_id, request_id);
                            Ok(CallToolResult::error("File picker cancelled or timed out"))
                        }
                    }
                }
                None => Ok(CallToolResult::error("File picker not available")),
            }
        })
        .build()
}

pub(crate) fn dialog_tool() -> Tool {
    #[derive(Deserialize, JsonSchema)]
    struct DialogInput {
        /// "confirm", "input", or "select"
        dialog_type: String,
        title: String,
        message: String,
        /// Options for "select" type (ignored for confirm/input)
        #[serde(default)]
        options: Vec<String>,
    }

    ToolBuilder::new("dialog")
        .title("Show dialog")
        .description("Prompt the user with a dialog (confirm, input, or select)")
        .handler(|input: DialogInput| async move {
            let (session_id, rx) = {
                let state = global_state();
                let guard = state.0.on_show_dialog.lock();
                let session_id = state.0.active_session_id.load(Ordering::Acquire);
                (
                    session_id,
                    guard.as_ref().map(|callback| {
                        callback(
                            session_id,
                            input.dialog_type,
                            input.title,
                            input.message,
                            input.options,
                        )
                    }),
                )
            }; // guard + state drop before await
            match rx {
                Some((request_id, rx)) => {
                    match tokio::time::timeout(Duration::from_secs(300), rx).await {
                        Ok(Ok(result)) => Ok(CallToolResult::text(result)),
                        _ => {
                            crate::android::ffi::cancel_request(session_id, request_id);
                            Ok(CallToolResult::error("Dialog cancelled or timed out"))
                        }
                    }
                }
                None => Ok(CallToolResult::error("Dialog not available")),
            }
        })
        .build()
}

// ── Router construction ──────────────────────────────────────────────────

/// Risk classification for `run_command` input.
///
/// Modeled on the three-level classifier in sushi-ssh's CommandSafety
/// (SAFE / CONFIRM / BLOCKED). torvox has no CONFIRM dialog (excluded by
/// the user), so commands are either Safe or Blocked; blocked commands are
/// refused **before** the Kotlin host executes anything.
///
/// The command never reaches a shell — the Kotlin host tokenizes it to
/// argv (no `sh -c`), so shell metacharacters are inert. The patterns
/// below therefore match the raw string for the destructive shapes that
/// would survive tokenization (`rm -rf /`, `mkfs.*`, `dd of=/dev/...`,
/// fork bombs, recursive root chmod/chown, system control commands).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CommandRisk {
    Safe,
    Blocked,
}

/// Normalize a command for matching: lowercase, collapse whitespace,
/// strip a leading `sudo`/`env`/`nice` prefix so `sudo rm -rf /` and
/// `rm -rf /` classify identically.
pub(crate) fn normalized_command(command: &str) -> String {
    let mut normalized = command.trim().to_lowercase();
    for prefix in ["sudo ", "env ", "nice ", "command "] {
        if let Some(rest) = normalized.strip_prefix(prefix) {
            normalized = rest.trim_start().to_string();
            break;
        }
    }
    // Collapse runs of whitespace so `rm -rf  /` matches `rm -rf /`.
    let mut collapsed = String::with_capacity(normalized.len());
    let mut in_space = false;
    for c in normalized.chars() {
        if c.is_whitespace() {
            if !in_space {
                collapsed.push(' ');
            }
            in_space = true;
        } else {
            collapsed.push(c);
            in_space = false;
        }
    }
    collapsed.trim().to_string()
}

/// The first whitespace-delimited token, used as argv[0] for system
/// control command matching (`reboot` as an argument is not dangerous).
pub(crate) fn first_token(command: &str) -> &str {
    command.split_whitespace().next().unwrap_or("")
}

pub(crate) fn classify_command(command: &str) -> CommandRisk {
    let normalized = normalized_command(command);
    if normalized.is_empty() {
        return CommandRisk::Safe;
    }

    // 1. Root filesystem deletion: `rm -rf /` and direct variants. The
    //    flags may be combined in any order (`-fr`, `-rfv`) or split
    //    (`-r -f`, `--recursive --force`); the target must be `/` or `/*`.
    let rm_root = [
        "rm -rf /",
        "rm -fr /",
        "rm -r -f /",
        "rm -f -r /",
        "rm -rf /*",
        "rm -fr /*",
        "rm -r -f /*",
        "rm -f -r /*",
        "rm --recursive --force /",
        "rm --force --recursive /",
        "rm -rf --no-preserve-root /",
        "rm -rf / --no-preserve-root",
    ];
    if normalized.starts_with("rm ")
        && rm_root
            .iter()
            .any(|p| normalized == *p || normalized.starts_with(&format!("{p} ")))
    {
        return CommandRisk::Blocked;
    }

    // 2. Filesystem formatting: `mkfs.*`, `mkswap`.
    let argv0 = first_token(&normalized);
    if argv0 == "mkfs" || argv0.starts_with("mkfs.") || argv0 == "mkswap" || argv0 == "mkfs.ext4" {
        return CommandRisk::Blocked;
    }

    // 3. Raw block-device writes: `dd of=/dev/<dev>` (of=/dev/null is
    //    harmless), `shred /dev/<dev>`.
    if argv0 == "dd"
        && let Some(of_pos) = normalized.find("of=/dev/")
    {
        let target = &normalized[of_pos + "of=/dev/".len()..];
        if !target.starts_with("null") {
            return CommandRisk::Blocked;
        }
    }
    if argv0 == "shred" && normalized.contains("/dev/") {
        return CommandRisk::Blocked;
    }

    // 4. Fork bombs: bash function definitions that recurse in the
    //    background (`:{:|:& };:`, `f() { f | f & }; f`). The `{`
    //    signature (or ` {` with a background `&`) is unmistakable.
    if normalized.contains("(){") || (normalized.contains("() {") && normalized.contains('&')) {
        return CommandRisk::Blocked;
    }

    // 5. Recursive permission destruction on the root: `chmod -R 777 /`,
    //    `chown -R root /` (the `-r` substring also covers `-R`, `-r`,
    //    and `--recursive` after lowercasing).
    if let Some(args) = normalized.strip_prefix("chmod ")
        && args.contains("-r")
        && (args.ends_with(" /") || args.ends_with(" /*"))
    {
        return CommandRisk::Blocked;
    }
    if normalized.starts_with("chown ")
        && (normalized.ends_with(" /") || normalized.ends_with(" /*"))
    {
        return CommandRisk::Blocked;
    }

    // 6. System control: shutdown/reboot/poweroff/halt as the program name.
    if matches!(argv0, "shutdown" | "reboot" | "poweroff" | "halt") {
        return CommandRisk::Blocked;
    }

    CommandRisk::Safe
}

pub(crate) fn run_command_tool() -> Tool {
    #[derive(Deserialize, JsonSchema)]
    struct RunCommandInput {
        /// Raw command line, e.g. `echo "hello world" | wc -c`. Tokenized
        /// into argv by the Kotlin host with ArgumentTokenizer (DrJava
        /// 4-state machine) — NEVER passed through `sh -c`, so shell
        /// metacharacters (`;`, `|`, `&&`, redirection, globbing) are
        /// inert data, not syntax.
        command: String,
    }

    ToolBuilder::new("run_command")
        .title("Run a command in the terminal session")
        .description(
            "Execute a raw command string safely (no sh -c): the string is \
             tokenized to argv with quote/backslash rules and executed with \
             the app's environment. Returns the exit code plus captured \
             stdout/stderr. Shell metacharacters are NOT interpreted.",
        )
        .handler(|input: RunCommandInput| async move {
            // refuse destructive commands before the Kotlin
            // host executes anything. The error text is returned directly
            // to the MCP client.
            if classify_command(&input.command) == CommandRisk::Blocked {
                return Ok(CallToolResult::error(
                    "BLOCKED: command refused by safety classifier (destructive pattern)",
                ));
            }
            let (session_id, rx) = {
                let state = global_state();
                let guard = state.0.on_run_command.lock();
                let session_id = state.0.active_session_id.load(Ordering::Acquire);
                (
                    session_id,
                    guard
                        .as_ref()
                        .map(|callback| callback(session_id, input.command.clone())),
                )
            }; // guard + state drop before await
            match rx {
                Some((request_id, rx)) => {
                    match tokio::time::timeout(Duration::from_secs(300), rx).await {
                        Ok(Ok(result)) => Ok(CallToolResult::text(result)),
                        _ => {
                            crate::android::ffi::cancel_request(session_id, request_id);
                            Ok(CallToolResult::error("run_command cancelled or timed out"))
                        }
                    }
                }
                None => Ok(CallToolResult::error("run_command not available")),
            }
        })
        .build()
}

/// MCP tool: capture the current terminal screen as a PNG image.
pub(crate) fn screenshot_tool() -> Tool {
    ToolBuilder::new("screenshot")
        .title("Screenshot terminal screen")
        .description(
            "Capture the current terminal rendering as a base64-encoded PNG image. \
             Returns the image dimensions and raw pixel data.",
        )
        .handler(|_input: ()| async move {
            let state = global_state();
            let session_id = state.0.active_session_id.load(Ordering::Acquire);
            if session_id == 0 {
                return Ok(CallToolResult::error("No active terminal session"));
            }
            let rx = {
                let guard = state.0.on_screenshot.lock();
                guard
                    .as_ref()
                    .map(|callback| callback(session_id))
            };
            match rx {
                Some((request_id, rx)) => {
                    match tokio::time::timeout(Duration::from_secs(10), rx).await {
                        Ok(Ok((width, height, rgba_bytes))) => {
                            let b64 = base64::engine::general_purpose::STANDARD
                                .encode(&rgba_bytes);
                            Ok(CallToolResult::text(format!(
                                "{{\"width\":{width},\"height\":{height},\"format\":\"rgba\",\"data\":\"{b64}\"}}"
                            )))
                        }
                        _ => {
                            crate::android::ffi::cancel_request(session_id, request_id);
                            Ok(CallToolResult::error("Screenshot cancelled or timed out"))
                        }
                    }
                }
                None => Ok(CallToolResult::error(
                    "Screenshot not available (no handler registered)",
                )),
            }
        })
        .build()
}
