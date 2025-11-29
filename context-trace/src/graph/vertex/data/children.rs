//! Child pattern operations for VertexData.
//!
//! Methods for managing and querying child token patterns, including pattern
//! access, mutation, iteration, and selection.

use super::core::VertexData;
use crate::{
    HashSet,
    TokenWidth,
    direction::{
        Direction,
        pattern::PatternDirection,
    },
    graph::{
        Hypergraph,
        getters::ErrorReason,
        kind::GraphKind,
        vertex::{
            ChildPatterns,
            IndexPosition,
            has_vertex_index::ToToken,
            location::{
                SubLocation,
                child::ChildLocation,
            },
            pattern::{
                self,
                IntoPattern,
                Pattern,
                id::PatternId,
                pattern_range::PatternRangeIndex,
                pattern_width,
            },
            token::{
                SubToken,
                Token,
            },
        },
    },
    trace::has_graph::{
        HasGraph,
        TravDir,
    },
};
use itertools::Itertools;
use std::{
    num::NonZeroUsize,
    slice::SliceIndex,
};

/// Helper function to clone child patterns into iterator
pub(crate) fn clone_child_patterns(
    tokens: &'_ ChildPatterns
) -> impl Iterator<Item = Pattern> + '_ {
    tokens.values().cloned()
}

/// Helper function to create localized child iterator
pub(crate) fn localized_children_iter_for_index(
    parent: impl ToToken,
    tokens: &ChildPatterns,
) -> impl IntoIterator<Item = (ChildLocation, &Token)> {
    let parent = parent.to_child();
    tokens.iter().flat_map(move |(&pid, pat)| {
        pat.iter()
            .enumerate()
            .map(move |(i, c)| (ChildLocation::new(parent, pid, i), c))
    })
}

impl VertexData {
    /// Get child pattern by ID
    pub(crate) fn get_child_pattern(
        &self,
        id: &PatternId,
    ) -> Result<&Pattern, ErrorReason> {
        self.children
            .get(id)
            .ok_or(ErrorReason::InvalidPattern(*id))
    }

    /// Get mutable child pattern by ID
    pub(crate) fn get_child_pattern_mut(
        &mut self,
        id: &PatternId,
    ) -> Result<&mut Pattern, ErrorReason> {
        self.children
            .get_mut(id)
            .ok_or(ErrorReason::NoTokenPatterns)
    }

    /// Get child pattern, panicking if not found
    #[track_caller]
    pub(crate) fn expect_child_pattern(
        &self,
        id: &PatternId,
    ) -> &Pattern {
        self.get_child_pattern(id).unwrap_or_else(|_| {
            panic!(
                "Token pattern with id {} does not exist in in vertex {:#?}",
                id, self,
            )
        })
    }

    /// Get mutable child pattern, panicking if not found
    #[track_caller]
    pub(crate) fn expect_child_pattern_mut(
        &mut self,
        id: &PatternId,
    ) -> &mut Pattern {
        self.get_child_pattern_mut(id).unwrap_or_else(|_| {
            panic!("Token pattern with id {} does not exist in in vertex", id,)
        })
    }

    /// Get any child pattern (first one found)
    #[track_caller]
    pub fn expect_any_child_pattern(&self) -> (&PatternId, &Pattern) {
        self.children.iter().next().unwrap_or_else(|| {
            panic!("Pattern vertex has no children {:#?}", self,)
        })
    }

    /// Get child pattern range by ID and range
    pub(crate) fn get_child_pattern_range<R: PatternRangeIndex>(
        &self,
        id: &PatternId,
        range: R,
    ) -> Result<&<R as SliceIndex<[Token]>>::Output, ErrorReason> {
        self.get_child_pattern(id).and_then(|p| {
            pattern::pattern_range::get_child_pattern_range(
                id,
                p,
                range.clone(),
            )
        })
    }

    /// Get child pattern range, panicking if not found
    #[track_caller]
    pub(crate) fn expect_child_pattern_range<R: PatternRangeIndex>(
        &self,
        id: &PatternId,
        range: R,
    ) -> &<R as SliceIndex<[Token]>>::Output {
        let p = self.expect_child_pattern(id);
        pattern::pattern_range::get_child_pattern_range(id, p, range.clone())
            .expect("Range in pattern")
    }

    /// Get child token at specific location
    pub(crate) fn get_child_at(
        &self,
        location: &SubLocation,
    ) -> Result<&Token, ErrorReason> {
        self.children
            .get(&location.pattern_id)
            .ok_or(ErrorReason::InvalidPattern(location.pattern_id))?
            .get(location.sub_index)
            .ok_or(ErrorReason::InvalidChild(location.sub_index))
    }

    /// Get child token at location, panicking if not found
    pub fn expect_child_at(
        &self,
        location: &SubLocation,
    ) -> &Token {
        self.get_child_at(location).unwrap()
    }

    /// Get pattern length, panicking if not found
    #[track_caller]
    pub(crate) fn expect_pattern_len(
        &self,
        id: &PatternId,
    ) -> usize {
        self.expect_child_pattern(id).len()
    }

    /// Get child token offset at location
    pub fn expect_child_offset(
        &self,
        loc: &SubLocation,
    ) -> TokenWidth {
        pattern_width(
            &self.expect_child_pattern(&loc.pattern_id)[0..loc.sub_index],
        )
    }
    /// Add a child pattern without updating cache
    pub fn add_pattern_no_update(
        &mut self,
        id: PatternId,
        pat: Pattern,
    ) {
        if pat.len() < 2 {
            assert!(pat.len() > 1);
        }
        self.children.insert(id, pat.into_pattern());
        #[cfg(any(test, feature = "test-api"))]
        self.invalidate_string_cache();
        self.validate();
    }

    /// Iterator over cloned child patterns
    pub(crate) fn child_pattern_iter(
        &'_ self
    ) -> impl Iterator<Item = Pattern> + '_ {
        clone_child_patterns(&self.children)
    }

    /// Get set of all child patterns
    pub fn child_pattern_set(&self) -> HashSet<Pattern> {
        self.child_pattern_iter().collect()
    }

    /// Get top-down containment nodes
    pub(crate) fn top_down_containment_nodes(&self) -> Vec<(usize, Token)> {
        self.children
            .iter()
            .flat_map(|(_, pat)| {
                pat.iter()
                    .enumerate()
                    .filter(|(_, c)| c.width + TokenWidth(1) == self.width())
                    .map(|(off, c)| (off, *c))
            })
            .sorted_by_key(|&(off, _)| off)
            .collect_vec()
    }

    /// Select children matching predicate
    pub(crate) fn selected_children(
        &self,
        selector: impl Fn(&PatternId, &Pattern) -> Option<usize>,
    ) -> Vec<SubToken> {
        self.children
            .iter()
            .filter_map(|(pid, child_pattern): (_, &Pattern)| {
                selector(pid, child_pattern).map(|sub_index| {
                    let &next = child_pattern.get(sub_index).unwrap();
                    SubToken {
                        location: SubLocation::new(*pid, sub_index),
                        token: next,
                    }
                })
            })
            .collect_vec()
    }

    /// Get prefix children based on graph direction
    pub fn prefix_children<G: HasGraph>(&self) -> Vec<SubToken> {
        self.selected_children(|_, pattern| {
            Some(TravDir::<G>::head_index(pattern))
        })
    }

    /// Get postfix children based on graph direction
    pub fn postfix_children<G: HasGraph>(&self) -> Vec<SubToken>
    where
        <<G::Kind as GraphKind>::Direction as Direction>::Opposite:
            PatternDirection,
    {
        self.selected_children(|_, pattern| {
            Some(TravDir::<G>::last_index(pattern))
        })
    }

    /// Convert patterns to string representation for display
    pub(crate) fn to_pattern_strings<G: GraphKind>(
        &self,
        g: &Hypergraph<G>,
    ) -> Vec<Vec<String>>
    where
        G::Atom: std::fmt::Display,
    {
        self.child_pattern_iter()
            .map(|pat| {
                pat.iter()
                    .map(|c| g.index_string(c.index))
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>()
    }
}

#[allow(dead_code)]
impl VertexData {
    /// Get child pattern at specific position
    pub(crate) fn get_child_pattern_position(
        &self,
        id: &PatternId,
        pos: IndexPosition,
    ) -> Result<&Token, ErrorReason> {
        self.children
            .get(id)
            .and_then(|p| p.get(pos))
            .ok_or(ErrorReason::NoTokenPatterns)
    }

    /// Find child pattern with specified prefix width
    pub(crate) fn get_child_pattern_with_prefix_width(
        &self,
        width: NonZeroUsize,
    ) -> Option<(&PatternId, &Pattern)> {
        self.children
            .iter()
            .find(|(_pid, pat)| pat[0].width == width.get())
    }

    /// Get mutable child token at location
    pub(crate) fn get_child_mut_at(
        &mut self,
        location: &SubLocation,
    ) -> Result<&mut Token, ErrorReason> {
        self.children
            .get_mut(&location.pattern_id)
            .ok_or(ErrorReason::InvalidPattern(location.pattern_id))?
            .get_mut(location.sub_index)
            .ok_or(ErrorReason::InvalidChild(location.sub_index))
    }

    /// Get mutable child token at location, panicking if not found
    pub(crate) fn expect_child_mut_at(
        &mut self,
        location: &SubLocation,
    ) -> &mut Token {
        self.get_child_mut_at(location).unwrap()
    }

    /// Find child pattern ID matching predicate
    pub(crate) fn find_child_pattern_id(
        &self,
        f: impl FnMut(&(&PatternId, &Pattern)) -> bool,
    ) -> Option<PatternId> {
        self.children.iter().find(f).map(|r| *r.0)
    }

    /// Add multiple child patterns without updating cache
    pub(crate) fn add_patterns_no_update(
        &mut self,
        patterns: impl IntoIterator<Item = (PatternId, Pattern)>,
    ) {
        for (id, pat) in patterns {
            if pat.len() < 2 {
                assert!(pat.len() > 1);
            }
            self.children.insert(id, pat.into_pattern());
        }
        #[cfg(any(test, feature = "test-api"))]
        self.invalidate_string_cache();
        self.validate();
    }
    /// Get vector of all child patterns
    pub(crate) fn child_pattern_vec(&self) -> Vec<Pattern> {
        self.child_pattern_iter().collect()
    }

    /// Get largest postfix token across all patterns
    pub(crate) fn largest_postfix(&self) -> (PatternId, Token) {
        let (id, c) = self
            .children
            .iter()
            .fold(None, |acc: Option<(&PatternId, &Token)>, (pid, p)| {
                if let Some(acc) = acc {
                    let c = p.last().unwrap();
                    if c.width > acc.1.width {
                        Some((pid, c))
                    } else {
                        Some(acc)
                    }
                } else {
                    Some((pid, p.last().unwrap()))
                }
            })
            .unwrap();
        (*id, *c)
    }

    /// Iterator over all child tokens
    pub(crate) fn all_children_iter(&self) -> impl IntoIterator<Item = &Token> {
        self.children.iter().flat_map(|(_, pat)| pat.iter())
    }

    /// Iterator over all localized child tokens
    pub(crate) fn all_localized_children_iter(
        &self
    ) -> impl IntoIterator<Item = (ChildLocation, &Token)> {
        localized_children_iter_for_index(self.to_child(), &self.children)
    }
}
