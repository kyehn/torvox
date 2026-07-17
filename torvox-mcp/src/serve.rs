//! MCP server runners — TCP and Unix socket transports.

use std::io::BufRead;
use std::path::Path;
use std::sync::Arc;

use serde_json::{json, Value};

use crate::server::McpServer;
use crate::types::{JsonRpcRequest, SessionStore};

/// Build an `McpServer` from a store, applying write-consent when requested.
fn build_server(store: Arc<dyn SessionStore>, write_consent: bool) -> Arc<McpServer> {
    let mut server = McpServer::new(store);
    if write_consent {
        server = server.with_write_consent();
    }
    Arc::new(server)
}

/// Socket types that can be split into an independent reader handle.
trait CloneStream: std::io::Read + std::io::Write {
    fn try_clone_stream(&self) -> std::io::Result<Self>
    where
        Self: Sized;
}

impl CloneStream for std::net::TcpStream {
    fn try_clone_stream(&self) -> std::io::Result<Self> {
        self.try_clone()
    }
}

impl CloneStream for std::os::unix::net::UnixStream {
    fn try_clone_stream(&self) -> std::io::Result<Self> {
        self.try_clone()
    }
}

/// Serve a single JSON-RPC connection: read newline-delimited requests, answer
/// them, and silently drop parse errors / notifications that carry no id.
fn handle_connection<S>(server: &Arc<McpServer>, mut socket: S)
where
    S: CloneStream,
{
    let reader = match socket.try_clone_stream() {
        Ok(cloned) => std::io::BufReader::new(cloned),
        Err(_) => return,
    };
    for line in reader.lines().map_while(Result::ok) {
        let line = line.trim().to_string();
        if line.is_empty() {
            continue;
        }
        let req = match serde_json::from_str::<JsonRpcRequest>(&line) {
            Ok(req) => req,
            Err(e) => {
                let env = json!({
                    "jsonrpc": "2.0",
                    "id": Value::Null,
                    "error": {
                        "code": -32700,
                        "message": format!("parse error: {e}"),
                    },
                });
                if writeln!(socket, "{env}").is_err() {
                    return;
                }
                let _ = socket.flush();
                continue;
            }
        };
        // JSON-RPC notifications carry no id and must not be answered.
        if req.id.is_null() {
            let _ = server.handle(&req);
            continue;
        }
        let response = match server.handle(&req) {
            Ok(result) => json!({
                "jsonrpc": "2.0",
                "id": req.id,
                "result": result,
            }),
            Err(e) => e.to_json_rpc_error(&req.id),
        };
        if writeln!(socket, "{response}").is_err() {
            return;
        }
        let _ = socket.flush();
    }
}

/// Serve the MCP protocol over a TCP listener (e.g. for `adb forward`).
///
/// This is the rootless alternative to `serve_unix` on Android, where the
/// `shell` domain cannot bind Unix-domain sockets but can listen on TCP.
pub fn serve_tcp(
    addr: &str,
    store: Arc<dyn SessionStore>,
    write_consent: bool,
) -> std::io::Result<()> {
    use std::net::TcpListener;

    let listener = TcpListener::bind(addr)?;
    let server = build_server(store, write_consent);
    for stream in listener.incoming() {
        match stream {
            Ok(initial_stream) => {
                let server = Arc::clone(&server);
                std::thread::spawn(move || handle_connection(&server, initial_stream));
            }
            Err(e) => {
                log::error!("mcp: accept failed: {e}");
            }
        }
    }
    Ok(())
}

pub fn serve_unix(
    socket_path: &Path,
    store: Arc<dyn SessionStore>,
    write_consent: bool,
) -> std::io::Result<()> {
    use std::os::unix::net::UnixListener;

    if let Some(parent) = socket_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    if let Err(error) = std::fs::remove_file(socket_path) {
        log::warn!("mcp: failed to remove existing socket at {socket_path:?}: {error}");
    }
    let listener = UnixListener::bind(socket_path)?;

    let server = build_server(store, write_consent);

    for stream in listener.incoming() {
        match stream {
            Ok(initial_stream) => {
                let server = Arc::clone(&server);
                std::thread::spawn(move || handle_connection(&server, initial_stream));
            }
            Err(e) => {
                log::error!("mcp: accept failed: {e}");
            }
        }
    }
    Ok(())
}
