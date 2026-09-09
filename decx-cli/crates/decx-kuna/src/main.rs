//! decx-kuna-server — kuna as a DECX-contract engine server.
//!
//! Usage: `decx-kuna-server <target-binary> --port N`
//!
//! Built on decx-server-sdk: same wire contract as decx-server and every
//! other engine (`GET /health`, `POST /api/decx/<endpoint>`), one
//! `cargo build --release` alongside the CLI.

use std::path::PathBuf;

use decx_cli_core::error::{DecxError, DecxResult};
use decx_kuna::KunaService;
use decx_server_sdk::{bind, serve};

fn main() {
    if let Err(err) = run() {
        eprintln!("  [ERR] {} ({})", err.message, err.code);
        std::process::exit(err.exit_code);
    }
}

fn run() -> DecxResult<()> {
    let mut target: Option<PathBuf> = None;
    let mut port = 25419u16;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--port" => {
                port = args
                    .next()
                    .and_then(|v| v.parse().ok())
                    .ok_or_else(|| DecxError::usage("--port requires a value"))?;
            }
            other if !other.starts_with('-') => target = Some(PathBuf::from(other)),
            other => return Err(DecxError::usage(format!("unknown argument '{other}'"))),
        }
    }
    let target = target.ok_or_else(|| DecxError::usage("usage: decx-kuna-server <target-binary> --port N"))?;
    if !target.exists() {
        return Err(DecxError::file(
            format!("Target not found: {}", target.display()),
            Some(target.display().to_string()),
        ));
    }

    let service = KunaService::load(&target)?;
    let listener = bind(port)?;
    serve(listener, std::sync::Arc::new(service))
}
