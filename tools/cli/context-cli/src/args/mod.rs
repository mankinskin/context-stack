//! Command-line grammar (plan 02 §2.2).

pub mod graph;
pub mod input;
pub mod search;
pub mod show;

use std::path::PathBuf;

use clap::{
    Args,
    Parser,
    Subcommand,
    ValueEnum,
};

pub use self::{
    graph::*,
    input::InputArgs,
    search::SearchArgs,
    show::ShowArgs,
};

/// Read text into context graphs, search them and draw them.
#[derive(Debug, Parser)]
#[command(name = "context-cli", version, about, propagate_version = true)]
pub struct Cli {
    /// Graph store directory.
    #[arg(
        long,
        global = true,
        env = "CONTEXT_STORE",
        value_name = "DIR",
        default_value = ".context-engine"
    )]
    pub store: PathBuf,

    /// Run against the server at URL (client mode).
    #[arg(long, global = true, env = "CONTEXT_SERVER", value_name = "URL")]
    pub server: Option<String>,

    /// Bearer token of the server.
    #[arg(
        long,
        global = true,
        env = "CONTEXT_TOKEN",
        hide_env_values = true,
        value_name = "TOKEN"
    )]
    pub token: Option<String>,

    /// Run in process: ignore CONTEXT_SERVER and never auto-connect to a
    /// server holding the graph.
    #[arg(long, global = true)]
    pub local: bool,

    /// Output format (`json` = the Response envelope).
    #[arg(long, value_enum, value_name = "FORMAT")]
    pub format: Option<OutputFormat>,

    /// No progress or warnings on stderr.
    #[arg(short, long, global = true)]
    pub quiet: bool,

    #[command(subcommand)]
    pub command: TopCommand,
}

/// The commands of the binary: the graph commands (also the shell's line
/// grammar) and the long-running modes.
#[derive(Debug, Subcommand)]
pub enum TopCommand {
    #[command(flatten)]
    Graph(CliCommand),
    /// Interactive shell (local, or remote with --server / a discovered
    /// server); reads commands line by line when stdin is not a terminal.
    Shell(ShellArgs),
    /// Run the HTTP and WebSocket server.
    #[cfg(feature = "server")]
    Serve(context_http::ServeArgs),
    /// Run the MCP server on stdio (proxy mode with --server).
    #[cfg(feature = "mcp")]
    Mcp(McpArgs),
}

#[derive(Debug, Args)]
pub struct ShellArgs {
    /// Default graph of commands without a graph argument (`use` changes it).
    pub graph: Option<String>,
}

#[cfg(feature = "mcp")]
#[derive(Debug, Args)]
pub struct McpArgs {
    /// Allow `files` inputs below DIR (local mode).
    #[arg(long, value_name = "DIR")]
    pub allow_fs: Option<PathBuf>,
    /// Maximum size of one tool result, e.g. 32768, 64KiB, 1MiB.
    #[arg(long, value_name = "BYTES", value_parser = context_mcp::parse_size, default_value_t = context_mcp::DEFAULT_MAX_OUTPUT)]
    pub max_output: usize,
    /// Register `context_delete_graph` and allow overwriting graphs.
    #[arg(long)]
    pub allow_delete: bool,
    /// Hard vertex cap of `context_render`.
    #[arg(long, value_name = "N", default_value_t = context_mcp::DEFAULT_RENDER_CAP)]
    pub render_cap: usize,
}

/// One line of the shell: a graph command without the program name.
#[derive(Debug, Parser)]
#[command(
    name = "context",
    no_binary_name = true,
    disable_version_flag = true,
    override_usage = "<COMMAND> [ARGS]   (shell: use <GRAPH>, :help, :quit)"
)]
pub struct ShellLine {
    #[command(subcommand)]
    pub command: CliCommand,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum OutputFormat {
    Text,
    Json,
}

/// `--format text|json` of commands without an own `--format`.
#[derive(Debug, Clone, Copy, Default, Args)]
pub struct OutArgs {
    /// Output format (`json` = the Response envelope).
    #[arg(long = "format", value_enum, value_name = "FORMAT")]
    pub format: Option<OutputFormat>,
}

#[derive(Debug, Subcommand)]
pub enum CliCommand {
    /// Create an empty graph.
    New(NewArgs),
    /// Create a graph from INPUT (new + read + save).
    Create(CreateArgs),
    /// Read INPUT into a graph and save it.
    Read(ReadArgs),
    /// Search a graph for a string.
    Search(SearchArgs),
    /// Draw a graph or a subgraph (ascii, dot, mermaid, json).
    Show(ShowArgs),
    /// Show a vertex (`#index` or text): composition and parents.
    Vertex(VertexArgs),
    /// List the inputs read into a graph.
    Inputs(InputsArgs),
    /// Summary of a graph.
    Info(GraphArgs),
    /// Statistics of a graph.
    Stats(GraphArgs),
    /// Validate a graph's structure.
    Validate(GraphArgs),
    /// Export a graph (v3 interchange JSON or native .ctx bytes).
    Export(ExportArgs),
    /// Import a graph from a v3 interchange or native .ctx file.
    Import(ImportArgs),
    /// List the graphs in the store.
    List(OutArgs),
    /// Delete a graph.
    Delete(GraphArgs),
}

impl Cli {
    /// The output format: the command's `--format` wins over the one given
    /// before the command.
    pub fn output_format(&self) -> OutputFormat {
        let own = match &self.command {
            TopCommand::Graph(command) => command.own_format(),
            _ => None,
        };
        own.or(self.format).unwrap_or(OutputFormat::Text)
    }
}

impl CliCommand {
    /// The command's own `--format`.
    pub fn own_format(&self) -> Option<OutputFormat> {
        match self {
            CliCommand::New(a) => a.out.format,
            CliCommand::Create(a) => a.out.format,
            CliCommand::Read(a) => a.out.format,
            CliCommand::Search(a) => a.out.format,
            CliCommand::Vertex(a) => a.out.format,
            CliCommand::Inputs(a) => a.out.format,
            CliCommand::Info(a)
            | CliCommand::Stats(a)
            | CliCommand::Validate(a)
            | CliCommand::Delete(a) => a.out.format,
            CliCommand::Import(a) => a.out.format,
            CliCommand::List(a) => a.format,
            CliCommand::Show(_) | CliCommand::Export(_) => None,
        }
    }

    /// The command name, selecting the text renderer.
    pub fn name(&self) -> &'static str {
        match self {
            CliCommand::New(_) => "new",
            CliCommand::Create(_) => "create",
            CliCommand::Read(_) => "read",
            CliCommand::Search(_) => "search",
            CliCommand::Show(_) => "show",
            CliCommand::Vertex(_) => "vertex",
            CliCommand::Inputs(_) => "inputs",
            CliCommand::Info(_) => "info",
            CliCommand::Stats(_) => "stats",
            CliCommand::Validate(_) => "validate",
            CliCommand::Export(_) => "export",
            CliCommand::Import(_) => "import",
            CliCommand::List(_) => "list",
            CliCommand::Delete(_) => "delete",
        }
    }
}
