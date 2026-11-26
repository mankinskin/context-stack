//! The rmcp server handler and its tools.

use std::{
    path::Path,
    sync::{
        Arc,
        Mutex,
    },
};

use context_api::{
    Backend,
    Command,
    ErrorKind,
    ErrorResponse,
    Outcome,
    Request,
    Response,
    types::{
        Gate,
        Input,
        Limits,
        RenderFormat,
        SavePolicy,
        SearchView,
        VertexRef,
    },
};
use rmcp::{
    ErrorData as McpError,
    ServerHandler,
    handler::server::{
        router::tool::ToolRouter,
        wrapper::Parameters,
    },
    model::{
        CallToolResult,
        Content,
        Implementation,
        ServerCapabilities,
        ServerInfo,
    },
    tool,
    tool_handler,
    tool_router,
};

use crate::{
    McpConfig,
    budget,
    params::*,
};

const SEARCH_LIMIT: usize = 20;
const SEARCH_WIDTH: usize = 40;

const HELP: &str = "\
context tools index text into a graph of shared substrings and answer \
exact-substring questions about it.

Workflow:
1. context_create_graph {graph, texts?}   create (and read texts into) a graph
   context_read {graph, texts|files}       add more inputs later (the only mutation)
2. context_search {graph, query, view}     find a string:
   - summary (default): is it in the graph, and where (vertex / inside a host)
   - occurrences: every position in the inputs, with line, column and snippet
   - contexts: distinct larger vertices containing it, narrowest first
   - parents: direct parent vertices; path: raw search path
3. context_vertex {graph, vertex}           inspect one vertex (\"#index\" or exact text)
   context_render {graph, root?, depth?}    draw a small (sub)graph as mermaid/dot/ascii
Also: context_list_graphs, context_graph_info, context_execute (raw Request).

Results are context-api Response JSON: {\"ok\": {...}} or {\"error\": {kind, message, hint}}.
Large results are shortened item-wise and marked \"truncated_output\": true; \
narrow them with limit, width or root/depth.";

const INSTRUCTIONS: &str = "Index text into named graphs and search it. Start with \
context_create_graph (texts) or context_read, then context_search (view summary, occurrences, \
contexts or parents), context_vertex and context_render. Call context_help for a short guide.";

/// The backend of every tool call.
///
/// Extension point for proxy mode (stage 4): a `RemoteBackend` forwarding
/// requests to a running context server implements
/// [`context_api::Backend`] and is passed to
/// [`ContextServer::with_backend`]; the tool code stays unchanged.
type SharedBackend = Arc<Mutex<Box<dyn Backend + Send>>>;

/// The MCP server handler. Serve it with any rmcp transport
/// (`server.serve(transport)`); [`crate::run_stdio`] uses stdio.
#[derive(Clone)]
pub struct ContextServer {
    backend: SharedBackend,
    config: Arc<McpConfig>,
    tool_router: ToolRouter<Self>,
}

impl ContextServer {
    /// Local mode: an in-process [`context_api::Session`] over the
    /// configured store.
    pub fn new(config: McpConfig) -> Self {
        let session = config.local_session();
        Self::with_backend(Box::new(session), config)
    }

    /// Serve the tools over any backend (local session or, later, a remote
    /// server). `config` still governs the MCP-side policies (output budget,
    /// delete, file paths, render cap).
    pub fn with_backend(
        backend: Box<dyn Backend + Send>,
        config: McpConfig,
    ) -> Self {
        let mut tool_router = Self::tool_router();
        if !config.allow_delete {
            tool_router.remove_route("context_delete_graph");
        }
        Self {
            backend: Arc::new(Mutex::new(backend)),
            config: Arc::new(config),
            tool_router,
        }
    }

    pub fn config(&self) -> &McpConfig {
        &self.config
    }

    /// Run a request on the backend in a blocking task.
    async fn run(
        &self,
        request: Request,
    ) -> Response {
        let backend = self.backend.clone();
        let id = request.id;
        tokio::task::spawn_blocking(move || {
            backend
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .execute(request)
        })
        .await
        .unwrap_or_else(|err| {
            error_response(
                id,
                ErrorKind::Internal,
                format!("tool task failed: {err}"),
                None,
            )
        })
    }

    /// Run a typed tool's command and convert the response.
    async fn typed(
        &self,
        command: Command,
    ) -> CallToolResult {
        if let Err(response) = self.check(&command) {
            return self.result(*response);
        }
        let mut response = self.run(Request::new(command)).await;
        self.rewrite_hint(&mut response);
        self.result(response)
    }

    /// MCP-side policies: graphs are store names (no paths), destructive
    /// commands need `--allow-delete`.
    fn check(
        &self,
        command: &Command,
    ) -> Result<(), Box<Response>> {
        if let Some(graph) = command.graph()
            && (graph.contains(['/', '\\']) || graph.ends_with(".ctx"))
        {
            return Err(Box::new(error_response(
                None,
                ErrorKind::Forbidden,
                format!(
                    "graph '{graph}': the MCP server only addresses graphs by name in its store"
                ),
                Some(
                    "use a plain name (letters, digits, '.', '_', '-')".into(),
                ),
            )));
        }
        let destructive = match command {
            Command::Delete { .. } => true,
            Command::New { overwrite, .. }
            | Command::Create { overwrite, .. }
            | Command::Import { overwrite, .. } => *overwrite,
            #[allow(unreachable_patterns)]
            other => matches!(other.name(), "delete_log" | "delete_logs"),
        };
        if destructive && !self.config.allow_delete {
            return Err(Box::new(error_response(
                None,
                ErrorKind::Forbidden,
                format!(
                    "'{}' deletes or replaces data; this MCP server was started without --allow-delete",
                    command.name()
                ),
                Some("use a new graph name instead".into()),
            )));
        }
        Ok(())
    }

    /// Replace CLI-oriented hints by tool-oriented ones.
    fn rewrite_hint(
        &self,
        response: &mut Response,
    ) {
        let Outcome::Error(error) = &mut response.outcome else {
            return;
        };
        let hint = match error.kind {
            ErrorKind::TooLarge => Some(format!(
                "render a subgraph: pass root (\"#index\" or a vertex text) and a small depth (1-2){}",
                self.config
                    .render_cap
                    .map(|cap| format!("; this server renders at most {cap} vertices"))
                    .unwrap_or_default()
            )),
            ErrorKind::NotFound if error.message.starts_with("graph '") => Some(
                "create it with context_create_graph, or call context_read with create: true"
                    .into(),
            ),
            ErrorKind::NotFound if error.message.contains("is not a vertex") => Some(
                "use context_search to find where the text occurs, or pass \"#index\"".into(),
            ),
            ErrorKind::Locked =>
                Some("another process (e.g. a context-cli server) holds this graph".into()),
            _ => return,
        };
        error.hint = hint;
    }

    /// The response JSON as text content, within the output budget.
    fn result(
        &self,
        response: Response,
    ) -> CallToolResult {
        let ok = response.is_ok();
        let text =
            serde_json::to_string(&response).expect("responses serialize");
        // Within budget: the exact `Response` serialization (field order as
        // in context-api, identical to every other transport).
        let fitted = if text.len() <= self.config.max_output {
            budget::Fitted {
                text,
                overflow: false,
            }
        } else {
            let value =
                serde_json::to_value(&response).expect("responses serialize");
            budget::fit(value, self.config.max_output)
        };
        let content = vec![Content::text(fitted.text)];
        if ok && !fitted.overflow {
            CallToolResult::success(content)
        } else {
            CallToolResult::error(content)
        }
    }

    /// `files` inputs: relative paths resolve below `--allow-fs`.
    fn file_input(
        &self,
        path: String,
    ) -> Input {
        let path = match &self.config.allow_fs {
            Some(root) if Path::new(&path).is_relative() =>
                root.join(&path).to_string_lossy().into_owned(),
            _ => path,
        };
        Input::File { path }
    }
}

fn error_response(
    id: Option<u64>,
    kind: ErrorKind,
    message: String,
    hint: Option<String>,
) -> Response {
    Response {
        id,
        outcome: Outcome::Error(ErrorResponse {
            kind,
            message,
            hint,
        }),
    }
}

fn text_inputs(texts: Vec<String>) -> impl Iterator<Item = Input> {
    texts
        .into_iter()
        .map(|text| Input::Text { text, label: None })
}

/// `"#123"` is an index, anything else a text.
fn vertex_ref(vertex: String) -> VertexRef {
    match vertex.strip_prefix('#').and_then(|n| n.parse().ok()) {
        Some(index) => VertexRef::Index(index),
        None => VertexRef::Text(vertex),
    }
}

#[tool_router]
impl ContextServer {
    #[tool(
        description = "List the graphs in the store with their sizes (vertices, inputs, chars). Returns ok.graphs.graphs[].",
        annotations(read_only_hint = true)
    )]
    async fn context_list_graphs(&self) -> Result<CallToolResult, McpError> {
        Ok(self.typed(Command::List).await)
    }

    #[tool(
        description = "Size and shape of one graph: summary (vertex, atom and input counts, chars, generation) plus width and parent-count histograms. Returns ok.stats.",
        annotations(read_only_hint = true)
    )]
    async fn context_graph_info(
        &self,
        Parameters(p): Parameters<GraphParams>,
    ) -> Result<CallToolResult, McpError> {
        Ok(self.typed(Command::Stats { graph: p.graph }).await)
    }

    #[tool(
        description = "Create a graph. With texts, every string is read as one input and the graph is saved (returns ok.read: input ids, vertices, chars); without texts an empty graph is created (returns ok.graph). Fails if the graph exists.",
        annotations(destructive_hint = false, idempotent_hint = false)
    )]
    async fn context_create_graph(
        &self,
        Parameters(p): Parameters<CreateGraphParams>,
    ) -> Result<CallToolResult, McpError> {
        let command = if p.texts.is_empty() {
            Command::New {
                graph: p.graph,
                overwrite: p.overwrite,
            }
        } else {
            Command::Create {
                graph: p.graph,
                inputs: text_inputs(p.texts).collect(),
                split: p.split.unwrap_or_default(),
                overwrite: p.overwrite,
            }
        };
        Ok(self.typed(command).await)
    }

    #[tool(
        description = "Read more text into a graph (the only way to add content); saved immediately. Pass texts and/or files (files only if the server runs with --allow-fs; relative paths resolve below it). Returns ok.read: per input its id, label, vertex and char count.",
        annotations(destructive_hint = false, idempotent_hint = false)
    )]
    async fn context_read(
        &self,
        Parameters(p): Parameters<ReadParams>,
    ) -> Result<CallToolResult, McpError> {
        let mut inputs: Vec<Input> = text_inputs(p.texts).collect();
        inputs.extend(p.files.into_iter().map(|path| self.file_input(path)));
        if inputs.is_empty() {
            return Ok(self.result(error_response(
                None,
                ErrorKind::InvalidInput,
                "nothing to read: pass texts and/or files".into(),
                None,
            )));
        }
        let command = Command::Read {
            graph: p.graph,
            inputs,
            split: p.split.unwrap_or_default(),
            create: p.create,
            save: SavePolicy::Auto,
            steps: false,
            dry_run: false,
        };
        Ok(self.typed(command).await)
    }

    #[tool(
        description = "Search a graph for an exact string. view: summary (default; found as a vertex, inside a host vertex, partially, or not), occurrences (every position in the read inputs with line, column and snippet: use for concordance/grep), contexts (distinct larger vertices containing it, narrowest first, with snippet: use to explore surroundings), parents (direct parent vertices), path (raw search path). Lists honour limit (default 20); width = snippet chars per side (default 40). Returns ok.search.",
        annotations(read_only_hint = true)
    )]
    async fn context_search(
        &self,
        Parameters(p): Parameters<SearchParams>,
    ) -> Result<CallToolResult, McpError> {
        let command = Command::Search {
            graph: p.graph,
            query: p.query,
            view: p.view.unwrap_or(SearchView::Summary),
            limits: Limits {
                limit: Some(p.limit.unwrap_or(SEARCH_LIMIT)),
                context_width: Some(p.width.unwrap_or(SEARCH_WIDTH)),
                label_width: p.label_width,
                ..Limits::default()
            },
        };
        Ok(self.typed(command).await)
    }

    #[tool(
        description = "Inspect one vertex: label, width, child composition and parents (with offsets), and the inputs it is the whole of. vertex: \"#123\" for an index, else its exact text (must be a vertex; use context_search first if unsure). Returns ok.vertex.",
        annotations(read_only_hint = true)
    )]
    async fn context_vertex(
        &self,
        Parameters(p): Parameters<VertexParams>,
    ) -> Result<CallToolResult, McpError> {
        let command = Command::Vertex {
            graph: p.graph,
            vertex: vertex_ref(p.vertex),
            limits: Limits {
                limit: p.limit,
                label_width: p.label_width,
                ..Limits::default()
            },
        };
        Ok(self.typed(command).await)
    }

    #[tool(
        description = "Draw a small graph, or the subgraph below root down to depth. format: mermaid (default), dot, ascii, ascii_dag, json. Graphs over the server's vertex cap (default 200) are refused with kind too_large: then pass root and a small depth. Returns ok.render.text (or ok.render.graph for json).",
        annotations(read_only_hint = true)
    )]
    async fn context_render(
        &self,
        Parameters(p): Parameters<RenderParams>,
    ) -> Result<CallToolResult, McpError> {
        let command = Command::Render {
            graph: p.graph,
            format: p.format.unwrap_or(RenderFormat::Mermaid),
            root: p.root.map(vertex_ref),
            depth: p.depth,
            gate: Gate {
                max_vertices: self.config.render_cap,
                max_label_chars: None,
                force: false,
            },
            limits: Limits::default(),
        };
        Ok(self.typed(command).await)
    }

    #[tool(
        description = "Escape hatch: run a raw context-api Request ({\"command\": \"search\", \"graph\": ..., ...}) and get the raw Response JSON. Prefer the typed tools.",
        annotations(destructive_hint = true, open_world_hint = false)
    )]
    async fn context_execute(
        &self,
        Parameters(request): Parameters<Request>,
    ) -> Result<CallToolResult, McpError> {
        if let Err(mut response) = self.check(&request.command) {
            response.id = request.id;
            return Ok(self.result(*response));
        }
        let response = self.run(request).await;
        Ok(self.result(response))
    }

    #[tool(
        description = "Delete a graph file permanently. Returns ok.deleted.",
        annotations(destructive_hint = true)
    )]
    async fn context_delete_graph(
        &self,
        Parameters(p): Parameters<GraphParams>,
    ) -> Result<CallToolResult, McpError> {
        Ok(self.typed(Command::Delete { graph: p.graph }).await)
    }

    #[tool(
        description = "Short usage guide: workflow and which search view to use.",
        annotations(read_only_hint = true)
    )]
    async fn context_help(&self) -> Result<CallToolResult, McpError> {
        Ok(CallToolResult::success(vec![Content::text(HELP)]))
    }
}

#[tool_handler]
impl ServerHandler for ContextServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            capabilities: ServerCapabilities::builder().enable_tools().build(),
            server_info: Implementation {
                name: "context-mcp".into(),
                title: Some("context engine".into()),
                version: env!("CARGO_PKG_VERSION").into(),
                icons: None,
                website_url: None,
            },
            instructions: Some(INSTRUCTIONS.into()),
            ..ServerInfo::default()
        }
    }
}
