//! Tool parameters. Doc comments become the JSON schema descriptions the
//! agent sees.

use context_api::types::{
    RenderFormat,
    SearchView,
    Split,
};
use schemars::JsonSchema;
use serde::{
    Deserialize,
    Serialize,
};

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct GraphParams {
    /// Graph name in the store (letters, digits, '.', '_', '-').
    pub graph: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct CreateGraphParams {
    /// Name of the new graph (letters, digits, '.', '_', '-').
    pub graph: String,
    /// Texts to read; each string is one input. Empty: create an empty graph.
    #[serde(default)]
    pub texts: Vec<String>,
    /// Split every text into several inputs: `none` (default), `lines`,
    /// `paragraphs`.
    #[serde(default)]
    pub split: Option<Split>,
    /// Replace an existing graph (needs a server started with
    /// --allow-delete).
    #[serde(default)]
    pub overwrite: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ReadParams {
    /// Graph to read into.
    pub graph: String,
    /// Texts to read; each string is one input.
    #[serde(default)]
    pub texts: Vec<String>,
    /// Files to read (only if the server allows file access); relative
    /// paths are resolved below the allowed directory.
    #[serde(default)]
    pub files: Vec<String>,
    /// Split every text/file into several inputs: `none` (default), `lines`,
    /// `paragraphs`.
    #[serde(default)]
    pub split: Option<Split>,
    /// Create the graph if it does not exist.
    #[serde(default)]
    pub create: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct SearchParams {
    /// Graph to search.
    pub graph: String,
    /// The exact string to find (case-sensitive).
    pub query: String,
    /// `summary` (default): where the query is. `occurrences`: every
    /// position in the read inputs with line, column and snippet.
    /// `contexts`: distinct larger vertices containing it, narrowest first.
    /// `parents`: direct parent vertices. `path`: the raw search path.
    #[serde(default)]
    pub view: Option<SearchView>,
    /// Maximum number of listed items (default 20).
    #[serde(default)]
    pub limit: Option<usize>,
    /// Snippet chars on each side of a match (default 40).
    #[serde(default)]
    pub width: Option<usize>,
    /// Maximum label length of vertices in chars.
    #[serde(default)]
    pub label_width: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct VertexParams {
    /// Graph of the vertex.
    pub graph: String,
    /// `"#123"` for vertex index 123, otherwise the vertex's exact text.
    pub vertex: String,
    /// Maximum number of listed parents.
    #[serde(default)]
    pub limit: Option<usize>,
    /// Maximum label length in chars.
    #[serde(default)]
    pub label_width: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct RenderParams {
    /// Graph to draw.
    pub graph: String,
    /// `mermaid` (default), `dot`, `ascii`, `ascii_dag` or `json`
    /// (vertices and edges as data).
    #[serde(default)]
    pub format: Option<RenderFormat>,
    /// Draw only the descendants of this vertex: `"#123"` or its exact text.
    #[serde(default)]
    pub root: Option<String>,
    /// Maximum child steps below `root`.
    #[serde(default)]
    pub depth: Option<usize>,
}
