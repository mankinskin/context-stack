//! Shared setup of the long-running modes (`serve`, `mcp`): a tokio
//! runtime and stderr logging.

use crate::error::EXIT_COMMAND;

/// Log to stderr (`RUST_LOG`, else `default`); stdout stays reserved for
/// the listening line (serve) or JSON-RPC (mcp).
pub fn init_logging(default: &str) {
    let filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new(default));
    let _ = tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_writer(std::io::stderr)
        .try_init();
}

/// Run `future` on a multi-threaded runtime; errors are printed as
/// `error: …` and exit 1.
pub fn block_on<E: std::fmt::Display>(
    future: impl Future<Output = Result<(), E>>
) -> u8 {
    let runtime = match tokio::runtime::Runtime::new() {
        Ok(runtime) => runtime,
        Err(err) => {
            eprintln!("error: cannot start the async runtime: {err}");
            return EXIT_COMMAND;
        },
    };
    match runtime.block_on(future) {
        Ok(()) => 0,
        Err(err) => {
            eprintln!("error: {err}");
            EXIT_COMMAND
        },
    }
}
