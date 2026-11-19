//! Structural edits applied by join publications.
//!
//! Child locations recorded before an insertion are not automatically valid
//! afterwards: publication replaces ranges of existing patterns by new or
//! existing tokens. [`Publication`] records exactly those composed range
//! replacements, so callers can translate occurrences they already know
//! instead of rediscovering them by search or by depth-first reconnection.
//!
//! Added decompositions never invalidate existing locations and are not
//! recorded. Pattern ids are stable under range replacement. Several
//! publications applied in sequence are kept as ordered stages, because each
//! stage's original indices refer to the graph left by the previous one.

use std::ops::Range;

use context_trace::legacy::*;

/// An original child index range of `location` replaced by `token`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RangeReplacement {
    pub location: PatternLocation,
    /// Child index range in the pattern before publication.
    pub range: Range<usize>,
    pub token: Token,
}

/// Edits of a single `publish` call.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PublicationStage {
    /// Composed (outermost, disjoint) replacements.
    pub replacements: Vec<RangeReplacement>,
    /// Decompositions added to existing vertices.
    pub added: Vec<PatternLocation>,
    /// Vertices allocated for retained candidates.
    pub created: Vec<Token>,
}

/// Evidence returned by one or more publications for translating prior
/// locations.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Publication {
    stages: Vec<PublicationStage>,
}

impl From<PublicationStage> for Publication {
    fn from(stage: PublicationStage) -> Self {
        Self {
            stages: vec![stage],
        }
    }
}

impl Publication {
    /// Append the evidence of a later publication.
    pub fn extend(
        &mut self,
        later: Publication,
    ) {
        self.stages.extend(later.stages);
    }

    pub fn stages(&self) -> &[PublicationStage] {
        &self.stages
    }

    /// Whether `token` was allocated by one of these publications.
    pub fn created(
        &self,
        token: Token,
    ) -> bool {
        self.stages.iter().any(|stage| stage.created.contains(&token))
    }

    /// Whether no existing pattern was rewritten.
    pub fn is_unchanged(&self) -> bool {
        self.stages.iter().all(|s| s.replacements.is_empty())
    }

    /// Translate a boundary path recorded before these publications.
    ///
    /// `path` connects `path[0].parent` to `target` through last (`prefix ==
    /// false`) or first (`prefix == true`) children of the original
    /// patterns. Each location is re-indexed through the composed range
    /// replacements of its pattern. A child moved into a replacement token
    /// gains the frames connecting that token to it; those are read from the
    /// replacement's boundary decompositions, descending only through tokens
    /// created by the same publication.
    ///
    /// # Panics
    ///
    /// Panics if a moved child is not exposed at the boundary of its
    /// replacement, or if the result does not reach `target`.
    pub fn translate(
        &self,
        graph: &Hypergraph,
        path: &[ChildLocation],
        target: Token,
        prefix: bool,
    ) -> Vec<ChildLocation> {
        let translated = self.stages.iter().fold(path.to_vec(), |path, stage| {
            stage.translate(graph, &path, target, prefix)
        });
        if let Some(first) = path.first() {
            assert_eq!(
                boundary_target(graph, first.parent, &translated, prefix),
                target,
                "translated path does not reach its target"
            );
        }
        translated
    }

    /// Translate a boundary chain and extend it with wider boundary tokens
    /// introduced by these publications.
    ///
    /// Like [`Self::translate`], but at each frame the boundary child of any
    /// decomposition rewritten or added by a publication is preferred when it
    /// is wider than the translated child and exposes it at its boundary.
    /// This keeps a widest-boundary chain (such as a postfix traversal) the
    /// widest chain after materialization, without traversing again: only
    /// structure created by the publications is considered.
    pub fn translate_chain(
        &self,
        graph: &Hypergraph,
        path: &[ChildLocation],
        target: Token,
        prefix: bool,
    ) -> Vec<ChildLocation> {
        let translated = self.stages.iter().fold(path.to_vec(), |path, stage| {
            let path = stage.translate(graph, &path, target, prefix);
            stage.widen(graph, &path, target, prefix)
        });
        if let Some(first) = path.first() {
            assert_eq!(
                boundary_target(graph, first.parent, &translated, prefix),
                target,
                "translated chain does not reach its target"
            );
        }
        translated
    }
}

fn boundary_index(
    pattern: &Pattern,
    prefix: bool,
) -> usize {
    if prefix { 0 } else { pattern.len() - 1 }
}

impl PublicationStage {
    /// Patterns of `parent` rewritten or added by this stage.
    fn touched(
        &self,
        parent: Token,
    ) -> Vec<PatternId> {
        let mut ids: Vec<PatternId> = Vec::new();
        for location in self
            .replacements
            .iter()
            .map(|r| r.location)
            .chain(self.added.iter().copied())
        {
            if location.parent == parent && !ids.contains(&location.pattern_id) {
                ids.push(location.pattern_id);
            }
        }
        ids
    }

    /// Replace frames by wider boundary children of touched patterns.
    fn widen(
        &self,
        graph: &Hypergraph,
        path: &[ChildLocation],
        target: Token,
        prefix: bool,
    ) -> Vec<ChildLocation> {
        let mut widened = Vec::with_capacity(path.len());
        for (i, location) in path.iter().enumerate() {
            let child = path.get(i + 1).map_or(target, |next| next.parent);
            let wider = self
                .touched(location.parent)
                .into_iter()
                .filter_map(|pattern_id| {
                    let pattern = graph.expect_pattern_at(
                        location.parent.to_pattern_location(pattern_id),
                    );
                    let sub_index = boundary_index(&pattern, prefix);
                    let token = pattern[sub_index];
                    (token.width() > child.width())
                        .then(|| self.enter(graph, token, child, prefix))
                        .flatten()
                        .map(|rest| {
                            (
                                token.width(),
                                ChildLocation::new(
                                    location.parent,
                                    pattern_id,
                                    sub_index,
                                ),
                                rest,
                            )
                        })
                })
                .max_by_key(|(width, ..)| *width);
            match wider {
                Some((_, location, rest)) => {
                    widened.push(location);
                    widened.extend(rest);
                },
                None => widened.push(*location),
            }
        }
        widened
    }

    fn translate(
        &self,
        graph: &Hypergraph,
        path: &[ChildLocation],
        target: Token,
        prefix: bool,
    ) -> Vec<ChildLocation> {
        let mut translated = Vec::with_capacity(path.len());
        for (i, location) in path.iter().enumerate() {
            let child = path.get(i + 1).map_or(target, |next| next.parent);
            let mut shift = 0;
            let mut moved = None;
            for replacement in self.replacements_in(location) {
                let range = &replacement.range;
                if range.end <= location.sub_index {
                    shift += range.len() - 1;
                } else if range.contains(&location.sub_index) {
                    moved = Some((range.start, replacement.token));
                }
            }
            match moved {
                None => translated.push(ChildLocation {
                    sub_index: location.sub_index - shift,
                    ..*location
                }),
                Some((start, wrapper)) => {
                    translated.push(ChildLocation {
                        sub_index: start - shift,
                        ..*location
                    });
                    translated.extend(
                        self.enter(graph, wrapper, child, prefix)
                            .unwrap_or_else(|| {
                                panic!(
                                    "replacement {} does not expose {} at its boundary",
                                    graph.index_string(wrapper),
                                    graph.index_string(child),
                                )
                            }),
                    );
                },
            }
        }
        translated
    }

    fn replacements_in<'a>(
        &'a self,
        location: &'a ChildLocation,
    ) -> impl Iterator<Item = &'a RangeReplacement> + 'a {
        self.replacements.iter().filter(move |r| {
            r.location.parent == location.parent
                && r.location.pattern_id == location.pattern_id
        })
    }

    /// Frames connecting `wrapper` to `child` along its boundary.
    ///
    /// Follows the widest boundary child that reaches `child`, descending
    /// only through vertices created by this stage.
    fn enter(
        &self,
        graph: &Hypergraph,
        wrapper: Token,
        child: Token,
        prefix: bool,
    ) -> Option<Vec<ChildLocation>> {
        if wrapper == child {
            return Some(Vec::new());
        }
        graph
            .expect_child_patterns(wrapper)
            .iter()
            .filter_map(|(&id, pattern)| {
                let sub_index = boundary_index(pattern, prefix);
                let next = pattern[sub_index];
                let rest = if next == child {
                    Some(Vec::new())
                } else if next.width() > child.width()
                    && self.created.contains(&next)
                {
                    self.enter(graph, next, child, prefix)
                } else {
                    None
                }?;
                Some((next.width(), ChildLocation::new(wrapper, id, sub_index), rest))
            })
            .max_by_key(|(width, ..)| *width)
            .map(|(_, location, mut rest)| {
                rest.insert(0, location);
                rest
            })
    }
}
