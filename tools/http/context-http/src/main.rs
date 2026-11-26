//! `context-http` — runs the HTTP and WebSocket server of the context API.
//!
//! Prints `{"listening":"http://ADDR","pid":PID}` on stdout once bound;
//! logs go to stderr (`RUST_LOG`).

use clap::Parser;
use context_http::ServeArgs;
use tracing_subscriber::EnvFilter;

#[derive(Parser)]
#[command(
    name = "context-http",
    version,
    about = "HTTP and WebSocket server of the context API"
)]
struct Cli {
    #[command(flatten)]
    serve: ServeArgs,
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .with_writer(std::io::stderr)
        .init();
    let config = Cli::parse().serve.into_config();
    if let Err(err) = context_http::serve(config, shutdown_signal()).await {
        eprintln!("context-http: {err}");
        std::process::exit(1);
    }
}

/// Ctrl-C or SIGTERM.
async fn shutdown_signal() {
    let ctrl_c = async {
        let _ = tokio::signal::ctrl_c().await;
    };
    #[cfg(unix)]
    let terminate = async {
        match tokio::signal::unix::signal(
            tokio::signal::unix::SignalKind::terminate(),
        ) {
            Ok(mut signal) => {
                signal.recv().await;
            },
            Err(_) => std::future::pending().await,
        }
    };
    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();
    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }
    tracing::info!("shutting down");
}
