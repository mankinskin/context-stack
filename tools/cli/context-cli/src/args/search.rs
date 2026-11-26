//! Arguments of `search`.

use clap::{
    Args,
    ValueEnum,
};
use context_api::types::SearchView;

use super::OutArgs;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, ValueEnum)]
pub enum ViewArg {
    /// Outcome only.
    #[default]
    Summary,
    /// The search path: coverage, span, start and end paths.
    Path,
    /// Distinct ancestors containing the match, narrowest first.
    Contexts,
    /// Positions of the query in the read inputs.
    Occurrences,
    /// Direct parents of the matched vertex.
    Parents,
}

impl From<ViewArg> for SearchView {
    fn from(view: ViewArg) -> Self {
        match view {
            ViewArg::Summary => SearchView::Summary,
            ViewArg::Path => SearchView::Path,
            ViewArg::Contexts => SearchView::Contexts,
            ViewArg::Occurrences => SearchView::Occurrences,
            ViewArg::Parents => SearchView::Parents,
        }
    }
}

#[derive(Debug, Args)]
pub struct SearchArgs {
    /// Graph name in the store, or a path ending in .ctx.
    pub graph: String,
    /// The text to search.
    #[arg(allow_hyphen_values = true)]
    pub query: String,
    /// What to report.
    #[arg(long, value_enum, default_value = "summary")]
    pub view: ViewArg,
    /// Maximum number of items (contexts, occurrences, parents).
    #[arg(long, value_name = "N")]
    pub limit: Option<usize>,
    /// Chars of context on each side of a snippet.
    #[arg(long, value_name = "N")]
    pub width: Option<usize>,
    /// Maximum number of parent steps from the match (contexts).
    #[arg(long, value_name = "N")]
    pub max_depth: Option<usize>,
    /// Maximum label length in chars.
    #[arg(long, value_name = "N")]
    pub label_width: Option<usize>,
    /// Exit 1 when the query is not (fully) found.
    #[arg(long)]
    pub fail_if_missing: bool,
    #[command(flatten)]
    pub out: OutArgs,
}
