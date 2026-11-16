//! Operation-local join plan.
//!
//! The join phase no longer mutates the hypergraph while it computes merged
//! partitions. Instead it records a [`JoinPlan`]:
//!
//! - **candidates**: proposed new vertices, identified by [`CandidateId`]
//!   (never a graph vertex index), each with proposed decompositions tagged
//!   by [`PatternOrigin`];
//! - **edits**: proposed modifications of existing vertices (added patterns,
//!   range replacements inside an original pattern), also tagged by origin;
//! - **provenance**: the requirement/incidence table collected during split
//!   collection.
//!
//! While a node is being merged, its decompositions are read through a
//! [`NodeOverlay`]: the original graph patterns with the replacements
//! proposed so far applied. Replacements are recorded against *original*
//! child index ranges (stable positions), so publication composes them
//! without replaying stale indices.
//!
//! [`select`](crate::join::select::select) (local retention: helpers are
//! inlined, sub-merge proposals accepted only with independent support) and
//! [`publish`] form the single explicit boundary at which the plan becomes
//! graph structure.

use std::{
    fmt::Debug,
    ops::Range,
};

use context_trace::*;

use crate::split::{
    cache::position::PosKey,
    provenance::SplitProvenance,
};

/// Identifier of a planned (not yet materialized) vertex.
///
/// Carries the candidate width so width arithmetic during planning does not
/// need to consult the plan.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) struct CandidateId {
    pub(crate) index: usize,
    pub(crate) width: usize,
}

/// Reference to either an existing graph token or a planned candidate.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum JoinRef {
    Existing(Token),
    Candidate(CandidateId),
}

impl JoinRef {
    pub(crate) fn existing(&self) -> Option<Token> {
        match self {
            JoinRef::Existing(token) => Some(*token),
            JoinRef::Candidate(_) => None,
        }
    }
    #[allow(dead_code)]
    pub(crate) fn candidate(&self) -> Option<CandidateId> {
        match self {
            JoinRef::Existing(_) => None,
            JoinRef::Candidate(id) => Some(*id),
        }
    }
}

impl From<Token> for JoinRef {
    fn from(token: Token) -> Self {
        JoinRef::Existing(token)
    }
}

impl Wide for JoinRef {
    fn width(&self) -> TokenWidth {
        match self {
            JoinRef::Existing(token) => token.width(),
            JoinRef::Candidate(id) => TokenWidth(id.width),
        }
    }
}

/// Split position key over join references.
pub(crate) type RefPosKey = PosKey<JoinRef>;

impl From<PosKey> for RefPosKey {
    fn from(key: PosKey) -> Self {
        Self {
            index: JoinRef::Existing(key.index),
            pos: key.pos,
        }
    }
}

/// Which binary sub-merge enumeration produced a decomposition.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SubMergeKind {
    /// Binary combinations added to a newly merged partition token.
    Partition,
    /// Binary combinations added to an intermediary node's full range.
    Intermediary,
}

/// Why a decomposition was proposed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PatternOrigin {
    /// Joined from an original decomposition (`source`) of the merged node.
    Joined { source: PatternId },
    /// A partition range was perfect in an original decomposition and is
    /// replaced in place by the merged token.
    PerfectReplacement,
    /// Binary sub-merge enumeration over already merged partitions.
    SubMerge(SubMergeKind),
    /// New root decomposition containing the inserted target.
    RootPattern,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ProposedPattern {
    pub(crate) refs: Vec<JoinRef>,
    pub(crate) origin: PatternOrigin,
}

/// A materialized interval of an existing node `host` at atom offsets
/// `start..end`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct HostInterval {
    pub(crate) host: Token,
    pub(crate) start: usize,
    pub(crate) end: usize,
}

#[derive(Debug, Clone)]
pub(crate) struct Candidate {
    pub(crate) id: CandidateId,
    /// Node whose merge proposed this candidate.
    #[allow(dead_code)]
    pub(crate) host: Token,
    pub(crate) patterns: Vec<ProposedPattern>,
    /// Node intervals this candidate was registered for.
    pub(crate) intervals: Vec<HostInterval>,
}

impl Candidate {
    pub(crate) fn width(&self) -> usize {
        self.id.width
    }
}

/// Proposed modification of an existing vertex.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum PlanEdit {
    AddPattern {
        target: Token,
        pattern: ProposedPattern,
    },
    /// Replace the *original* child index range `range` of `pattern_id`
    /// with `replacement`.
    ReplaceRange {
        target: Token,
        pattern_id: PatternId,
        range: Range<usize>,
        replacement: JoinRef,
        origin: PatternOrigin,
    },
}

#[derive(Debug, Default)]
pub(crate) struct JoinPlan {
    pub(crate) candidates: Vec<Candidate>,
    pub(crate) edits: Vec<PlanEdit>,
    pub(crate) target: Option<JoinRef>,
    /// Split-collection provenance, kept for inspection and selection.
    #[allow(dead_code)]
    pub(crate) provenance: SplitProvenance,
    by_pattern: HashMap<Vec<JoinRef>, CandidateId>,
}

impl JoinPlan {
    pub(crate) fn new(provenance: SplitProvenance) -> Self {
        Self {
            provenance,
            ..Default::default()
        }
    }

    pub(crate) fn candidate(
        &self,
        id: CandidateId,
    ) -> &Candidate {
        &self.candidates[id.index]
    }

    /// Propose a vertex with the given decompositions.
    ///
    /// Mirrors `Hypergraph::insert_patterns`: a single-element pattern
    /// resolves to that element. Proposals sharing a decomposition with an
    /// earlier candidate resolve to that candidate.
    pub(crate) fn propose(
        &mut self,
        host: Token,
        patterns: Vec<ProposedPattern>,
    ) -> JoinRef {
        assert!(!patterns.is_empty(), "Tried to propose no patterns");
        if let Some(single) = patterns.iter().find(|p| p.refs.len() == 1) {
            return single.refs[0];
        }
        if let Some(&id) =
            patterns.iter().find_map(|p| self.by_pattern.get(&p.refs))
        {
            for pattern in patterns {
                self.add_candidate_pattern(id, pattern);
            }
            return JoinRef::Candidate(id);
        }
        let width = patterns[0].refs.iter().map(|elem| *elem.width()).sum();
        let id = CandidateId {
            index: self.candidates.len(),
            width,
        };
        for p in &patterns {
            debug_assert_eq!(
                p.refs.iter().map(|elem| *elem.width()).sum::<usize>(),
                width,
                "candidate decompositions must have equal width"
            );
            self.by_pattern.insert(p.refs.clone(), id);
        }
        self.candidates.push(Candidate {
            id,
            host,
            patterns,
            intervals: Vec::new(),
        });
        JoinRef::Candidate(id)
    }

    /// Add a decomposition to a candidate unless it already has it.
    pub(crate) fn add_candidate_pattern(
        &mut self,
        id: CandidateId,
        pattern: ProposedPattern,
    ) {
        let candidate = &mut self.candidates[id.index];
        if candidate.patterns.iter().any(|p| p.refs == pattern.refs) {
            return;
        }
        self.by_pattern.entry(pattern.refs.clone()).or_insert(id);
        candidate.patterns.push(pattern);
    }

    pub(crate) fn candidate_pattern_set(
        &self,
        id: CandidateId,
    ) -> HashSet<Vec<JoinRef>> {
        self.candidate(id)
            .patterns
            .iter()
            .map(|p| p.refs.clone())
            .collect()
    }

    /// Patterns proposed to be added to an existing token so far.
    pub(crate) fn added_patterns(
        &self,
        target: Token,
    ) -> impl Iterator<Item = &Vec<JoinRef>> {
        self.edits.iter().filter_map(move |edit| match edit {
            PlanEdit::AddPattern { target: t, pattern } if *t == target =>
                Some(&pattern.refs),
            _ => None,
        })
    }

    pub(crate) fn record_interval(
        &mut self,
        interval: HostInterval,
        token: JoinRef,
    ) {
        if let JoinRef::Candidate(id) = token {
            let intervals = &mut self.candidates[id.index].intervals;
            if !intervals.contains(&interval) {
                intervals.push(interval);
            }
        }
    }

    pub(crate) fn push_edit(
        &mut self,
        edit: PlanEdit,
    ) {
        self.edits.push(edit);
    }

    /// Find the candidate registered for a node interval.
    #[allow(dead_code)]
    pub(crate) fn candidate_for_interval(
        &self,
        interval: HostInterval,
    ) -> Option<&Candidate> {
        self.candidates
            .iter()
            .find(|c| c.intervals.contains(&interval))
    }

    /// Expand a reference to its atom sequence using the first decomposition
    /// of each candidate and the graph for existing tokens.
    #[allow(dead_code)]
    pub(crate) fn expand_atoms(
        &self,
        graph: &Hypergraph,
        r: JoinRef,
    ) -> Vec<Token> {
        match r {
            JoinRef::Existing(token) => expand_existing(graph, token),
            JoinRef::Candidate(id) => self.candidate(id).patterns[0]
                .refs
                .iter()
                .flat_map(|r| self.expand_atoms(graph, *r))
                .collect(),
        }
    }
}

fn expand_existing(
    graph: &Hypergraph,
    token: Token,
) -> Vec<Token> {
    if *token.width() == 1 {
        return vec![token];
    }
    let data = graph.expect_vertex_data(token);
    let (_, pattern) = data.expect_any_child_pattern();
    pattern
        .iter()
        .flat_map(|c| expand_existing(graph, *c))
        .collect()
}

/// Element of an overlay pattern: a reference covering the original child
/// index range `span`.
#[derive(Debug, Clone)]
struct OverlayElem {
    r: JoinRef,
    span: Range<usize>,
}

/// An original decomposition with the replacements proposed so far applied.
#[derive(Debug, Clone)]
pub(crate) struct OverlayPattern {
    elems: Vec<OverlayElem>,
    refs: Vec<JoinRef>,
}

impl OverlayPattern {
    fn from_original(pattern: &Pattern) -> Self {
        let elems: Vec<_> = pattern
            .iter()
            .enumerate()
            .map(|(i, t)| OverlayElem {
                r: JoinRef::Existing(*t),
                span: i..i + 1,
            })
            .collect();
        let refs = elems.iter().map(|e| e.r).collect();
        Self { elems, refs }
    }
    pub(crate) fn refs(&self) -> &[JoinRef] {
        &self.refs
    }
    /// Replace the overlay range `range` by `r`, returning the covered
    /// original child index range.
    fn replace(
        &mut self,
        range: Range<usize>,
        r: JoinRef,
    ) -> Range<usize> {
        assert!(
            range.start < range.end && range.end <= self.elems.len(),
            "invalid overlay replacement range {range:?}"
        );
        let span =
            self.elems[range.start].span.start..self.elems[range.end - 1].span.end;
        self.elems
            .splice(range, [OverlayElem { r, span: span.clone() }]);
        self.refs = self.elems.iter().map(|e| e.r).collect();
        span
    }
}

/// Operation-local view of an existing node's decompositions during its
/// merge.
#[derive(Debug, Clone)]
pub(crate) struct NodeOverlay {
    token: Token,
    order: Vec<PatternId>,
    patterns: HashMap<PatternId, OverlayPattern>,
}

impl NodeOverlay {
    pub(crate) fn new(
        token: Token,
        patterns: &ChildPatterns,
    ) -> Self {
        let order: Vec<_> = patterns.keys().cloned().collect();
        let patterns = patterns
            .iter()
            .map(|(pid, p)| (*pid, OverlayPattern::from_original(p)))
            .collect();
        Self {
            token,
            order,
            patterns,
        }
    }
    pub(crate) fn pattern(
        &self,
        pid: &PatternId,
    ) -> &[JoinRef] {
        self.patterns
            .get(pid)
            .unwrap_or_else(|| {
                panic!("pattern {pid:?} not found in {:?}", self.token)
            })
            .refs()
    }
    /// Patterns in the graph's iteration order.
    pub(crate) fn iter(&self) -> impl Iterator<Item = (&PatternId, &[JoinRef])> {
        self.order.iter().map(|pid| (pid, self.pattern(pid)))
    }
    pub(crate) fn first(&self) -> (PatternId, &[JoinRef]) {
        let pid = *self
            .order
            .first()
            .expect("node must have at least one child pattern");
        (pid, self.pattern(&pid))
    }
    pub(crate) fn pattern_set(&self) -> HashSet<Vec<JoinRef>> {
        self.patterns.values().map(|p| p.refs().to_vec()).collect()
    }
    /// Propose an in-place range replacement. Returns the original child
    /// index range it covers.
    pub(crate) fn replace(
        &mut self,
        pid: PatternId,
        range: Range<usize>,
        r: JoinRef,
    ) -> Range<usize> {
        self.patterns
            .get_mut(&pid)
            .expect("replacement pattern must exist")
            .replace(range, r)
    }
}

/// Materialize a plan into the graph and return the target token.
///
/// Candidates are inserted children-first; edits to existing vertices are
/// applied afterwards. Range replacements in the same original pattern are
/// composed from their original index ranges (outermost wins) and applied
/// right-to-left so earlier indices stay valid.
pub(crate) fn publish(
    graph: &Hypergraph,
    plan: JoinPlan,
) -> Token {
    let mut tokens: Vec<Option<Token>> = vec![None; plan.candidates.len()];
    for index in 0..plan.candidates.len() {
        materialize(graph, &plan, index, &mut tokens);
    }
    let resolve = |r: JoinRef| -> Token {
        match r {
            JoinRef::Existing(t) => t,
            JoinRef::Candidate(id) =>
                tokens[id.index].expect("candidate must be materialized"),
        }
    };

    // compose replacements per (target, pattern), preserving first-seen order
    let mut replacement_groups: Vec<((Token, PatternId), Vec<(Range<usize>, JoinRef)>)> =
        Vec::new();
    for edit in &plan.edits {
        if let PlanEdit::ReplaceRange {
            target,
            pattern_id,
            range,
            replacement,
            ..
        } = edit
        {
            let key = (*target, *pattern_id);
            match replacement_groups.iter_mut().find(|(k, _)| *k == key) {
                Some((_, group)) => group.push((range.clone(), *replacement)),
                None => replacement_groups
                    .push((key, vec![(range.clone(), *replacement)])),
            }
        }
    }
    for ((target, pid), group) in replacement_groups {
        let mut outermost = compose_replacements(group);
        outermost.sort_by(|a, b| b.0.start.cmp(&a.0.start));
        for (range, r) in outermost {
            graph.replace_in_pattern(
                target.to_pattern_location(pid),
                range,
                vec![resolve(r)],
            );
        }
    }
    for edit in &plan.edits {
        if let PlanEdit::AddPattern { target, pattern } = edit {
            let pattern: Pattern =
                pattern.refs.iter().map(|r| resolve(*r)).collect();
            graph.add_pattern_with_update(*target, pattern);
        }
    }
    resolve(plan.target.expect("plan has no target"))
}

/// Keep only the outermost replacements; later proposals win ties.
fn compose_replacements(
    group: Vec<(Range<usize>, JoinRef)>
) -> Vec<(Range<usize>, JoinRef)> {
    let contains = |outer: &Range<usize>, inner: &Range<usize>| {
        outer.start <= inner.start && inner.end <= outer.end
    };
    group
        .iter()
        .enumerate()
        .filter(|(i, (range, _))| {
            !group.iter().enumerate().any(|(j, (other, _))| {
                j != *i
                    && contains(other, range)
                    && (other != range || j > *i)
            })
        })
        .map(|(_, (range, r))| {
            debug_assert!(
                group.iter().all(|(other, _)| other.end <= range.start
                    || range.end <= other.start
                    || contains(other, range)
                    || contains(range, other)),
                "partially overlapping replacements"
            );
            (range.clone(), *r)
        })
        .collect()
}

fn materialize(
    graph: &Hypergraph,
    plan: &JoinPlan,
    index: usize,
    tokens: &mut Vec<Option<Token>>,
) -> Token {
    if let Some(token) = tokens[index] {
        return token;
    }
    let candidate = &plan.candidates[index];
    let patterns: Vec<Pattern> = candidate
        .patterns
        .iter()
        .map(|p| {
            p.refs
                .iter()
                .map(|r| match r {
                    JoinRef::Existing(t) => *t,
                    JoinRef::Candidate(id) => {
                        assert_ne!(id.index, index, "self-referencing candidate");
                        materialize(graph, plan, id.index, tokens)
                    },
                })
                .collect()
        })
        .collect();
    let mut patterns = patterns.into_iter();
    let first = patterns.next().expect("candidate without patterns");
    assert!(first.len() > 1, "candidate patterns must have 2+ children");
    let token = graph.insert_pattern(first);
    for pattern in patterns {
        graph.add_pattern_with_update(token, pattern);
    }
    debug_assert_eq!(*token.width(), candidate.width());
    tokens[index] = Some(token);
    token
}
