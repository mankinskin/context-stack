//! Split graph of a positioned insertion (insert plan rev 3, §4; task 3.2).
//!
//! The root gets the span borders as split positions with a role (start or
//! end border). Top-down in decreasing `(width, index)`, every position is
//! pushed into every child occurrence that strictly contains it. Required
//! intervals are derived per vertex ([`SplitVertex::intervals`]). Read-only.

use std::{
    collections::{
        BTreeMap,
        BTreeSet,
    },
    ops::Range,
};

use context_search::Response;
use context_trace::{
    AtomPosition,
    GraphKind,
    Token,
    TokenWidth,
    graph::vertex::VertexIndex,
    positioned::{
        OccurrenceLocation,
        PositionedGraph,
    },
};

/// Which span border(s) a split position stands for.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Roles {
    /// This occurrence of the vertex contains the span start here.
    pub start: bool,
    /// This occurrence of the vertex contains the span end here.
    pub end: bool,
}

impl Roles {
    pub const START: Self = Self {
        start: true,
        end: false,
    };
    pub const END: Self = Self {
        start: false,
        end: true,
    };
    fn union(
        self,
        other: Self,
    ) -> Self {
        Self {
            start: self.start || other.start,
            end: self.end || other.end,
        }
    }
    fn contains(
        self,
        other: Self,
    ) -> bool {
        (self.start || !other.start) && (self.end || !other.end)
    }
}

/// Split positions of one vertex, relative to it (`0 < p < width`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SplitVertex {
    width: TokenWidth,
    positions: BTreeMap<AtomPosition, Roles>,
}

impl SplitVertex {
    fn new(width: TokenWidth) -> Self {
        Self {
            width,
            positions: BTreeMap::new(),
        }
    }
    fn add(
        &mut self,
        position: AtomPosition,
        roles: Roles,
    ) -> bool {
        debug_assert!(0 < *position && *position < self.width.0);
        let entry = self.positions.entry(position).or_default();
        let added = !entry.contains(roles);
        *entry = entry.union(roles);
        added
    }
    pub fn width(&self) -> TokenWidth {
        self.width
    }
    pub fn positions(&self) -> &BTreeMap<AtomPosition, Roles> {
        &self.positions
    }
    fn with_role(
        &self,
        role: fn(&Roles) -> bool,
    ) -> impl Iterator<Item = AtomPosition> + '_ {
        self.positions
            .iter()
            .filter(move |(_, roles)| role(roles))
            .map(|(&p, _)| p)
    }
    /// Required intervals (§4.3) in join order: increasing width, ties by
    /// start. The root needs only its target `[o, e)`; any other vertex
    /// the postfix `[s, w)` per start position `s`, the prefix `[0, t)` per
    /// end position `t` and the overlap `[s, t)` per pair with `s < t`.
    pub fn intervals(
        &self,
        is_root: bool,
    ) -> Vec<Range<AtomPosition>> {
        let width = AtomPosition::from(self.width);
        let starts: Vec<_> = self.with_role(|r| r.start).collect();
        let ends: Vec<_> = self.with_role(|r| r.end).collect();
        if is_root {
            let start = starts.first().copied().unwrap_or_default();
            let end = ends.first().copied().unwrap_or(width);
            return vec![start..end];
        }
        let mut intervals = BTreeSet::new();
        intervals.extend(starts.iter().map(|&s| (s, width)));
        intervals.extend(ends.iter().map(|&t| (AtomPosition::default(), t)));
        for &s in &starts {
            intervals.extend(ends.iter().filter(|&&t| s < t).map(|&t| (s, t)));
        }
        let mut intervals: Vec<_> = intervals.into_iter().collect();
        intervals.sort_by_key(|&(x, y)| (*y - *x, x));
        intervals.into_iter().map(|(x, y)| x..y).collect()
    }
}

/// Split graph of one response: split positions per vertex (§4.1).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SplitGraph {
    root: Token,
    span: Range<AtomPosition>,
    nodes: BTreeMap<VertexIndex, SplitVertex>,
    #[cfg(any(test, feature = "test-api"))]
    stats: SplitStats,
}

/// How many split positions the response paths provide (`test-api`).
#[cfg(any(test, feature = "test-api"))]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SplitStats {
    /// Positions of all non-root split nodes, one per (vertex, position, role).
    pub positions: usize,
    /// Of these, the positions given by a response path edge.
    pub path_positions: usize,
}

#[cfg(any(test, feature = "test-api"))]
impl SplitStats {
    /// Positions that are not on a response path edge.
    pub fn off_path(&self) -> usize {
        self.positions - self.path_positions
    }
}

/// `(child, position, role)` given by the edges of the response paths.
type PathPosition = (VertexIndex, AtomPosition, Roles);

impl SplitGraph {
    /// Seed the root with the span borders, take the positions of the
    /// response path edges and propagate (§4.2). `None` for an `EntireRoot`
    /// response, which has no split graph.
    pub fn from_response<G: GraphKind>(
        graph: &PositionedGraph<G>,
        response: &Response,
    ) -> Option<Self> {
        if response.is_entire_root() {
            return None;
        }
        let root = response.root_token();
        let span = response.span();
        let path = response.path();
        let mut on_paths: Vec<PathPosition> = Vec::new();
        if let Some(start) = path.start_path() {
            on_paths.extend(path_positions(graph, start.top_down(), span.start, Roles::START));
        }
        if let Some(end) = path.end_path() {
            on_paths.extend(path_positions(graph, end.top_down(), span.end, Roles::END));
        }

        let mut nodes = BTreeMap::new();
        let mut root_node = SplitVertex::new(root.width);
        if *span.start > 0 {
            root_node.add(span.start, Roles::START);
        }
        if *span.end < root.width.0 {
            root_node.add(span.end, Roles::END);
        }
        nodes.insert(root.index, root_node);
        let width = |vertex: VertexIndex| {
            graph.vertex(vertex).expect("vertex of the response").width()
        };
        let mut queue: BTreeSet<(TokenWidth, VertexIndex)> = BTreeSet::new();
        queue.insert((root.width, root.index));
        for &(child, position, roles) in &on_paths {
            nodes
                .entry(child)
                .or_insert_with(|| SplitVertex::new(width(child)))
                .add(position, roles);
            queue.insert((width(child), child));
        }

        #[cfg(debug_assertions)]
        let mut pushed: BTreeSet<(VertexIndex, AtomPosition, bool)> = BTreeSet::new();
        // Children are narrower than their parents, so every vertex is
        // processed after all its parents among the split nodes.
        while let Some((_, vertex)) = queue.pop_last() {
            let composition = graph
                .vertex(vertex)
                .and_then(|v| v.composition())
                .expect("a split node with positions is composite");
            let positions: Vec<_> = nodes[&vertex]
                .positions
                .iter()
                .map(|(&p, &r)| (p, r))
                .collect();
            for (position, roles) in positions {
                for occurrence in composition.crossing(position) {
                    let child = occurrence.child;
                    let inner = position - *occurrence.offset;
                    nodes
                        .entry(child.index)
                        .or_insert_with(|| SplitVertex::new(child.width))
                        .add(inner, roles);
                    queue.insert((child.width, child.index));
                    #[cfg(debug_assertions)]
                    {
                        if roles.start {
                            pushed.insert((child.index, inner, true));
                        }
                        if roles.end {
                            pushed.insert((child.index, inner, false));
                        }
                    }
                }
            }
        }
        #[cfg(debug_assertions)]
        for &(child, position, roles) in &on_paths {
            debug_assert!(
                pushed.contains(&(child, position, roles.start)),
                "response path edge to {child:?} at {position} is not a split edge",
            );
        }

        Some(Self {
            root,
            #[cfg(any(test, feature = "test-api"))]
            stats: SplitStats {
                positions: nodes
                    .iter()
                    .filter(|(v, _)| **v != root.index)
                    .flat_map(|(_, node)| node.positions.values())
                    .map(|roles| roles.start as usize + roles.end as usize)
                    .sum(),
                path_positions: on_paths
                    .iter()
                    .collect::<BTreeSet<_>>()
                    .len(),
            },
            span,
            nodes,
        })
    }
    pub fn root(&self) -> Token {
        self.root
    }
    pub fn span(&self) -> Range<AtomPosition> {
        self.span.clone()
    }
    pub fn nodes(&self) -> &BTreeMap<VertexIndex, SplitVertex> {
        &self.nodes
    }
    pub fn node(
        &self,
        vertex: VertexIndex,
    ) -> Option<&SplitVertex> {
        self.nodes.get(&vertex)
    }
    /// Split nodes in join order: increasing `(width, index)`.
    pub fn bottom_up(&self) -> Vec<(VertexIndex, &SplitVertex)> {
        let mut nodes: Vec<_> = self.nodes.iter().map(|(&v, node)| (v, node)).collect();
        nodes.sort_by_key(|(v, node)| (node.width, *v));
        nodes
    }
    /// Required intervals of `vertex` ([`SplitVertex::intervals`]).
    pub fn intervals(
        &self,
        vertex: VertexIndex,
    ) -> Vec<Range<AtomPosition>> {
        self.nodes
            .get(&vertex)
            .map(|node| node.intervals(vertex == self.root.index))
            .unwrap_or_default()
    }
    #[cfg(any(test, feature = "test-api"))]
    pub fn stats(&self) -> SplitStats {
        self.stats
    }
}

/// Split positions of the response path edges: every step except the last
/// gives its child the border position relative to the child (R4, R5).
fn path_positions<'a, G: GraphKind>(
    graph: &PositionedGraph<G>,
    steps: impl ExactSizeIterator<Item = &'a OccurrenceLocation>,
    border: AtomPosition,
    roles: Roles,
) -> Vec<PathPosition> {
    let last = steps.len().saturating_sub(1);
    let mut position = border;
    let mut out = Vec::new();
    for step in steps.take(last) {
        position -= *step.offset;
        let width = graph.vertex(step.child).expect("path vertex").width().0;
        debug_assert!(
            0 < *position && *position < width,
            "path step {step:?} does not strictly contain the border",
        );
        out.push((step.child, position, roles));
    }
    out
}
