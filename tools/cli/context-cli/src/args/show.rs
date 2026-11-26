//! Arguments of `show`.

use std::path::{
    Path,
    PathBuf,
};

use clap::{
    Args,
    ValueEnum,
};
use context_api::types::RenderFormat;

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum ShowFormat {
    /// Layered ASCII drawing.
    Ascii,
    /// ASCII DAG drawing.
    AsciiDag,
    /// Graphviz dot.
    Dot,
    /// Mermaid flowchart.
    Mermaid,
    /// Vertices and edges as JSON.
    Json,
}

impl From<ShowFormat> for RenderFormat {
    fn from(format: ShowFormat) -> Self {
        match format {
            ShowFormat::Ascii => RenderFormat::Ascii,
            ShowFormat::AsciiDag => RenderFormat::AsciiDag,
            ShowFormat::Dot => RenderFormat::Dot,
            ShowFormat::Mermaid => RenderFormat::Mermaid,
            ShowFormat::Json => RenderFormat::Json,
        }
    }
}

#[derive(Debug, Args)]
pub struct ShowArgs {
    /// Graph name in the store, or a path ending in .ctx.
    pub graph: String,
    /// Render only the descendants of this vertex (`#index` or text).
    #[arg(long, value_name = "VERTEX", allow_hyphen_values = true)]
    pub root: Option<String>,
    /// Maximum child steps below --root.
    #[arg(long, value_name = "N")]
    pub depth: Option<usize>,
    /// Drawing format (default: from the -o extension, else ascii).
    #[arg(long, value_enum)]
    pub format: Option<ShowFormat>,
    /// Vertex limit of the size gate.
    #[arg(long, value_name = "N")]
    pub max_vertices: Option<usize>,
    /// Lift the default size gate.
    #[arg(long)]
    pub force: bool,
    /// Write to FILE (atomically) instead of stdout.
    #[arg(short, long, value_name = "FILE")]
    pub output: Option<PathBuf>,
}

impl ShowArgs {
    /// The drawing format and whether mermaid is wrapped in a fenced block
    /// (`-o *.md`).
    pub fn resolved_format(&self) -> (ShowFormat, bool) {
        let ext = self
            .output
            .as_deref()
            .and_then(Path::extension)
            .and_then(|ext| ext.to_str())
            .map(str::to_ascii_lowercase);
        let inferred = match ext.as_deref() {
            Some("dot" | "gv") => ShowFormat::Dot,
            Some("mmd" | "mermaid" | "md") => ShowFormat::Mermaid,
            Some("json") => ShowFormat::Json,
            _ => ShowFormat::Ascii,
        };
        let format = self.format.unwrap_or(inferred);
        let fenced =
            format == ShowFormat::Mermaid && ext.as_deref() == Some("md");
        (format, fenced)
    }
}
