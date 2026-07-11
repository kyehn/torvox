//! torvox-mcp binary entry point.
//!
//! Listens on a Unix domain socket for JSON-RPC 2.0 / MCP requests
//! that allow AI agents to inspect Torvox terminal sessions.

use std::path::PathBuf;
use std::sync::Arc;

use clap::Parser;
use torvox_mcp::{SessionInfo, SessionStore, serve_unix};

#[derive(Parser)]
#[command(
    name = "torvox-mcp",
    about = "Model Context Protocol server for Torvox terminal sessions"
)]
struct Cli {
    /// Unix domain socket path to listen on
    #[arg(long)]
    socket: PathBuf,

    /// Allow send_input tool to write to terminal PTY (DANGEROUS)
    #[arg(long)]
    mcp_allow_write: bool,
}

struct NoOpStore;

impl SessionStore for NoOpStore {
    fn read(&self, _: torvox_mcp::ReadRequest) -> Result<torvox_mcp::ReadResponse, String> {
        Ok(torvox_mcp::ReadResponse::Sessions(Vec::<SessionInfo>::new()))
    }
    fn write(&self, _: u32, _: Vec<u8>) -> Result<(), String> {
        Err("no GUI connected; cannot write to PTY".into())
    }
    fn signal(&self, _: u32, _: torvox_mcp::SignalKind) -> Result<(), String> {
        Err("no GUI connected; cannot send signal".into())
    }
}

fn main() -> std::process::ExitCode {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    let command_line = Cli::parse();

    let store: Arc<dyn SessionStore> = Arc::new(NoOpStore);

    if let Err(error) = serve_unix(&command_line.socket, store, command_line.mcp_allow_write) {
        log::error!("mcp: serve failed: {error}");
        return std::process::ExitCode::from(1);
    }
    std::process::ExitCode::SUCCESS
}
