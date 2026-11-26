//! `context-cli mcp`: the MCP server on stdio, in process or as a proxy of
//! a running server (`--server`).

use std::sync::Arc;

use context_api::client::RemoteBackend;
use context_mcp::{
    ContextServer,
    McpConfig,
};

use crate::{
    args::{
        Cli,
        McpArgs,
    },
    daemon,
};

pub fn run(
    cli: &Cli,
    args: &McpArgs,
) -> u8 {
    daemon::init_logging("context_mcp=info");
    let config = McpConfig {
        store_dir: cli.store.clone(),
        allow_fs: args.allow_fs.clone(),
        max_output: args.max_output,
        allow_delete: args.allow_delete,
        render_cap: Some(args.render_cap),
    };
    let server = match cli
        .server
        .as_deref()
        .filter(|url| !cli.local && !url.is_empty())
    {
        Some(url) => {
            let remote = RemoteBackend::new(url, cli.token.clone());
            if let Err(err) = remote.health() {
                eprintln!("warning: {}", err.message);
            }
            eprintln!(
                "context-cli mcp: proxy mode, forwarding to {}",
                remote.url()
            );
            ContextServer::with_backend(Arc::new(remote), config)
        },
        None => ContextServer::new(config),
    };
    daemon::block_on(context_mcp::serve_stdio(server))
}
