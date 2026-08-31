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

/// Timeout for MCP tools that wait for user interaction (clipboard read/write,
/// file picker, dialog, run_command). Callers should expect up to 5 minutes of
/// blocking when invoking these tools.
const INTERACTIVE_TIMEOUT: Duration = Duration::from_secs(300);

/// Tokenize a raw command line into argv using POSIX shell-words rules.
/// Replaces hand-rolled `ArgumentTokenizer` (Kotlin DrJava port) for Rust-side
/// validation; Kotlin host still uses the same rules for exec. No `sh -c`
/// semantics: `;`/`|`/`&&` remain inert data.
// ponytail: shell-words crate covers POSIX quoting/escaping; hand-rolled state machine if perf matters
pub(crate) fn tokenize_command(command: &str) -> Result<Vec<String>, String> {
    shell_words::split(command).map_err(|e| e.to_string())
}

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
                    state.session_exit_code(session_id)
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
                    match tokio::time::timeout(INTERACTIVE_TIMEOUT, rx).await {
                        // Empty text is a legitimate result (empty clipboard),
                        // not a failure. Only a dropped sender (session closed)
                        // or the timeout is reported as an error.
                        Ok(Ok(text)) => Ok(CallToolResult::text(text)),
                        _ => {
                            global_state().cancel_request(session_id, request_id);
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
            match state.session_last_command_output(session_id) {
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
                    match tokio::time::timeout(INTERACTIVE_TIMEOUT, rx).await {
                        Ok(Ok(path)) if !path.is_empty() => Ok(CallToolResult::text(path)),
                        _ => {
                            // Drop the pending registry entry so a
                            // never-answered picker cannot leak one Sender
                            // per call. The request_id is only known here,
                            // after the callback registered it.
                            global_state().cancel_request(session_id, request_id);
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
                    match tokio::time::timeout(INTERACTIVE_TIMEOUT, rx).await {
                        Ok(Ok(result)) => Ok(CallToolResult::text(result)),
                        _ => {
                            global_state().cancel_request(session_id, request_id);
                            Ok(CallToolResult::error("Dialog cancelled or timed out"))
                        }
                    }
                }
                None => Ok(CallToolResult::error("Dialog not available")),
            }
        })
        .build()
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
            "Run a raw command line in the terminal session (no sh -c): the \
             string is tokenized to argv with quote/backslash rules and \
             executed with the app's environment. Returns the exit code plus \
             captured stdout/stderr. Shell metacharacters are NOT interpreted.",
        )
        .handler(|input: RunCommandInput| async move {
            if let Err(reason) = tokenize_command(&input.command) {
                return Ok(CallToolResult::error(format!(
                    "invalid command: {reason}"
                )));
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
                    match tokio::time::timeout(INTERACTIVE_TIMEOUT, rx).await {
                        Ok(Ok(result)) => Ok(CallToolResult::text(result)),
                        _ => {
                            global_state().cancel_request(session_id, request_id);
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
                guard.as_ref().map(|callback| callback(session_id))
            };
            match rx {
                Some((request_id, rx)) => {
                    match tokio::time::timeout(Duration::from_secs(10), rx).await {
                        Ok(Ok((width, height, rgba_bytes))) => {
                            let b64 = base64::engine::general_purpose::STANDARD.encode(&rgba_bytes);
                            Ok(CallToolResult::text(
                                json!({
                                    "width": width,
                                    "height": height,
                                    "format": "rgba",
                                    "data": b64,
                                })
                                .to_string(),
                            ))
                        }
                        _ => {
                            global_state().cancel_request(session_id, request_id);
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

#[cfg(test)]
mod tests {
    use super::*;

    // ──: tool definitions (identity + contract) ────────────────────

    fn all_tools() -> Vec<Tool> {
        vec![
            terminal_info_tool(),
            clipboard_get_tool(),
            clipboard_set_tool(),
            notify_tool(),
            toast_tool(),
            open_url_tool(),
            send_signal_tool(),
            last_command_output_tool(),
            pick_file_tool(),
            dialog_tool(),
            run_command_tool(),
            screenshot_tool(),
        ]
    }

    #[test]
    fn tool_names_are_unique_and_valid() {
        let mut names: Vec<String> = all_tools().into_iter().map(|tool| tool.name).collect();
        let original = names.clone();
        names.sort();
        names.dedup();
        assert_eq!(names.len(), original.len(), "tool names must be unique");
        // MCP spec: `^[a-zA-Z0-9_-]{1,64}$` — names are identifiers, so
        // neither dots nor long names are allowed.
        for name in names {
            assert!(
                !name.is_empty() && name.len() <= 64,
                "tool name must be 1..=64 chars: {name}"
            );
            assert!(
                name.chars()
                    .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-')),
                "tool name must be alphanumeric/_/-: {name}"
            );
        }
    }

    #[test]
    fn every_tool_has_title_description_and_schema() {
        for tool in all_tools() {
            assert!(
                tool.title.as_deref().is_some_and(|t| !t.is_empty()),
                "tool {} must have a non-empty title",
                tool.name
            );
            assert!(
                tool.description.as_deref().is_some_and(|d| !d.is_empty()),
                "tool {} must have a non-empty description",
                tool.name
            );
            assert!(
                tool.definition().input_schema.is_object()
                    || tool.definition().input_schema.is_null(),
                "tool {} must declare an object (or empty) input schema",
                tool.name
            );
        }
    }

    #[test]
    fn tokenize_command_handles_quotes_and_escapes() {
        assert_eq!(
            tokenize_command(r#"echo "hello world" --flag"#).unwrap(),
            vec!["echo", "hello world", "--flag"]
        );
        assert_eq!(
            tokenize_command("echo 'a b' c\\ d").unwrap(),
            vec!["echo", "a b", "c d"]
        );
        assert!(tokenize_command(r#"echo "unclosed"#).is_err());
    }
}
