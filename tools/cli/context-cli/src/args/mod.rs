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

    /// Run against a server (client mode, stage 4).
    #[arg(long, global = true, env = "CONTEXT_SERVER", value_name = "URL")]
    pub server: Option<String>,

    /// Run in process even if a server is configured.
    #[arg(long, global = true)]
    pub local: bool,

    /// Output format (`json` = the Response envelope).
    #[arg(long, value_enum, value_name = "FORMAT")]
    pub format: Option<OutputFormat>,

    /// No progress or warnings on stderr.
    #[arg(short, long, global = true)]
    pub quiet: bool,

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
        };
        own.or(self.format).unwrap_or(OutputFormat::Text)
    }
}
