//! Output: `--format json` prints the `Response` envelope unchanged; text
//! renderers per result type (plan 02 §2.3).

mod graph;
mod search;

use std::io::IsTerminal;

use context_api::{
    CommandResult,
    ErrorResponse,
    Response,
    types::{
        Snippet,
        TokenInfo,
    },
};

/// Print the response JSON on stdout (pretty on a terminal).
pub fn print_json(response: &Response) {
    let json = if std::io::stdout().is_terminal() {
        serde_json::to_string_pretty(response)
    } else {
        serde_json::to_string(response)
    };
    println!("{}", json.expect("responses serialize"));
}

/// Print an error and its hint on stderr.
pub fn print_error(error: &ErrorResponse) {
    eprintln!("error: {}", error.message);
    if let Some(hint) = &error.hint {
        eprintln!("hint: {hint}");
    }
}

/// The text rendering of a result of the CLI command `command`.
pub fn text(
    command: &str,
    result: &CommandResult,
) -> String {
    match result {
        CommandResult::Graph(summary) => graph::summary(command, summary),
        CommandResult::Graphs { graphs } => graph::list(graphs),
        CommandResult::Deleted { graph } => format!("deleted {graph}"),
        CommandResult::Read(report) => graph::read(command, report),
        CommandResult::Search(result) => search::search(result),
        CommandResult::Vertex(info) => graph::vertex(info),
        CommandResult::Inputs(list) => graph::inputs(list),
        CommandResult::Stats(stats) => graph::stats(stats),
        CommandResult::Validation(report) => graph::validation(report),
        CommandResult::Render(render) => match (&render.text, &render.graph) {
            (Some(text), _) => text.trim_end().to_string(),
            (None, Some(graph)) => serde_json::to_string_pretty(graph)
                .expect("render graphs serialize"),
            (None, None) => String::new(),
        },
        CommandResult::Export(export) => export.data.trim_end().to_string(),
        other =>
            serde_json::to_string_pretty(other).expect("results serialize"),
    }
}

/// `#index "label"`.
pub(crate) fn token(token: &TokenInfo) -> String {
    format!("#{} {}", token.index, label(token))
}

/// The quoted, escaped label; `…` marks truncation.
pub(crate) fn label(token: &TokenInfo) -> String {
    format!(
        "\"{}{}\"",
        token.label.escape_debug(),
        if token.label_truncated { "…" } else { "" }
    )
}

/// `…left [match] right…` on one line.
pub(crate) fn snippet(snippet: &Snippet) -> String {
    format!(
        "{}{}[{}]{}{}",
        if snippet.left_truncated { "…" } else { "" },
        one_line(&snippet.left),
        one_line(&snippet.matched),
        one_line(&snippet.right),
        if snippet.right_truncated { "…" } else { "" },
    )
}

/// Control characters escaped so the text stays on one line.
pub(crate) fn one_line(text: &str) -> String {
    text.chars()
        .flat_map(|c| -> Box<dyn Iterator<Item = char>> {
            if c.is_control() {
                Box::new(c.escape_default())
            } else {
                Box::new(std::iter::once(c))
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snippet_marks_match_and_truncation() {
        let s = Snippet {
            left: "the quick ".into(),
            matched: "brown fox".into(),
            right: " jumps\nover".into(),
            left_truncated: true,
            right_truncated: true,
        };
        assert_eq!(snippet(&s), "…the quick [brown fox] jumps\\nover…");
    }
}
