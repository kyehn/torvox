//! MCP server binary.
//!
//! The server speaks JSON-RPC 2.0 over stdio or a Unix domain socket and
//! exposes terminal-session inspection/manipulation tools to AI agents.
//!
//! By default it runs with a no-op backend (`NoOpStore`) so the protocol can
//! be exercised without a GUI. With `--live` it spawns real shell sessions via
//! `terminal-engine` (feature `live`), which is what powers end-to-end testing
//! on the host and inside the Android emulator.

use std::path::PathBuf;
use std::sync::Arc;

use clap::{Parser, ValueEnum};
use mcp_server::{SessionStore, serve_tcp, serve_unix};

#[derive(Parser)]
#[command(
    name = "mcp-server",
    about = "MCP server exposing terminal sessions to AI agents"
)]
struct Cli {
    /// Socket path (Unix domain socket). Required unless `--tcp` is given.
    #[arg(long, value_name = "PATH")]
    socket: Option<PathBuf>,

    /// TCP listen address, e.g. `127.0.0.1:8731` (rootless alternative to `--socket`).
    #[arg(long, value_name = "ADDR")]
    tcp: Option<String>,

    /// Allow write/signal tools without explicit per-call consent gating.
    #[arg(long)]
    write_consent: bool,

    /// Spawn a real shell session via terminal-engine and serve it (feature `live`).
    #[cfg(feature = "live")]
    #[arg(long)]
    live: bool,

    /// Serve an in-memory mock session (no GUI / no PTY). Useful for tests and demos.
    #[cfg(feature = "mock")]
    #[arg(long)]
    mock: bool,

    /// Shell to spawn in `--live` mode.
    #[cfg(feature = "live")]
    #[arg(long, default_value = "/bin/sh")]
    shell: String,

    /// Rows for the spawned `--live` session.
    #[cfg(feature = "live")]
    #[arg(long, default_value_t = 24)]
    rows: u32,

    /// Columns for the spawned `--live` session.
    #[cfg(feature = "live")]
    #[arg(long, default_value_t = 80)]
    cols: u32,

    /// Log level filter (e.g. info, debug).
    #[arg(long, value_enum, default_value_t = LogLevel::Info)]
    log: LogLevel,
}

#[derive(Copy, Clone, PartialEq, Eq, ValueEnum)]
enum LogLevel {
    Off,
    Error,
    Warn,
    Info,
    Debug,
    Trace,
}

impl LogLevel {
    fn as_str(self) -> &'static str {
        match self {
            LogLevel::Off => "off",
            LogLevel::Error => "error",
            LogLevel::Warn => "warn",
            LogLevel::Info => "info",
            LogLevel::Debug => "debug",
            LogLevel::Trace => "trace",
        }
    }
}

/// No-op backend: the binary cannot reach a running GUI session, so read tools
/// that need live state return an error and write tools are rejected.
struct NoOpStore;

impl SessionStore for NoOpStore {
    fn read(&self, req: mcp_server::ReadRequest) -> Result<mcp_server::ReadResponse, String> {
        match req {
            mcp_server::ReadRequest::Sessions => {
                Ok(mcp_server::ReadResponse::Sessions(Vec::<SessionInfo>::new()))
            }
            _ => Err("no GUI connected; cannot read session state".into()),
        }
    }
    fn write(&self, _: u32, _: Vec<u8>) -> Result<(), String> {
        Err("no GUI connected; cannot write to PTY".into())
    }
    fn signal(&self, _: u32, _: mcp_server::SignalKind) -> Result<(), String> {
        Err("no GUI connected; cannot send signal".into())
    }
}

use mcp_server::SessionInfo;
#[cfg(feature = "live")]
use mcp_server::live::LiveShellStore;
#[cfg(feature = "mock")]
use mcp_server::mock::MockStore;

fn main() {
    let cli = Cli::parse();

    let filter = format!(
        "mcp_server={},terminal_engine={},warn",
        cli.log.as_str(),
        cli.log.as_str()
    );
    let _ = env_logger::Builder::from_env(env_logger::Env::default().default_filter_or(filter))
        .try_init();

    let store: Arc<dyn SessionStore> = {
        #[cfg(feature = "live")]
        {
            if cli.live {
                let live = LiveShellStore::new();
                let id = live
                    .spawn_session(&cli.shell, cli.rows, cli.cols)
                    .unwrap_or_else(|e| {
                        eprintln!("error: {e}");
                        std::process::exit(1)
                    });
                log::info!("mcp: live session {id} spawned via {}", cli.shell);
                Arc::new(live)
            } else {
                #[cfg(feature = "mock")]
                {
                    if cli.mock {
                        log::info!("mcp: serving mock store");
                        Arc::new(MockStore::new())
                    } else {
                        Arc::new(NoOpStore)
                    }
                }
                #[cfg(not(feature = "mock"))]
                {
                    Arc::new(NoOpStore)
                }
            }
        }
        #[cfg(not(feature = "live"))]
        {
            #[cfg(feature = "mock")]
            {
                if cli.mock {
                    log::info!("mcp-server: serving mock store");
                    Arc::new(MockStore::new())
                } else {
                    Arc::new(NoOpStore)
                }
            }
            #[cfg(not(feature = "mock"))]
            {
                Arc::new(NoOpStore)
            }
        }
    };

    let write_consent = cli.write_consent || {
        #[cfg(feature = "live")]
        {
            cli.live
        }
        #[cfg(not(feature = "live"))]
        {
            false
        }
    };

    if let Some(addr) = &cli.tcp {
        if let Err(e) = serve_tcp(addr, store, write_consent) {
            eprintln!("mcp: server error: {e}");
            std::process::exit(1);
        }
    } else if let Some(path) = &cli.socket {
        if let Err(e) = serve_unix(path, store, write_consent) {
            eprintln!("mcp: server error: {e}");
            std::process::exit(1);
        }
    } else {
        eprintln!("mcp: either --socket PATH or --tcp ADDR is required");
        std::process::exit(1);
    }
}
