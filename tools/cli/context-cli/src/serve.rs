//! `context-cli serve`: the context-http server.

use context_http::ServeArgs;

use crate::daemon;

/// Serve until Ctrl-C or SIGTERM; dirty graphs are saved on shutdown.
pub fn run(args: ServeArgs) -> u8 {
    daemon::init_logging("info");
    daemon::block_on(context_http::serve(args.into_config(), shutdown_signal()))
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
}
