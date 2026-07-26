//! End-to-end MCP test against a real PTY-backed shell (feature `live`).
//!
//! Spawns a genuine `/bin/sh` session, serves the MCP server over a temp Unix
//! socket, then drives it like an AI agent would: types a command, waits for
//! output, and asserts the shell output is visible through `read_scrollback`.

#![cfg(feature = "live")]

use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::sync::Arc;
use std::time::{Duration, Instant};

use mcp_server::live::LiveShellStore;
use mcp_server::{McpServer, SessionStore, serve_unix};
use serde_json::{Value, json};

fn spawn_live_server(store: Arc<LiveShellStore>) -> (std::path::PathBuf,) {
    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let dir = std::env::temp_dir();
    let n = COUNTER.fetch_add(1, Ordering::SeqCst);
    let socket = dir.join(format!("mcp-server-live-{}-{}.sock", std::process::id(), n));
    let _ = std::fs::remove_file(&socket);
    let s = socket.clone();
    std::thread::spawn(move || {
        let _ = serve_unix(&s, store as Arc<dyn SessionStore>, true);
    });
    std::thread::sleep(Duration::from_millis(150));
    (socket,)
}

fn request(socket: &std::path::Path, req: Value) -> Value {
    let mut stream = UnixStream::connect(socket).expect("connect");
    stream
        .set_read_timeout(Some(Duration::from_secs(5)))
        .unwrap();
    let mut reader = BufReader::new(stream.try_clone().unwrap());
    writeln!(stream, "{req}").unwrap();
    stream.flush().unwrap();
    let mut buf = String::new();
    reader
        .read_line(&mut buf)
        .expect("read response line")
        .to_string();
    serde_json::from_str(buf.trim()).expect("parse response")
}

#[test]
fn live_shell_echo_round_trip() {
    let store = Arc::new(LiveShellStore::new());
    let session_id = store.spawn_session("/bin/sh", 24, 80);
    let _server = McpServer::new(store.clone()).with_write_consent();

    let (socket,) = spawn_live_server(store);

    // 1) list sessions -> our live session is present
    let list = request(
        &socket,
        json!({
            "jsonrpc": "2.0",
            "method": "tools/call",
            "params": {"name": "list_sessions", "arguments": {}},
            "id": 1
        }),
    );
    let sessions = list["result"]["content"][0]["data"]["Sessions"]
        .as_array()
        .unwrap();
    assert_eq!(sessions.len(), 1);
    assert_eq!(sessions[0]["id"], session_id);

    // 2) drive the shell: type a command followed by Enter
    let marker = "MCP_LIVE_MARKER_42";
    let send = request(
        &socket,
        json!({
            "jsonrpc": "2.0",
            "method": "tools/call",
            "params": {"name": "send_input", "arguments": {"session_id": session_id, "data": format!("echo {marker}\n")}},
            "id": 2
        }),
    );
    assert!(
        send["result"]["content"][0]["text"]
            .as_str()
            .unwrap()
            .contains("wrote to PTY")
    );

    // 3) poll scrollback until the marker appears (shell output is observed)
    let deadline = Instant::now() + Duration::from_secs(5);
    let mut found = false;
    while Instant::now() < deadline {
        let sb = request(
            &socket,
            json!({
                "jsonrpc": "2.0",
                "method": "tools/call",
                "params": {"name": "read_scrollback", "arguments": {"session_id": session_id, "max_lines": 200}},
                "id": 3
            }),
        );
        let lines = sb["result"]["content"][0]["data"]["Scrollback"]
            .as_array()
            .unwrap();
        if lines
            .iter()
            .any(|l| l.as_str().unwrap_or("").contains(marker))
        {
            found = true;
            break;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    assert!(found, "marker '{marker}' never appeared in scrollback");

    let _ = std::fs::remove_file(&socket);
}

#[test]
fn live_signal_interrupts_session() {
    let store = Arc::new(LiveShellStore::new());
    let session_id = store.spawn_session("/bin/sh", 24, 80);
    let (socket,) = spawn_live_server(store);

    // Start a long-running foreground process, then interrupt it.
    let _ = request(
        &socket,
        json!({
            "jsonrpc": "2.0",
            "method": "tools/call",
            "params": {"name": "send_input", "arguments": {"session_id": session_id, "data": "sleep 30\n"}},
            "id": 1
        }),
    );
    std::thread::sleep(Duration::from_millis(300));
    let sig = request(
        &socket,
        json!({
            "jsonrpc": "2.0",
            "method": "tools/call",
            "params": {"name": "send_signal", "arguments": {"session_id": session_id, "signal": "SIGINT"}},
            "id": 2
        }),
    );
    assert!(
        sig["result"]["content"][0]["text"]
            .as_str()
            .unwrap()
            .contains("sent signal")
    );

    let _ = std::fs::remove_file(&socket);
}
