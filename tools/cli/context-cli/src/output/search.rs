//! Text renderer of search results (one block per view).

use std::fmt::Write;

use context_api::types::{
    PathStep,
    SearchOutcome,
    SearchResult,
    TokenInfo,
};

use super::{
    graph::offsets,
    one_line,
    snippet,
    token,
};

pub(super) fn search(result: &SearchResult) -> String {
    let mut out = outcome(result);
    if let Some(path) = &result.path {
        let _ = write!(
            out,
            "\nhost {}  coverage {:?}  span [{}, {})  query [{}, {})",
            token(&path.root),
            path.coverage,
            path.span.start,
            path.span.end,
            path.query_start,
            path.checkpoint_position
        );
        let _ = write!(out, "\nstart: {}", steps(&path.root, &path.start_path));
        let _ = write!(out, "\nend:   {}", steps(&path.root, &path.end_path));
    }
    if let Some(contexts) = &result.contexts {
        let _ = write!(
            out,
            "\n{} (narrowest first)",
            count(
                contexts.total,
                contexts.items.len(),
                contexts.truncated,
                "context"
            )
        );
        for item in &contexts.items {
            let _ = write!(
                out,
                "\n  {:<7} width {:<5} @{:<6} {}",
                format!("#{}", item.ancestor.index),
                item.ancestor.width,
                offsets(&item.offsets),
                snippet(&item.snippet)
            );
        }
    }
    if let Some(occurrences) = &result.occurrences {
        let _ = write!(
            out,
            "\n{} in {} input{}",
            count(
                occurrences.total,
                occurrences.items.len(),
                occurrences.truncated,
                "occurrence"
            ),
            occurrences.inputs_with_matches,
            if occurrences.inputs_with_matches == 1 {
                ""
            } else {
                "s"
            }
        );
        let positions: Vec<_> = occurrences
            .items
            .iter()
            .map(|item| source_position(&one_line(&item.label), item.line, item.column))
            .collect();
        let width = positions
            .iter()
            .map(|p| p.chars().count())
            .max()
            .unwrap_or(0);
        for (item, position) in occurrences.items.iter().zip(positions) {
            let _ = write!(
                out,
                "\n  [{}] {position:<width$}  {}",
                item.input_id,
                snippet(&item.snippet)
            );
        }
    }
    if let Some(parents) = &result.parents {
        let _ = write!(
            out,
            "\n{} parent{}",
            parents.len(),
            if parents.len() == 1 { "" } else { "s" }
        );
        for parent in parents {
            let _ = write!(
                out,
                "\n  {}  width {}  @{}",
                token(&parent.parent),
                parent.parent.width,
                offsets(&parent.offsets)
            );
        }
    }
    out
}

fn outcome(result: &SearchResult) -> String {
    match &result.outcome {
        SearchOutcome::NotFound { unknown_chars }
            if unknown_chars.is_empty() =>
            format!("not found: {:?}", result.query),
        SearchOutcome::NotFound { unknown_chars } => {
            let chars: Vec<_> =
                unknown_chars.iter().map(|c| format!("{c:?}")).collect();
            format!(
                "not found: {:?} contains unknown chars {}",
                result.query,
                chars.join(", ")
            )
        },
        SearchOutcome::Vertex { token: t } =>
            format!("found: vertex {} (width {})", token(t), t.width),
        SearchOutcome::InHost { host, span } => format!(
            "found in host {} at [{}, {})",
            token(host),
            span.start,
            span.end
        ),
        SearchOutcome::Partial {
            host,
            matched_chars,
            query_chars,
            ..
        } => {
            let matched: String =
                result.query.chars().take(*matched_chars).collect();
            format!(
                "partial: matched {:?} ({matched_chars}/{query_chars} chars) in host {}; full query not in graph",
                matched,
                token(host)
            )
        },
    }
}

/// `3 occurrences`, `at least 3 occurrences`, `3 of 10 occurrences`.
fn count(
    total: Option<usize>,
    shown: usize,
    truncated: bool,
    noun: &str,
) -> String {
    let plural = |n: usize| {
        if n == 1 {
            noun.to_string()
        } else {
            format!("{noun}s")
        }
    };
    match total {
        Some(total) if truncated || shown < total =>
            format!("{total} {} (showing {shown})", plural(total)),
        Some(total) => format!("{total} {}", plural(total)),
        None => format!("at least {shown} {} (work budget hit)", plural(shown)),
    }
}

/// `#812 ─@0→ #95 "brown" ─@0→ #7 "b"`.
fn steps(
    root: &TokenInfo,
    path: &[PathStep],
) -> String {
    let mut out = format!("#{}", root.index);
    if path.is_empty() {
        out = token(root);
    }
    for step in path {
        let _ = write!(out, " ─@{}→ {}", step.offset, token(&step.child));
    }
    out
}

/// `label:line:column` in source coordinates: inputs split from a source
/// are labelled `source:first_line`, and the occurrence line is relative to
/// the input.
fn source_position(
    label: &str,
    line: usize,
    column: usize,
) -> String {
    match label.rsplit_once(':') {
        Some((source, first)) if !source.is_empty() => match first.parse::<usize>() {
            Ok(first) if first > 0 => format!("{source}:{}:{column}", first + line - 1),
            _ => format!("{label}:{line}:{column}"),
        },
        _ => format!("{label}:{line}:{column}"),
    }
}

#[cfg(test)]
mod source_position_tests {
    use super::source_position;

    #[test]
    fn split_labels_map_to_source_lines() {
        assert_eq!(source_position("notes.md:12", 3, 5), "notes.md:14:5");
        assert_eq!(source_position("<stdin>:1", 1, 1), "<stdin>:1:1");
        assert_eq!(source_position("<text>", 2, 4), "<text>:2:4");
    }
}
