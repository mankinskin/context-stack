//! Arguments of the graph lifecycle and query commands.

use std::path::PathBuf;

use clap::{
    Args,
    ValueEnum,
};

use super::{
    InputArgs,
    OutArgs,
};

#[derive(Debug, Args)]
pub struct GraphArgs {
    /// Graph name in the store, or a path ending in .ctx.
    pub graph: String,
    #[command(flatten)]
    pub out: OutArgs,
}

#[derive(Debug, Args)]
pub struct NewArgs {
    /// Graph name in the store, or a path ending in .ctx.
    pub graph: String,
    /// Replace an existing graph.
    #[arg(long)]
    pub force: bool,
    #[command(flatten)]
    pub out: OutArgs,
}

#[derive(Debug, Args)]
pub struct CreateArgs {
    /// Graph name in the store, or a path ending in .ctx.
    pub graph: String,
    #[command(flatten)]
    pub input: InputArgs,
    /// Replace an existing graph.
    #[arg(long)]
    pub force: bool,
    #[command(flatten)]
    pub out: OutArgs,
}

#[derive(Debug, Args)]
pub struct ReadArgs {
    /// Graph name in the store, or a path ending in .ctx.
    pub graph: String,
    #[command(flatten)]
    pub input: InputArgs,
    /// Create the graph if it does not exist.
    #[arg(long)]
    pub create: bool,
    /// Report what would change without saving.
    #[arg(long)]
    pub dry_run: bool,
    /// Include the reader steps in the report.
    #[arg(long)]
    pub steps: bool,
    #[command(flatten)]
    pub out: OutArgs,
}

#[derive(Debug, Args)]
pub struct VertexArgs {
    /// Graph name in the store, or a path ending in .ctx.
    pub graph: String,
    /// `#index` or the vertex text.
    #[arg(allow_hyphen_values = true)]
    pub vertex: String,
    /// Maximum number of parents.
    #[arg(long, value_name = "N")]
    pub limit: Option<usize>,
    /// Maximum label length in chars.
    #[arg(long, value_name = "N")]
    pub label_width: Option<usize>,
    #[command(flatten)]
    pub out: OutArgs,
}

#[derive(Debug, Args)]
pub struct InputsArgs {
    /// Graph name in the store, or a path ending in .ctx.
    pub graph: String,
    /// Maximum number of inputs listed.
    #[arg(long, value_name = "N")]
    pub limit: Option<usize>,
    #[command(flatten)]
    pub out: OutArgs,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum ExportFormatArg {
    /// Content-keyed v3 interchange JSON (oracle format).
    V3,
    /// The native .ctx file bytes.
    Native,
}

#[derive(Debug, Args)]
pub struct ExportArgs {
    /// Graph name in the store, or a path ending in .ctx.
    pub graph: String,
    /// Export format.
    #[arg(long, value_enum, default_value = "v3")]
    pub format: ExportFormatArg,
    /// Write to FILE instead of stdout (required for native in text mode).
    #[arg(short, long, value_name = "FILE")]
    pub output: Option<PathBuf>,
    /// Lift the default size gate of the v3 export.
    #[arg(long)]
    pub force: bool,
}

#[derive(Debug, Args)]
pub struct ImportArgs {
    /// Graph name in the store, or a path ending in .ctx.
    pub graph: String,
    /// A v3 interchange JSON file or a native .ctx file.
    pub file: PathBuf,
    /// Replace an existing graph.
    #[arg(long)]
    pub force: bool,
    #[command(flatten)]
    pub out: OutArgs,
}
