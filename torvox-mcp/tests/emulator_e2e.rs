//! Automated end-to-end MCP test on a real Android device / emulator.
//!
//! This drives the actual `torvox-mcp` Android binary (built with `--features
//! mock`) over a TCP socket forwarded via `adb`. It is the on-device proof that
//! the server, protocol, and transport work on Android — no root required,
//! because the `shell` user can listen on TCP (unlike Unix sockets).
//!
//! Gated behind `feature = "emulator"` and requires `TORVOX_EMULATOR_TEST=1`
//! so it never runs in ordinary CI without a connected device + prebuilt binary.
//!
//! Run with:
//! ```text
//! TORVOX_EMULATOR_TEST=1 cargo test --package torvox-mcp --features emulator \
//!     --test emulator_e2e
//! ```

#![cfg(feature = "emulator")]

use std::io::{BufRead, BufReader, Write};
use std::net::TcpStream;
use std::process::Command;
use std::time::{Duration, Instant};

use serde_json::{Value, json};

fn adb() -> String {
    std::env::var("ADB").unwrap_or_else(|_| "adb".to_string())
}

fn serial() -> String {
    std::env::var("ANDROID_SERIAL").unwrap_or_else(|_| "emulator-5554".to_string())
}

fn android_bin() -> String {
    if let Ok(path) = std::env::var("TORVOX_MCP_ANDROID_BIN") {
        return path;
    }
    // CARGO_MANIFEST_DIR is the crate dir; the android artifact lives under
    // the workspace-root target directory.
    let manifest = std::env::var("CARGO_MANIFEST_DIR").unwrap_or_else(|_| ".".to_string());
    let workspace = std::path::Path::new(&manifest)
        .parent()
        .unwrap_or_else(|| std::path::Path::new("."));
    workspace
        .join("target/x86_64-linux-android/debug/torvox-mcp")
        .to_string_lossy()
        .into_owned()
}

fn adb_run(args: &[&str]) -> std::process::Output {
    Command::new(adb())
        .arg("-s")
        .arg(serial())
        .args(args)
        .output()
        .expect("failed to run adb")
}

fn rpc(
    stream: &mut TcpStream,
    method: &str,
    params: Option<Value>,
    id: Option<u64>,
) -> Option<Value> {
    let mut msg = json!({ "jsonrpc": "2.0", "method": method });
    if let Some(p) = params {
        msg["params"] = p;
    }
    if let Some(i) = id {
        msg["id"] = json!(i);
    }
    stream
        .write_all(format!("{msg}\n").as_bytes())
        .expect("write request");
    stream.flush().expect("flush");
    if id.is_none() {
        return None;
    }
    stream
        .set_read_timeout(Some(Duration::from_secs(3)))
        .unwrap();
    let mut reader = BufReader::new(stream.try_clone().unwrap());
    let mut line = String::new();
    let n = reader.read_line(&mut line).expect("read response");
    if n == 0 {
        eprintln!("EMULATOR_E2E: server closed connection (EOF) on method {method}");
        return None;
    }
    match serde_json::from_str::<Value>(line.trim()) {
        Ok(v) => Some(v),
        Err(e) => {
            eprintln!("EMULATOR_E2E: parse fail on raw={line:?}: {e}");
            panic!("parse response: {e}");
        }
    }
}

#[test]
fn emulator_full_protocol_round_trip() {
    if std::env::var("TORVOX_EMULATOR_TEST").as_deref() != Ok("1") {
        eprintln!("EMULATOR_E2E: skipped (set TORVOX_EMULATOR_TEST=1 to run)");
        return;
    }

    let bin = android_bin();
    assert!(
        std::path::Path::new(&bin).exists(),
        "android binary not found at {bin}; build with: cargo ndk --target x86_64 --platform 21 build --package torvox-mcp --features mock"
    );

    // Always tear down the device server + forward, even on panic.
    let _cleanup = DeviceCleanup;

    // 1) deploy + launch the server on the device (rootless, TCP).
    adb_run(&["push", &bin, "/data/local/tmp/torvox-mcp"]);
    adb_run(&["shell", "chmod", "755", "/data/local/tmp/torvox-mcp"]);
    adb_run(&["shell", "pkill", "-f", "torvox-mcp"]);
    // Wait until any previous instance has actually exited.
    std::thread::sleep(Duration::from_millis(800));
    let port = 8700 + (std::process::id() % 300) as u16;
    let launch = format!(
        "nohup setsid /data/local/tmp/torvox-mcp --tcp 127.0.0.1:{port} --mock --write-consent > /data/local/tmp/mcp_run.log 2>&1 < /dev/null &"
    );
    // Pass the command directly to the device shell (no `sh -c` wrapper, which
    // would mangle the `nohup ... &` detachment).
    adb_run(&["shell", &launch]);

    adb_run(&["forward", "--remove", &format!("tcp:{port}")]);
    adb_run(&["forward", &format!("tcp:{port}"), &format!("tcp:{port}")]);

    // 2) connect + initialize, retrying until the server is up.
    let mut stream = connect_with_retry(port);
    let r = rpc(
        &mut stream,
        "initialize",
        Some(json!({"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"emu-test","version":"0"}})),
        Some(1),
    )
    .unwrap();
    assert_eq!(r["result"]["protocolVersion"], "2024-11-05");

    // 3) notification -> no response
    stream
        .set_read_timeout(Some(Duration::from_secs(1)))
        .unwrap();
    rpc(&mut stream, "notifications/initialized", None, None);
    let mut reader = BufReader::new(stream.try_clone().unwrap());
    let mut extra = String::new();
    let got_extra = reader.read_line(&mut extra).is_ok() && !extra.is_empty();
    assert!(!got_extra, "notification unexpectedly produced a response");

    // 4) tools/list == 21
    let r = rpc(&mut stream, "tools/list", None, Some(2)).unwrap();
    let names: Vec<String> = r["result"]["tools"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["name"].as_str().unwrap().to_string())
        .collect();
    assert_eq!(names.len(), 21, "tool count");

    // 5) list_sessions -> 1 mock session
    let r = rpc(
        &mut stream,
        "tools/call",
        Some(json!({"name":"list_sessions","arguments":{}})),
        Some(3),
    )
    .unwrap();
    let sessions = &r["result"]["content"][0]["data"]["Sessions"];
    assert_eq!(sessions.as_array().unwrap().len(), 1);

    // 6) send_input -> scrollback round-trip
    let marker = "EMU_MARKER_77";
    let r = rpc(
        &mut stream,
        "tools/call",
        Some(json!({"name":"send_input","arguments":{"session_id":1,"data":format!("echo {marker}\n")}})),
        Some(4),
    )
    .unwrap();
    assert!(
        r["result"]["content"][0]["text"]
            .as_str()
            .unwrap()
            .contains("wrote to PTY")
    );

    let deadline = Instant::now() + Duration::from_secs(5);
    let mut found = false;
    while Instant::now() < deadline {
        let r = rpc(
            &mut stream,
            "tools/call",
            Some(json!({"name":"read_scrollback","arguments":{"session_id":1,"max_lines":200}})),
            Some(5),
        )
        .unwrap();
        let lines = r["result"]["content"][0]["data"]["Scrollback"]
            .as_array()
            .unwrap();
        if lines
            .iter()
            .any(|l| l.as_str().unwrap_or("").contains(marker))
        {
            found = true;
            break;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    assert!(found, "marker '{marker}' not found in scrollback");

    // 7) clipboard read/write
    rpc(
        &mut stream,
        "tools/call",
        Some(json!({"name":"write_clipboard","arguments":{"text":"hello-clip"}})),
        Some(6),
    );
    let r = rpc(
        &mut stream,
        "tools/call",
        Some(json!({"name":"read_clipboard","arguments":{}})),
        Some(7),
    )
    .unwrap();
    assert_eq!(r["result"]["content"][0]["data"]["Clipboard"], "hello-clip");

    // 8) unknown tool -> error
    let r = rpc(
        &mut stream,
        "tools/call",
        Some(json!({"name":"nope","arguments":{}})),
        Some(8),
    )
    .unwrap();
    assert!(r.get("error").is_some(), "unknown tool should error");

    // 9) scrollback_search
    rpc(
        &mut stream,
        "tools/call",
        Some(
            json!({"name":"send_input","arguments":{"session_id":1,"data":"unique-search-token-xyz\n"}}),
        ),
        Some(9),
    );
    let mut searched = false;
    let deadline = Instant::now() + Duration::from_secs(5);
    while Instant::now() < deadline {
        let r = rpc(
            &mut stream,
            "tools/call",
            Some(json!({"name":"scrollback_search","arguments":{"session_id":1,"pattern":"unique-search-token-xyz","max_matches":5}})),
            Some(10),
        )
        .unwrap();
        if !r["result"]["content"][0]["data"]["SearchMatches"]
            .as_array()
            .unwrap()
            .is_empty()
        {
            searched = true;
            break;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    assert!(searched, "scrollback_search found nothing");

    stream.flush().ok();
    drop(stream);
    eprintln!("EMULATOR_E2E: PASS");
}

/// Tear down the on-device server and the adb forward on scope exit.
struct DeviceCleanup;

impl Drop for DeviceCleanup {
    fn drop(&mut self) {
        adb_run(&["shell", "pkill", "-f", "torvox-mcp"]);
        // removing all torvox forwards is safest; tests use a small port range.
        for p in 8700..9000u16 {
            adb_run(&["forward", "--remove", &format!("tcp:{p}")]);
        }
    }
}

/// Connect to the forwarded port and run `initialize`, retrying until the
/// device server is ready (it is launched asynchronously).
fn connect_with_retry(port: u16) -> TcpStream {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        if let Ok(mut s) = TcpStream::connect(("127.0.0.1", port)) {
            s.set_read_timeout(Some(Duration::from_secs(2))).ok();
            let init = rpc(
                &mut s,
                "initialize",
                Some(
                    json!({"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"emu-test","version":"0"}}),
                ),
                Some(999),
            );
            if init.is_some() {
                return s;
            }
        }
        if Instant::now() > deadline {
            panic!("device MCP server did not become ready on port {port}");
        }
        std::thread::sleep(Duration::from_millis(250));
    }
}
