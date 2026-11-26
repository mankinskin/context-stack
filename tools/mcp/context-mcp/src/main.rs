//! `context-mcp` — MCP server for context-engine graphs over stdio.
//!
//! stdout is reserved for MCP JSON-RPC; all logs go to stderr.

use std::path::PathBuf;

use clap::Parser;
use context_mcp::{
    DEFAULT_MAX_OUTPUT,
    DEFAULT_RENDER_CAP,
    McpConfig,
};

#[derive(Debug, Parser)]
#[command(
    name = "context-mcp",
    version,
    about = "MCP server for context-engine graphs (stdio)"
)]
struct Args {
    /// Directory of the graph store.
    #[arg(long, env = "CONTEXT_STORE", default_value = ".context-engine")]
    store: PathBuf,
    /// Allow `files` inputs below this directory.
    #[arg(long, value_name = "DIR")]
    allow_fs: Option<PathBuf>,
    /// Maximum size of one tool result, e.g. 32768, 64KiB, 1MiB.
    #[arg(long, value_name = "BYTES", value_parser = parse_size, default_value_t = DEFAULT_MAX_OUTPUT)]
    max_output: usize,
    /// Register `context_delete_graph` and allow overwriting graphs.
    #[arg(long)]
    allow_delete: bool,
    /// Hard vertex cap of `context_render`.
    #[arg(long, value_name = "N", default_value_t = DEFAULT_RENDER_CAP)]
    render_cap: usize,
}

fn parse_size(text: &str) -> Result<usize, String> {
    let text = text.trim();
    let split = text
        .find(|c: char| !c.is_ascii_digit())
        .unwrap_or(text.len());
    let (number, unit) = text.split_at(split);
    let number: usize = number
        .parse()
        .map_err(|_| format!("invalid size '{text}'"))?;
    let factor = match unit.trim().to_ascii_lowercase().as_str() {
        "" | "b" => 1,
        "k" | "kb" | "kib" => 1024,
        "m" | "mb" | "mib" => 1024 * 1024,
        _ =>
            return Err(format!(
                "invalid size unit in '{text}' (use B, KiB or MiB)"
            )),
    };
    Ok(number * factor)
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| {
                    tracing_subscriber::EnvFilter::new("context_mcp=info")
                }),
        )
        .with_writer(std::io::stderr)
        .init();

    let args = Args::parse();
    let config = McpConfig {
        store_dir: args.store,
        allow_fs: args.allow_fs,
        max_output: args.max_output,
        allow_delete: args.allow_delete,
        render_cap: Some(args.render_cap),
    };
    if let Err(err) = context_mcp::run_stdio(config).await {
        eprintln!("context-mcp: {err}");
        std::process::exit(1);
    }
}
