//! Search and its views (plan 01 §1.5).
//!
//! The outcome comes from `find_ancestor`. The `contexts` and `occurrences`
//! views share one enumeration: a breadth-first climb over parent
//! occurrences from the match, deduplicated by `(ancestor, absolute
//! offset)` (overlapping children reach the same position through several
//! paths; path counts are never summed).

use std::collections::{
    BTreeSet,
    HashMap,
    HashSet,
    VecDeque,
};

use context_search::{
    Find,
    Response,
    SearchError,
    state::matched::PathCoverage,
};
use context_trace::{
    VertexIndex,
    positioned::{
        GraphRead,
        OccurrenceLocation,
        PositionedGraph,
        VertexRead,
    },
};

use super::{
    View,
    info,
    text::{
        self,
        DEFAULT_CONTEXT_WIDTH,
        DEFAULT_LIMIT,
        DEFAULT_MAX_VISITED,
        label_width,
    },
};
use crate::{
    error::ApiError,
    types::{
        ContextItem,
        ContextList,
        Coverage,
        Limits,
        OccurrenceItem,
        OccurrenceList,
        ParentInfo,
        PathInfo,
        PathStep,
        SearchOutcome,
        SearchResult,
        SearchView,
        Span,
    },
};

/// `max_visited_cap`: the session's upper bound for `Limits::max_visited`.
pub(crate) fn search(
    view: &View<'_>,
    query: &str,
    search_view: SearchView,
    limits: &Limits,
    max_visited_cap: Option<usize>,
) -> Result<SearchResult, ApiError> {
    let graph = view.graph;
    let label_width = label_width(limits);
    let found = find(graph, query)?;
    let outcome = found.outcome(graph, label_width);
    let mut result = SearchResult {
        graph: view.name.to_string(),
        query: query.to_string(),
        outcome,
        path: None,
        contexts: None,
        occurrences: None,
        parents: None,
    };
    let limit = limits.limit.unwrap_or(DEFAULT_LIMIT);
    let context_width = limits.context_width.unwrap_or(DEFAULT_CONTEXT_WIDTH);
    let max_visited = {
        let requested = limits.max_visited.unwrap_or(DEFAULT_MAX_VISITED);
        max_visited_cap.map_or(requested, |cap| requested.min(cap))
    };
    let start = found.start();
    let reach = || match start {
        Some(start) => enumerate(graph, start, limits.max_depth, max_visited),
        None => Reach::default(),
    };
    match search_view {
        SearchView::Summary => {},
        SearchView::Path => {
            result.path = found
                .response()
                .map(|response| path_info(graph, response, label_width));
        },
        SearchView::Contexts => {
            result.contexts = Some(contexts(graph, start, &reach(), limit, context_width, label_width));
        },
        SearchView::Occurrences => {
            result.occurrences = Some(occurrences(view, start, &reach(), limit, context_width));
        },
        SearchView::Parents => {
            result.parents = Some(match &found {
                Found::Vertex { token, .. } =>
                    info::parents(view, *token, label_width).into_iter().take(limit).collect(),
                Found::InHost { response } if limit > 0 => {
                    let span = response.span();
                    vec![ParentInfo {
                        parent: text::token_info(graph, response.root_token().index, label_width),
                        offsets: vec![span.start.into()],
                    }]
                },
                _ => Vec::new(),
            });
        },
    }
    Ok(result)
}

enum Found {
    NotFound(Vec<char>),
    /// `response` is `None` if the query was a single token the search
    /// refused to search beyond.
    Vertex {
        token: VertexIndex,
        response: Option<Response>,
    },
    InHost {
        response: Response,
    },
    Partial {
        response: Response,
        query_chars: usize,
    },
}

fn find(
    graph: &PositionedGraph,
    query: &str,
) -> Result<Found, ApiError> {
    let mut unknown = Vec::new();
    for c in query.chars() {
        if graph.atom(&c).is_none() && !unknown.contains(&c) {
            unknown.push(c);
        }
    }
    if query.is_empty() || !unknown.is_empty() {
        return Ok(Found::NotFound(unknown));
    }
    let response = match graph.find_ancestor(query.chars()) {
        Ok(response) => response,
        Err(SearchError::SingleIndex(token)) => {
            return Ok(Found::Vertex {
                token: token.index,
                response: None,
            });
        },
        Err(SearchError::EmptyPatterns) => return Ok(Found::NotFound(Vec::new())),
        Err(error) => return Err(ApiError::Internal(format!("search failed: {error}"))),
    };
    Ok(if response.query_exhausted() && response.is_entire_root() {
        Found::Vertex {
            token: response.root_token().index,
            response: Some(response),
        }
    } else if response.query_exhausted() {
        Found::InHost { response }
    } else {
        Found::Partial {
            response,
            query_chars: query.chars().count(),
        }
    })
}

/// Where the enumeration starts: the match in `vertex` at `offset`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Start {
    vertex: VertexIndex,
    offset: usize,
    len: usize,
    /// The start vertex is the query itself, not a context of it.
    is_query: bool,
}

impl Found {
    fn response(&self) -> Option<&Response> {
        match self {
            Self::NotFound(_) => None,
            Self::Vertex { response, .. } => response.as_ref(),
            Self::InHost { response } | Self::Partial { response, .. } => Some(response),
        }
    }

    fn start(&self) -> Option<Start> {
        match self {
            Self::Vertex { token, .. } => Some(Start {
                vertex: *token,
                offset: 0,
                len: 0,
                is_query: true,
            }),
            Self::InHost { response } => {
                let span = response.span();
                let (start, end): (usize, usize) = (span.start.into(), span.end.into());
                Some(Start {
                    vertex: response.root_token().index,
                    offset: start,
                    len: end - start,
                    is_query: false,
                })
            },
            Self::NotFound(_) | Self::Partial { .. } => None,
        }
    }

    fn outcome(
        &self,
        graph: &PositionedGraph,
        label_width: usize,
    ) -> SearchOutcome {
        let span_of = |response: &Response| {
            let span = response.span();
            Span {
                start: span.start.into(),
                end: span.end.into(),
            }
        };
        let host_of =
            |response: &Response| text::token_info(graph, response.root_token().index, label_width);
        match self {
            Self::NotFound(unknown_chars) => SearchOutcome::NotFound {
                unknown_chars: unknown_chars.clone(),
            },
            Self::Vertex { token, .. } => SearchOutcome::Vertex {
                token: text::token_info(graph, *token, label_width),
            },
            Self::InHost { response } => SearchOutcome::InHost {
                host: host_of(response),
                span: span_of(response),
            },
            Self::Partial {
                response,
                query_chars,
            } => {
                let end: usize = response.checkpoint_position().into();
                let start: usize = response.query_start().into();
                SearchOutcome::Partial {
                    host: host_of(response),
                    span: span_of(response),
                    matched_chars: end - start,
                    query_chars: *query_chars,
                }
            },
        }
    }
}

/// Vertices reached from the start with the absolute offsets of the match
/// in each (the start vertex included).
#[derive(Debug, Default)]
struct Reach {
    /// Vertex → (width, offsets).
    ancestors: HashMap<VertexIndex, (usize, BTreeSet<usize>)>,
    /// The work budget stopped the climb: counts are incomplete.
    budget_hit: bool,
    /// `max_depth` cut existing parents off.
    depth_cut: bool,
}

impl Reach {
    fn truncated(&self) -> bool {
        self.budget_hit || self.depth_cut
    }
}

/// Breadth-first climb over parent occurrences. `max_visited` counts the
/// visited `(vertex, offset)` pairs, the start included.
fn enumerate(
    graph: &PositionedGraph,
    start: Start,
    max_depth: Option<usize>,
    max_visited: usize,
) -> Reach {
    let mut reach = Reach::default();
    let mut seen = HashSet::from([(start.vertex, start.offset)]);
    reach
        .ancestors
        .entry(start.vertex)
        .or_insert_with(|| (text::width(graph, start.vertex), BTreeSet::new()))
        .1
        .insert(start.offset);
    let mut queue = VecDeque::from([(start.vertex, start.offset, 0usize)]);
    'climb: while let Some((index, offset, depth)) = queue.pop_front() {
        let vertex = graph.vertex(index).expect("vertex of this graph");
        if max_depth.is_some_and(|max| depth >= max) {
            reach.depth_cut |= vertex.parents().next().is_some();
            continue;
        }
        for (parent, at) in vertex.parents() {
            let item = (parent.index, usize::from(at) + offset);
            if seen.contains(&item) {
                continue;
            }
            if seen.len() >= max_visited {
                reach.budget_hit = true;
                break 'climb;
            }
            seen.insert(item);
            reach
                .ancestors
                .entry(item.0)
                .or_insert_with(|| (parent.width.0, BTreeSet::new()))
                .1
                .insert(item.1);
            queue.push_back((item.0, item.1, depth + 1));
        }
    }
    reach
}

fn contexts(
    graph: &PositionedGraph,
    start: Option<Start>,
    reach: &Reach,
    limit: usize,
    context_width: usize,
    label_width: usize,
) -> ContextList {
    let Some(start) = start else {
        return ContextList {
            total: Some(0),
            truncated: false,
            items: Vec::new(),
        };
    };
    let mut ancestors: Vec<_> = reach
        .ancestors
        .iter()
        .filter(|(index, _)| !(start.is_query && **index == start.vertex))
        .map(|(index, (width, offsets))| (*width, *index, offsets))
        .collect();
    ancestors.sort_unstable_by_key(|(width, index, _)| (*width, *index));
    let total = ancestors.len();
    let items = ancestors
        .into_iter()
        .take(limit)
        .map(|(_, index, offsets)| {
            let first = *offsets.first().expect("reached at some offset");
            ContextItem {
                ancestor: text::token_info(graph, index, label_width),
                offsets: offsets.iter().copied().collect(),
                snippet: text::snippet(graph, index, first, query_len(graph, start), context_width),
            }
        })
        .collect();
    ContextList {
        total: (!reach.budget_hit).then_some(total),
        truncated: reach.truncated() || total > limit,
        items,
    }
}

/// Length of the match in chars.
fn query_len(
    graph: &PositionedGraph,
    start: Start,
) -> usize {
    if start.is_query {
        text::width(graph, start.vertex)
    } else {
        start.len
    }
}

fn occurrences(
    view: &View<'_>,
    start: Option<Start>,
    reach: &Reach,
    limit: usize,
    context_width: usize,
) -> OccurrenceList {
    let graph = view.graph;
    let Some(start) = start else {
        return OccurrenceList {
            total: Some(0),
            truncated: false,
            inputs_with_matches: 0,
            items: Vec::new(),
        };
    };
    let len = query_len(graph, start);
    let mut inputs: Vec<_> = view
        .meta
        .inputs
        .iter()
        .filter_map(|input| {
            reach
                .ancestors
                .get(&VertexIndex(input.token))
                .map(|(_, offsets)| (input, offsets))
        })
        .collect();
    inputs.sort_by_key(|(input, _)| input.id);
    let total: usize = inputs.iter().map(|(_, offsets)| offsets.len()).sum();
    let mut items = Vec::with_capacity(total.min(limit));
    'inputs: for (input, offsets) in &inputs {
        let token = VertexIndex(input.token);
        let mut lines = LineCounter::default();
        for &offset in offsets.iter() {
            if items.len() >= limit {
                break 'inputs;
            }
            let (line, column) = lines.advance(graph, token, offset);
            items.push(OccurrenceItem {
                input_id: input.id,
                label: input.label.clone(),
                offset,
                line,
                column,
                snippet: text::snippet(graph, token, offset, len, context_width),
            });
        }
    }
    OccurrenceList {
        total: (!reach.budget_hit).then_some(total),
        truncated: reach.truncated() || total > limit,
        inputs_with_matches: inputs.len(),
        items,
    }
}

/// Line and column of ascending offsets in one vertex, reading each char
/// before the last requested offset once.
#[derive(Debug, Default)]
struct LineCounter {
    position: usize,
    /// Completed lines before `position`.
    newlines: usize,
    /// Offset of the current line's first char.
    line_start: usize,
}

impl LineCounter {
    /// 1-based `(line, column)` of `offset`; offsets must not decrease.
    fn advance(
        &mut self,
        graph: &PositionedGraph,
        token: VertexIndex,
        offset: usize,
    ) -> (usize, usize) {
        debug_assert!(offset >= self.position);
        let mut chunk = String::new();
        text::read_range(graph, token, self.position..offset, &mut chunk);
        for (i, c) in chunk.chars().enumerate() {
            if c == '\n' {
                self.newlines += 1;
                self.line_start = self.position + i + 1;
            }
        }
        self.position = offset;
        (self.newlines + 1, offset - self.line_start + 1)
    }
}

fn path_info(
    graph: &PositionedGraph,
    response: &Response,
    label_width: usize,
) -> PathInfo {
    let path = response.path();
    let span = response.span();
    let step = |location: &OccurrenceLocation| PathStep {
        parent: text::token_info(graph, location.parent, label_width),
        offset: location.offset.into(),
        child: text::token_info(graph, location.child, label_width),
    };
    PathInfo {
        root: text::token_info(graph, response.root_token().index, label_width),
        coverage: match path {
            PathCoverage::EntireRoot(_) => Coverage::EntireRoot,
            PathCoverage::Prefix(_) => Coverage::Prefix,
            PathCoverage::Postfix(_) => Coverage::Postfix,
            PathCoverage::Range(_) => Coverage::Range,
        },
        span: Span {
            start: span.start.into(),
            end: span.end.into(),
        },
        query_start: response.query_start().into(),
        checkpoint_position: response.checkpoint_position().into(),
        query_exhausted: response.query_exhausted(),
        start_path: path
            .start_path()
            .map(|start| start.top_down().map(step).collect())
            .unwrap_or_default(),
        end_path: path
            .end_path()
            .map(|end| end.top_down().map(step).collect())
            .unwrap_or_default(),
        generation: response.generation().base,
    }
}

#[cfg(test)]
mod tests {
    use context_trace::positioned::PositionedGraphRef;

    use super::*;

    fn graph(inputs: &[&str]) -> PositionedGraph {
        let graph = PositionedGraphRef::new(PositionedGraph::default());
        for input in inputs {
            context_read::read(&graph, input.chars()).unwrap();
        }
        graph.read().clone()
    }

    fn offsets_by_text(
        graph: &PositionedGraph,
        reach: &Reach,
    ) -> BTreeSet<(String, Vec<usize>)> {
        reach
            .ancestors
            .iter()
            .map(|(index, (_, offsets))| {
                (text::text(graph, *index), offsets.iter().copied().collect())
            })
            .collect()
    }

    #[test]
    fn overlapping_paths_reach_each_position_once() {
        let graph = graph(&["aaaa", "aaa"]);
        let Found::Vertex { token, .. } = find(&graph, "a").unwrap() else {
            panic!("an atom is a vertex");
        };
        let start = Found::Vertex {
            token,
            response: None,
        }
        .start()
        .unwrap();
        let reach = enumerate(&graph, start, None, usize::MAX);
        assert!(!reach.truncated());
        let reached = offsets_by_text(&graph, &reach);
        for (text, offsets) in &reached {
            let every: Vec<usize> = (0..text.len()).collect();
            assert_eq!(offsets, &every, "{text:?}");
        }
        assert!(reached.iter().any(|(text, _)| text == "aaaa"));
        assert!(reached.iter().any(|(text, _)| text == "aaa"));

        // The budget counts distinct (vertex, offset) pairs, not paths.
        let pairs: usize = reach.ancestors.values().map(|(_, offsets)| offsets.len()).sum();
        assert!(!enumerate(&graph, start, None, pairs).budget_hit);
        let cut = enumerate(&graph, start, None, pairs - 1);
        assert!(cut.budget_hit && cut.truncated());
    }

    #[test]
    fn vertex_starts_at_itself_and_is_no_context() {
        let graph = graph(&["abcabc", "xabc"]);
        let found = find(&graph, "abc").unwrap();
        let start = found.start().unwrap();
        assert_eq!(text::text(&graph, start.vertex), "abc");
        assert_eq!((start.offset, start.is_query), (0, true));
        let reach = enumerate(&graph, start, None, usize::MAX);
        let list = contexts(&graph, Some(start), &reach, usize::MAX, 1, 80);
        let found: Vec<_> = list
            .items
            .iter()
            .map(|item| (item.ancestor.label.as_str(), item.offsets.clone()))
            .collect();
        assert_eq!(found, vec![("xabc", vec![1]), ("abcabc", vec![0, 3])]);
        assert_eq!(list.items[0].snippet.matched, "abc");
        assert_eq!((list.total, list.truncated), (Some(2), false));

        let one = contexts(&graph, Some(start), &reach, 1, 1, 80);
        assert_eq!((one.items.len(), one.total, one.truncated), (1, Some(2), true));
    }

    #[test]
    fn in_host_starts_at_the_span_and_includes_the_host() {
        let graph = graph(&["abcabc"]);
        let found = find(&graph, "bca").unwrap();
        let Found::InHost { response } = &found else {
            panic!("bca is no vertex");
        };
        let start = found.start().unwrap();
        assert_eq!(start.vertex, response.root_token().index);
        assert_eq!((start.offset, start.len, start.is_query), (1, 3, false));
        assert_eq!(text::text_range(&graph, start.vertex, 1..4), "bca");
        let reach = enumerate(&graph, start, None, usize::MAX);
        let list = contexts(&graph, Some(start), &reach, usize::MAX, 0, 80);
        let host = text::text(&graph, start.vertex);
        assert_eq!(list.items[0].ancestor.label, host);
        assert_eq!(list.items[0].offsets, vec![1]);
        assert_eq!(list.items[0].snippet.matched, "bca");
    }

    #[test]
    fn outcomes() {
        let graph = graph(&["abcabc"]);
        let outcome = |query: &str| find(&graph, query).unwrap().outcome(&graph, 80);
        assert_eq!(outcome(""), SearchOutcome::NotFound { unknown_chars: vec![] });
        assert_eq!(outcome("zaxz"), SearchOutcome::NotFound { unknown_chars: vec!['z', 'x'] });
        assert!(matches!(
            outcome("abca"),
            SearchOutcome::InHost { span: Span { start: 0, end: 4 }, .. }
        ));
        assert!(matches!(
            outcome("bcab"),
            SearchOutcome::InHost { span: Span { start: 1, end: 5 }, .. }
        ));
        assert!(matches!(
            outcome("cabb"),
            SearchOutcome::Partial { matched_chars: 3, query_chars: 4, .. }
        ));
    }
}
