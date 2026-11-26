//! Read inputs into a graph (the only mutation).

use std::{
    fs,
    path::{
        Path,
        PathBuf,
    },
    time::{
        Duration,
        Instant,
    },
};

use context_read::{
    Step,
    StepKind as ReadStepKind,
};
use context_trace::positioned::{
    PositionedGraph,
    PositionedGraphRef,
};

use super::text;
use crate::{
    error::ApiError,
    event::Event,
    store::file::{
        GraphMeta,
        now,
    },
    types::{
        Input,
        InputRecord,
        ReadInputReport,
        Split,
        StepInfo,
        StepKind,
    },
};

/// One reader input after resolving sources and splitting.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Item {
    pub label: String,
    pub text: String,
}

/// Resolve every input to text (files checked against `allow_fs`) and split
/// it, before anything is read: an invalid input changes nothing.
pub(crate) fn resolve_inputs(
    inputs: Vec<Input>,
    split: Split,
    allow_fs: Option<&Path>,
) -> Result<Vec<Item>, ApiError> {
    let mut items = Vec::new();
    for input in inputs {
        let (label, text) = match input {
            Input::Text { text, label } => (label.unwrap_or_else(|| "<text>".into()), text),
            Input::File { path } => {
                let full = check_file(&path, allow_fs)?;
                let bytes = fs::read(&full)?;
                let text = String::from_utf8(bytes).map_err(|_| {
                    ApiError::InvalidInput(format!("'{path}' is not valid UTF-8"))
                })?;
                (path, text)
            },
        };
        split_into(&label, &text, split, &mut items);
    }
    Ok(items)
}

fn check_file(
    path: &str,
    allow_fs: Option<&Path>,
) -> Result<PathBuf, ApiError> {
    let Some(root) = allow_fs else {
        return Err(ApiError::FileAccess(format!(
            "file inputs are disabled; '{path}' was not read"
        )));
    };
    let root = root.canonicalize()?;
    let full = Path::new(path)
        .canonicalize()
        .map_err(|e| ApiError::InvalidInput(format!("cannot read '{path}': {e}")))?;
    if !full.starts_with(&root) {
        return Err(ApiError::FileAccess(format!(
            "'{path}' is outside the allowed directory {}",
            root.display()
        )));
    }
    Ok(full)
}

/// Split a source; empty items are kept only for `Split::None` (reported as
/// skipped by the reader).
fn split_into(
    label: &str,
    text: &str,
    split: Split,
    items: &mut Vec<Item>,
) {
    let item = |label: String, text: &str| Item {
        label,
        text: text.to_string(),
    };
    match split {
        Split::None => items.push(item(label.to_string(), text)),
        Split::Lines => items.extend(
            text.lines()
                .enumerate()
                .filter(|(_, line)| !line.trim().is_empty())
                .map(|(n, line)| item(format!("{label}:{}", n + 1), line)),
        ),
        Split::Paragraphs => {
            let mut start: Option<usize> = None;
            let mut lines: Vec<&str> = Vec::new();
            for (n, line) in text.lines().chain(std::iter::once("")).enumerate() {
                if line.trim().is_empty() {
                    if let Some(first) = start.take() {
                        items.push(item(format!("{label}:{}", first + 1), &lines.join("\n")));
                        lines.clear();
                    }
                } else {
                    start.get_or_insert(n);
                    lines.push(line);
                }
            }
        },
    }
}

/// Emits throttled progress events of one input.
struct Progress<'a> {
    emit: Option<&'a dyn Fn(Event)>,
    graph: &'a str,
    request: Option<u64>,
    input: usize,
    total: usize,
    last_chars: usize,
    last_time: Instant,
}

impl Progress<'_> {
    fn step(
        &mut self,
        chars: usize,
    ) {
        let Some(emit) = self.emit else { return };
        let enough = chars - self.last_chars >= (self.total / 100).max(1);
        let late = self.last_time.elapsed() >= Duration::from_millis(100);
        if chars == self.total || (enough && late) {
            self.last_chars = chars;
            self.last_time = Instant::now();
            emit(Event::ReadProgress {
                graph: self.graph.to_string(),
                request: self.request,
                input: self.input,
                chars,
                total: self.total,
            });
        }
    }
}

fn step_info(step: &Step) -> StepInfo {
    StepInfo {
        kind: match step.kind {
            ReadStepKind::Unknown => StepKind::Unknown,
            ReadStepKind::Forward(width) => StepKind::Forward { width },
            ReadStepKind::Expansion { postfix, expansion } =>
                StepKind::Expansion { postfix, expansion },
        },
        root_width: step.root,
        dissolved: step.dissolved,
        discarded: step.discarded,
        stabilized: step.stabilized,
    }
}

pub(crate) struct ReadOutcome {
    pub inputs: Vec<ReadInputReport>,
    pub skipped: Vec<String>,
}

/// Read `items` into `graph`, appending to the input log in `meta`.
#[allow(clippy::too_many_arguments)]
pub(crate) fn read_items(
    graph: &PositionedGraphRef,
    meta: &mut GraphMeta,
    items: Vec<Item>,
    steps: bool,
    graph_name: &str,
    request: Option<u64>,
    emit: Option<&dyn Fn(Event)>,
) -> ReadOutcome {
    let mut inputs = Vec::new();
    let mut skipped = Vec::new();
    for (input, Item { label, text }) in items.into_iter().enumerate() {
        let total = text.chars().count();
        if total == 0 {
            skipped.push(label);
            continue;
        }
        let mut progress = Progress {
            emit,
            graph: graph_name,
            request,
            input,
            total,
            last_chars: 0,
            last_time: Instant::now(),
        };
        let mut recorded = Vec::new();
        let token = context_read::read_observed(graph, text.chars(), |step| {
            progress.step(step.root);
            if steps {
                recorded.push(step_info(step));
            }
        })
        .expect("a non-empty input reads to a token");
        let id = meta.inputs.len() as u32;
        meta.inputs.push(InputRecord {
            id,
            label: label.clone(),
            token: token.index.0,
            width: token.width.0,
            read_at: now(),
        });
        let guard = graph.read();
        inputs.push(ReadInputReport {
            id,
            label,
            token: text::token_info(&guard, token.index, text::DEFAULT_LABEL_WIDTH),
            chars: total,
            steps: steps.then_some(recorded),
        });
    }
    ReadOutcome { inputs, skipped }
}

/// A detached copy for dry runs.
pub(crate) fn copy(graph: &PositionedGraphRef) -> PositionedGraphRef {
    PositionedGraphRef::new(PositionedGraph::clone(&graph.read()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn labels_and_texts(items: &[Item]) -> Vec<(&str, &str)> {
        items.iter().map(|i| (i.label.as_str(), i.text.as_str())).collect()
    }

    #[test]
    fn splitting() {
        let text = "one\n\n  \ntwo\nthree\n\n\nfour";
        let mut items = Vec::new();
        split_into("f", text, Split::Lines, &mut items);
        assert_eq!(
            labels_and_texts(&items),
            [("f:1", "one"), ("f:4", "two"), ("f:5", "three"), ("f:8", "four")]
        );
        items.clear();
        split_into("f", text, Split::Paragraphs, &mut items);
        assert_eq!(
            labels_and_texts(&items),
            [("f:1", "one"), ("f:4", "two\nthree"), ("f:8", "four")]
        );
        items.clear();
        split_into("f", "", Split::None, &mut items);
        assert_eq!(labels_and_texts(&items), [("f", "")]);
    }

    #[test]
    fn file_access() {
        let dir = tempfile::tempdir().unwrap();
        let inside = dir.path().join("a.txt");
        fs::write(&inside, "abc").unwrap();
        let file = |path: &Path| vec![Input::File { path: path.display().to_string() }];
        assert!(matches!(
            resolve_inputs(file(&inside), Split::None, None),
            Err(ApiError::FileAccess(_))
        ));
        let items = resolve_inputs(file(&inside), Split::None, Some(dir.path())).unwrap();
        assert_eq!(items[0].text, "abc");
        let other = tempfile::tempdir().unwrap();
        let outside = other.path().join("b.txt");
        fs::write(&outside, "x").unwrap();
        assert!(matches!(
            resolve_inputs(file(&outside), Split::None, Some(dir.path())),
            Err(ApiError::FileAccess(_))
        ));
        let binary = dir.path().join("bin");
        fs::write(&binary, [0xff, 0xfe]).unwrap();
        assert!(matches!(
            resolve_inputs(file(&binary), Split::None, Some(dir.path())),
            Err(ApiError::InvalidInput(_))
        ));
    }
}
