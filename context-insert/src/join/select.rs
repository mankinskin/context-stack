//! Local retention selection between join planning and publication.
//!
//! Planning proposes every interval it needs to resolve boundaries plus all
//! binary sub-merge regroupings of merged partitions. Only part of that is
//! structure worth keeping; the rest are transient helpers. Selection
//! decides, using only the plan itself:
//!
//! - **Evidence** decompositions (joined from an original pattern, perfect
//!   replacements, root patterns) describe how planned structure connects to
//!   the target and the existing graph. Only evidence references count as
//!   parents of a candidate.
//! - **Proposals** (binary sub-merges) never justify their children. A
//!   proposal is accepted only if all its children are retained on their own
//!   and it adds a boundary its host does not already expose.
//!
//! A candidate (other than the target) is
//! - *retained* when it has two or more evidence parent occurrences
//!   (distinct `(host, offset)`), or one parent and two or more
//!   decompositions (evidence or accepted proposals), or when inlining it
//!   would make two decompositions of its parent share a border;
//! - *inlined* when it has exactly one evidence parent occurrence and one
//!   decomposition: its children are substituted at that occurrence;
//! - *dropped* when it has no evidence parent, unless it is an overlap
//!   wrapper (two or more evidence decompositions) linked by an accepted
//!   proposal.
//!
//! Statuses start at "retained" and only ever decrease; candidates are
//! re-evaluated widest first until nothing changes. Inlining preserves the
//! parent occurrence count of the inlined children. Proposals cannot produce
//! circular support because they never count as parents.

use context_trace::*;
use itertools::Itertools;
use std::ops::Range;

use crate::join::plan::{
    CandidateId,
    JoinPlan,
    JoinRef,
    PatternOrigin,
    PlanEdit,
    ProposedPattern,
};

impl PatternOrigin {
    /// Whether this decomposition is only a regrouping proposal.
    pub(crate) fn is_proposal(&self) -> bool {
        matches!(self, PatternOrigin::SubMerge(_))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum Retention {
    Dropped,
    Inlined,
    Retained,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum Host {
    Existing(Token),
    Candidate(usize),
}

/// Identifies a proposal: a candidate pattern or an existing-vertex edit.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum ProposalKey {
    Candidate { index: usize, pattern: usize },
    Edit(usize),
}

struct Selector<'a> {
    graph: &'a Hypergraph,
    plan: &'a JoinPlan,
    status: Vec<Retention>,
}

impl<'a> Selector<'a> {
    fn new(
        graph: &'a Hypergraph,
        plan: &'a JoinPlan,
    ) -> Self {
        Self {
            graph,
            plan,
            status: vec![Retention::Retained; plan.candidates.len()],
        }
    }

    fn target_index(&self) -> Option<usize> {
        self.plan.target.and_then(|t| t.candidate()).map(|id| id.index)
    }

    fn is_retained(
        &self,
        r: &JoinRef,
    ) -> bool {
        match r {
            JoinRef::Existing(_) => true,
            JoinRef::Candidate(id) =>
                self.status[id.index] == Retention::Retained,
        }
    }

    fn evidence(
        &self,
        index: usize,
    ) -> impl Iterator<Item = &'a [JoinRef]> + 'a {
        self.plan.candidates[index]
            .patterns
            .iter()
            .filter(|p| !p.origin.is_proposal())
            .map(|p| p.refs.as_slice())
    }

    fn inline_pattern(
        &self,
        index: usize,
    ) -> &'a [JoinRef] {
        self.evidence(index)
            .next()
            .expect("candidate must have an evidence decomposition")
    }

    /// Substitute inlined candidates by their single decomposition.
    fn expand(
        &self,
        refs: &[JoinRef],
    ) -> Vec<JoinRef> {
        let mut out = Vec::with_capacity(refs.len());
        for r in refs {
            match r {
                JoinRef::Candidate(id)
                    if self.status[id.index] == Retention::Inlined =>
                    out.extend(self.expand(self.inline_pattern(id.index))),
                _ => out.push(*r),
            }
        }
        out
    }

    /// Distinct expanded evidence decompositions of a candidate.
    fn decompositions(
        &self,
        index: usize,
    ) -> Vec<Vec<JoinRef>> {
        let mut out: Vec<Vec<JoinRef>> = Vec::new();
        for p in self.evidence(index) {
            let p = self.expand(p);
            if !out.contains(&p) {
                out.push(p);
            }
        }
        out
    }

    /// Record parent occurrences `(host, atom offset)` of every candidate
    /// referenced by `refs`, including candidates nested in inlined ones.
    fn record(
        &self,
        occ: &mut [HashSet<(Host, usize)>],
        host: Host,
        mut offset: usize,
        refs: &[JoinRef],
    ) {
        for r in refs {
            if let JoinRef::Candidate(id) = r {
                occ[id.index].insert((host, offset));
                if self.status[id.index] == Retention::Inlined {
                    self.record(occ, host, offset, self.inline_pattern(id.index));
                }
            }
            offset += r.width();
        }
    }

    /// Evidence parent occurrences of each candidate.
    fn occurrences(&self) -> Vec<HashSet<(Host, usize)>> {
        let mut occ = vec![HashSet::default(); self.plan.candidates.len()];
        for edit in &self.plan.edits {
            match edit {
                PlanEdit::ReplaceRange {
                    target,
                    pattern_id,
                    range,
                    replacement,
                    ..
                } => {
                    let offset = self.original_offset(*target, pattern_id, range.start);
                    self.record(
                        &mut occ,
                        Host::Existing(*target),
                        offset,
                        &[*replacement],
                    );
                },
                PlanEdit::AddPattern { target, pattern }
                    if !pattern.origin.is_proposal() =>
                    self.record(
                        &mut occ,
                        Host::Existing(*target),
                        0,
                        &pattern.refs,
                    ),
                PlanEdit::AddPattern { .. } => {},
            }
        }
        for (index, status) in self.status.iter().enumerate() {
            if *status == Retention::Retained {
                for p in self.evidence(index) {
                    self.record(&mut occ, Host::Candidate(index), 0, p);
                }
            }
        }
        occ
    }

    fn original_offset(
        &self,
        target: Token,
        pattern_id: &PatternId,
        index: usize,
    ) -> usize {
        self.graph
            .expect_child_patterns(target)
            .get(pattern_id)
            .expect("replacement pattern must exist")
            .iter()
            .take(index)
            .map(|t| *t.width())
            .sum()
    }

    /// Interior borders of a host's evidence decompositions under the
    /// current statuses (inlined candidates expanded, retained replacements
    /// applied to original patterns).
    fn evidence_borders(
        &self,
        host: Host,
    ) -> HashSet<usize> {
        match host {
            Host::Candidate(index) => self
                .decompositions(index)
                .iter()
                .flat_map(|p| boundaries(p))
                .collect(),
            Host::Existing(target) => {
                let replaced: Vec<(PatternId, Range<usize>)> = self
                    .plan
                    .edits
                    .iter()
                    .filter_map(|edit| match edit {
                        PlanEdit::ReplaceRange {
                            target: t,
                            pattern_id,
                            range,
                            replacement,
                            ..
                        } if *t == target && self.is_retained(replacement) =>
                            Some((*pattern_id, range.clone())),
                        _ => None,
                    })
                    .collect();
                let mut borders = HashSet::default();
                for (pid, pattern) in self.graph.expect_child_patterns(target)
                {
                    let mut offset = 0;
                    for (k, child) in pattern.iter().enumerate() {
                        if k > 0
                            && !replaced.iter().any(|(p, r)| {
                                *p == pid && r.start < k && k < r.end
                            })
                        {
                            borders.insert(offset);
                        }
                        offset += *child.width();
                    }
                }
                for edit in &self.plan.edits {
                    if let PlanEdit::AddPattern { target: t, pattern } = edit
                        && *t == target
                        && !pattern.origin.is_proposal()
                    {
                        borders.extend(boundaries(&self.expand(&pattern.refs)));
                    }
                }
                borders
            },
        }
    }

    /// All proposals with their host, in plan order.
    fn proposals(&self) -> Vec<(ProposalKey, Host, &'a [JoinRef])> {
        let plan = self.plan;
        let mut out = Vec::new();
        for (index, candidate) in plan.candidates.iter().enumerate() {
            if self.status[index] != Retention::Retained {
                continue;
            }
            for (pattern, p) in candidate.patterns.iter().enumerate() {
                if p.origin.is_proposal() {
                    out.push((
                        ProposalKey::Candidate { index, pattern },
                        Host::Candidate(index),
                        p.refs.as_slice(),
                    ));
                }
            }
        }
        for (e, edit) in plan.edits.iter().enumerate() {
            if let PlanEdit::AddPattern { target, pattern } = edit
                && pattern.origin.is_proposal()
            {
                out.push((
                    ProposalKey::Edit(e),
                    Host::Existing(*target),
                    pattern.refs.as_slice(),
                ));
            }
        }
        out
    }

    /// Accept proposals whose children are retained and which add a border
    /// their host does not already expose. Returns accepted proposals and
    /// the resulting border set of each host that has proposals.
    fn accepted(&self) -> (HashSet<ProposalKey>, HashMap<Host, HashSet<usize>>) {
        let mut accepted = HashSet::default();
        let mut bounds: HashMap<Host, HashSet<usize>> = HashMap::default();
        for (key, host, refs) in self.proposals() {
            let bounds = bounds
                .entry(host)
                .or_insert_with(|| self.evidence_borders(host));
            if !refs.iter().all(|r| self.is_retained(r)) {
                continue;
            }
            let new = boundaries(refs);
            if new.iter().any(|b| bounds.contains(b)) {
                continue;
            }
            bounds.extend(new);
            accepted.insert(key);
        }
        (accepted, bounds)
    }

    fn linked(
        &self,
        accepted: &HashSet<ProposalKey>,
    ) -> HashSet<usize> {
        self.proposals()
            .into_iter()
            .filter(|(key, _, _)| accepted.contains(key))
            .flat_map(|(_, _, refs)| refs)
            .filter_map(|r| r.candidate().map(|id| id.index))
            .collect()
    }

    /// Whether substituting `index` at its single occurrence keeps the
    /// host's decompositions border-disjoint.
    fn can_inline(
        &self,
        index: usize,
        (host, offset): (Host, usize),
        host_borders: &HashMap<Host, HashSet<usize>>,
    ) -> bool {
        let own;
        let borders = match host_borders.get(&host) {
            Some(b) => b,
            None => {
                own = self.evidence_borders(host);
                &own
            },
        };
        boundaries(&self.expand(self.inline_pattern(index)))
            .into_iter()
            .all(|b| !borders.contains(&(offset + b)))
    }

    fn decide(
        &self,
        index: usize,
    ) -> Retention {
        let occ = self.occurrences();
        let (accepted, host_borders) = self.accepted();
        let parents = &occ[index];
        let evidence = self.decompositions(index).len();
        let proposals = self
            .proposals()
            .iter()
            .filter(|(key, host, _)| {
                *host == Host::Candidate(index) && accepted.contains(key)
            })
            .count();
        match parents.len() {
            0 =>
            // an overlap wrapper only referenced by accepted proposals
                if evidence >= 2 && self.linked(&accepted).contains(&index) {
                    Retention::Retained
                } else {
                    Retention::Dropped
                },
            1 => {
                let parent = *parents.iter().next().unwrap();
                if evidence + proposals >= 2
                    || !self.can_inline(index, parent, &host_borders)
                {
                    Retention::Retained
                } else {
                    Retention::Inlined
                }
            },
            _ => Retention::Retained,
        }
    }

    /// Greatest fixpoint: statuses only decrease. Candidates are evaluated
    /// one at a time, widest first, so every decision sees the effect of
    /// earlier ones on its parents.
    fn run(&mut self) -> HashSet<ProposalKey> {
        let target = self.target_index();
        let mut order: Vec<usize> = (0..self.status.len())
            .filter(|i| Some(*i) != target)
            .collect();
        order.sort_by_key(|i| {
            (std::cmp::Reverse(self.plan.candidates[*i].width()), *i)
        });
        loop {
            let mut changed = false;
            for &index in &order {
                let next = self.decide(index);
                if next < self.status[index] {
                    self.status[index] = next;
                    changed = true;
                }
            }
            if !changed {
                return self.accepted().0;
            }
        }
    }
}

/// Interior atom boundaries of a pattern.
fn boundaries(refs: &[JoinRef]) -> Vec<usize> {
    let mut offset = 0;
    refs.iter()
        .take(refs.len().saturating_sub(1))
        .map(|r| {
            offset += r.width();
            offset
        })
        .collect()
}

/// Select the structure to publish.
///
/// Returns a plan containing only retained candidates (re-indexed, with
/// inlined helpers substituted), the edits still needed, and accepted
/// proposals.
pub(crate) fn select(
    graph: &Hypergraph,
    plan: JoinPlan,
) -> JoinPlan {
    let mut selector = Selector::new(graph, &plan);
    let accepted = selector.run();
    let status = selector.status.clone();

    // re-index retained candidates children-first, merging any that became
    // identical through inlining
    let mut order: Vec<usize> = (0..plan.candidates.len())
        .filter(|i| status[*i] == Retention::Retained)
        .collect();
    order.sort_by_key(|i| (plan.candidates[*i].width(), *i));

    let mut out = JoinPlan::new(plan.provenance.clone());
    let mut remap: HashMap<usize, CandidateId> = HashMap::default();
    let map_refs = |remap: &HashMap<usize, CandidateId>, refs: Vec<JoinRef>| {
        refs.into_iter()
            .map(|r| match r {
                JoinRef::Candidate(id) => JoinRef::Candidate(
                    *remap.get(&id.index).expect("child must be selected"),
                ),
                existing => existing,
            })
            .collect_vec()
    };
    for index in order {
        let candidate = &plan.candidates[index];
        let mut patterns: Vec<ProposedPattern> = Vec::new();
        for (p, pattern) in candidate.patterns.iter().enumerate() {
            let keep = !pattern.origin.is_proposal()
                || accepted.contains(&ProposalKey::Candidate { index, pattern: p });
            if !keep {
                continue;
            }
            let refs = map_refs(&remap, selector.expand(&pattern.refs));
            if !patterns.iter().any(|q| q.refs == refs) {
                patterns.push(ProposedPattern {
                    refs,
                    origin: pattern.origin,
                });
            }
        }
        let new = out.propose(candidate.host, patterns);
        let id = new.candidate().expect("retained candidate stays a candidate");
        for interval in &candidate.intervals {
            out.record_interval(*interval, new);
        }
        remap.insert(index, id);
    }

    let mut added: HashMap<Token, Vec<Vec<JoinRef>>> = HashMap::default();
    for (e, edit) in plan.edits.iter().enumerate() {
        match edit {
            PlanEdit::ReplaceRange {
                target,
                pattern_id,
                range,
                replacement,
                origin,
            } => {
                if let JoinRef::Candidate(id) = replacement {
                    debug_assert_ne!(status[id.index], Retention::Dropped);
                    if status[id.index] == Retention::Inlined {
                        // the replaced range already is the inlined pattern
                        continue;
                    }
                }
                out.push_edit(PlanEdit::ReplaceRange {
                    target: *target,
                    pattern_id: *pattern_id,
                    range: range.clone(),
                    replacement: map_refs(&remap, vec![*replacement])[0],
                    origin: *origin,
                });
            },
            PlanEdit::AddPattern { target, pattern } => {
                if pattern.origin.is_proposal()
                    && !accepted.contains(&ProposalKey::Edit(e))
                {
                    continue;
                }
                let refs = map_refs(&remap, selector.expand(&pattern.refs));
                let seen = added.entry(*target).or_insert_with(|| {
                    graph
                        .expect_child_patterns(*target)
                        .values()
                        .map(|p| {
                            p.iter().map(|t| JoinRef::Existing(*t)).collect()
                        })
                        .collect()
                });
                if seen.contains(&refs) {
                    continue;
                }
                seen.push(refs.clone());
                out.push_edit(PlanEdit::AddPattern {
                    target: *target,
                    pattern: ProposedPattern {
                        refs,
                        origin: pattern.origin,
                    },
                });
            },
        }
    }
    out.target = plan.target.map(|t| map_refs(&remap, vec![t])[0]);
    out
}

/// Retention status of each candidate (test inspection).
#[cfg(test)]
pub(crate) fn retention(
    graph: &Hypergraph,
    plan: &JoinPlan,
) -> Vec<Retention> {
    let mut selector = Selector::new(graph, plan);
    selector.run();
    selector.status
}
