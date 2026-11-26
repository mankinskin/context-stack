//! Helpers over the public contract only: `Session::execute` with
//! `Request`/`Command`, results as `CommandResult`.

use context_api::{
    Command,
    CommandResult,
    ErrorResponse,
    Request,
    Session,
    SessionConfig,
    types::*,
};
use context_trace::positioned::interchange::PositionedGraphSnapshot;
use tempfile::TempDir;

/// A session on a fresh temporary store.
pub struct Env {
    pub dir: TempDir,
    pub session: Session,
}

impl Env {
    pub fn new() -> Self {
        Self::with(|_| {})
    }

    pub fn with(configure: impl FnOnce(&mut SessionConfig)) -> Self {
        let dir = TempDir::new().unwrap();
        let mut config = SessionConfig::new(dir.path());
        configure(&mut config);
        Self {
            session: Session::new(config),
            dir,
        }
    }

    /// Another session on the same store.
    pub fn session(
        &self,
        configure: impl FnOnce(&mut SessionConfig),
    ) -> Session {
        let mut config = SessionConfig::new(self.dir.path());
        configure(&mut config);
        Session::new(config)
    }
}

pub fn exec(
    session: &Session,
    command: Command,
) -> CommandResult {
    let name = command.name();
    match session.execute(Request::new(command)).into_result() {
        Ok(result) => result,
        Err(error) => panic!("`{name}` failed: {error:?}"),
    }
}

pub fn exec_err(
    session: &Session,
    command: Command,
) -> ErrorResponse {
    let name = command.name();
    match session.execute(Request::new(command)).into_result() {
        Ok(result) => panic!("`{name}` unexpectedly succeeded: {result:?}"),
        Err(error) => error,
    }
}

/// Limits large enough to never cut a result of these tests.
pub fn unlimited() -> Limits {
    Limits {
        limit: Some(1_000_000),
        context_width: None,
        label_width: Some(1_000_000),
        max_depth: None,
        max_visited: Some(usize::MAX / 2),
    }
}

pub fn text_inputs(texts: &[String]) -> Vec<Input> {
    texts
        .iter()
        .map(|text| Input::Text {
            text: text.clone(),
            label: None,
        })
        .collect()
}

pub fn strings(texts: &[&str]) -> Vec<String> {
    texts.iter().map(ToString::to_string).collect()
}

pub fn create(
    session: &Session,
    graph: &str,
    texts: &[String],
) {
    exec(
        session,
        Command::Create {
            graph: graph.into(),
            inputs: text_inputs(texts),
            split: Split::None,
            overwrite: false,
        },
    );
}

pub fn read(
    session: &Session,
    graph: &str,
    inputs: Vec<Input>,
    create: bool,
) -> ReadReport {
    match exec(
        session,
        Command::Read {
            graph: graph.into(),
            inputs,
            split: Split::None,
            create,
            save: SavePolicy::Auto,
            steps: false,
            dry_run: false,
        },
    ) {
        CommandResult::Read(report) => report,
        other => panic!("read: unexpected result {other:?}"),
    }
}

pub fn info(
    session: &Session,
    graph: &str,
) -> GraphSummary {
    match exec(session, Command::Info { graph: graph.into() }) {
        CommandResult::Graph(summary) => summary,
        other => panic!("info: unexpected result {other:?}"),
    }
}

pub fn list(session: &Session) -> Vec<GraphSummary> {
    match exec(session, Command::List) {
        CommandResult::Graphs { graphs } => graphs,
        other => panic!("list: unexpected result {other:?}"),
    }
}

/// The input log, ordered by id.
pub fn inputs(
    session: &Session,
    graph: &str,
) -> Vec<InputRecord> {
    match exec(
        session,
        Command::Inputs {
            graph: graph.into(),
            limits: unlimited(),
        },
    ) {
        CommandResult::Inputs(list) => {
            assert_eq!(list.total, list.items.len(), "inputs: total");
            let mut ids: Vec<u32> = list.items.iter().map(|i| i.id).collect();
            ids.sort_unstable();
            ids.dedup();
            assert_eq!(
                ids,
                list.items.iter().map(|i| i.id).collect::<Vec<_>>(),
                "inputs: ids must be distinct and ascending"
            );
            list.items
        },
        other => panic!("inputs: unexpected result {other:?}"),
    }
}

pub fn search(
    session: &Session,
    graph: &str,
    query: &str,
    view: SearchView,
    limits: Limits,
) -> SearchResult {
    match exec(
        session,
        Command::Search {
            graph: graph.into(),
            query: query.into(),
            view,
            limits,
        },
    ) {
        CommandResult::Search(result) => result,
        other => panic!("search: unexpected result {other:?}"),
    }
}

pub fn vertex(
    session: &Session,
    graph: &str,
    vertex: VertexRef,
) -> VertexInfo {
    match exec(
        session,
        Command::Vertex {
            graph: graph.into(),
            vertex,
            limits: unlimited(),
        },
    ) {
        CommandResult::Vertex(info) => info,
        other => panic!("vertex: unexpected result {other:?}"),
    }
}

/// The full text of a vertex.
pub fn label(
    session: &Session,
    graph: &str,
    index: usize,
) -> String {
    let info = vertex(session, graph, VertexRef::Index(index));
    assert!(!info.token.label_truncated, "label of {index} truncated");
    assert_eq!(info.token.label.chars().count(), info.token.width);
    info.token.label
}

pub fn render(
    session: &Session,
    graph: &str,
    format: RenderFormat,
    root: Option<VertexRef>,
    gate: Gate,
) -> Result<RenderOutput, ErrorResponse> {
    let command = Command::Render {
        graph: graph.into(),
        format,
        root,
        depth: None,
        gate,
        limits: unlimited(),
    };
    session
        .execute(Request::new(command))
        .into_result()
        .map(|result| match result {
            CommandResult::Render(output) => output,
            other => panic!("render: unexpected result {other:?}"),
        })
}

pub fn force() -> Gate {
    Gate {
        force: true,
        ..Gate::default()
    }
}

/// The graph as a v3 interchange document (forced past the gate).
pub fn export(
    session: &Session,
    graph: &str,
) -> PositionedGraphSnapshot {
    match exec(
        session,
        Command::Export {
            graph: graph.into(),
            format: ExportFormat::InterchangeV3,
            gate: force(),
        },
    ) {
        CommandResult::Export(output) => {
            assert_eq!(output.format, ExportFormat::InterchangeV3);
            serde_json::from_str(&output.data)
                .unwrap_or_else(|error| panic!("export: invalid v3 document: {error}"))
        },
        other => panic!("export: unexpected result {other:?}"),
    }
}

/// `text[start..start + len]` in chars.
pub fn char_slice(
    text: &[char],
    start: usize,
    len: usize,
) -> String {
    text[start..start + len].iter().collect()
}

/// Every (overlapping) char offset of `query` in `text`.
pub fn brute_offsets(
    text: &str,
    query: &str,
) -> Vec<usize> {
    let text: Vec<char> = text.chars().collect();
    let query: Vec<char> = query.chars().collect();
    if query.is_empty() || query.len() > text.len() {
        return Vec::new();
    }
    (0..=text.len() - query.len())
        .filter(|&i| text[i..i + query.len()] == query[..])
        .collect()
}

/// The snippet of the match `text[offset..offset + len]` with `context`
/// chars per side.
pub fn snippet(
    text: &[char],
    offset: usize,
    len: usize,
    context: usize,
) -> Snippet {
    let left_start = offset.saturating_sub(context);
    let right_end = (offset + len + context).min(text.len());
    Snippet {
        left: char_slice(text, left_start, offset - left_start),
        matched: char_slice(text, offset, len),
        right: char_slice(text, offset + len, right_end - offset - len),
        left_truncated: left_start > 0,
        right_truncated: right_end < text.len(),
    }
}

/// Every distinct nonempty substring of the texts.
pub fn substrings(texts: &[String]) -> std::collections::BTreeSet<String> {
    let mut all = std::collections::BTreeSet::new();
    for text in texts {
        let chars: Vec<char> = text.chars().collect();
        for start in 0..chars.len() {
            for end in start + 1..=chars.len() {
                all.insert(chars[start..end].iter().collect());
            }
        }
    }
    all
}

/// Collects failures of a loop and reports them together.
#[derive(Default)]
pub struct Failures(Vec<String>);

impl Failures {
    pub fn check(
        &mut self,
        ok: bool,
        message: impl FnOnce() -> String,
    ) {
        if !ok {
            self.0.push(message());
        }
    }

    pub fn assert_none(
        self,
        what: &str,
    ) {
        assert!(
            self.0.is_empty(),
            "{what}: {} failures, first 30:\n{}",
            self.0.len(),
            self.0.iter().take(30).cloned().collect::<Vec<_>>().join("\n")
        );
    }
}
