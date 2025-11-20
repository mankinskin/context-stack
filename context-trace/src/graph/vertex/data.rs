use crate::{
    HashSet,
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
            PatternId,
            VertexIndex,
            VertexParents,
            has_vertex_index::{
                HasVertexIndex,
                ToToken,
            },
            key::VertexKey,
            location::{
                SubLocation,
                child::ChildLocation,
            },
            parent::{
                Parent,
                PatternIndex,
            },
            pattern::{
                self,
                IntoPattern,
                Pattern,
                pattern_range::PatternRangeIndex,
                pattern_width,
            },
            token::Token,
            wide::Wide,
        },
    },
    logging::compact_format::{
        CompactFormat,
        write_indent,
    },
    trace::has_graph::{
        HasGraph,
        TravDir,
    },
};
use derive_builder::Builder;
use either::Either;
use itertools::Itertools;
use serde::{
    Deserialize,
    Serialize,
};
use std::{
    fmt::{
        Debug,
        Display,
    },
    num::NonZeroUsize,
    slice::SliceIndex,
};

use crate::graph::vertex::token::SubToken;

pub(crate) fn clone_child_patterns(
    tokens: &'_ ChildPatterns
) -> impl Iterator<Item = Pattern> + '_ {
    tokens.values().cloned()
}
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

#[derive(Debug, Builder, Serialize, Deserialize)]
#[cfg_attr(not(any(test, feature = "test-api")), derive(PartialEq, Eq, Clone))]
pub struct VertexData {
    pub(crate) width: usize,
    pub(crate) index: VertexIndex,

    #[builder(default)]
    pub(crate) key: VertexKey,

    #[builder(default)]
    pub(crate) parents: VertexParents,

    #[builder(default)]
    pub(crate) children: ChildPatterns,

    #[cfg(any(test, feature = "test-api"))]
    #[serde(skip)]
    #[builder(setter(skip), default = "std::sync::RwLock::new(None)")]
    pub(crate) cached_string: std::sync::RwLock<Option<String>>,
}

#[cfg(any(test, feature = "test-api"))]
impl PartialEq for VertexData {
    fn eq(
        &self,
        other: &Self,
    ) -> bool {
        self.width == other.width
            && self.index == other.index
            && self.key == other.key
            && self.parents == other.parents
            && self.children == other.children
        // cached_string is not compared
    }
}

#[cfg(any(test, feature = "test-api"))]
impl Eq for VertexData {}

#[cfg(any(test, feature = "test-api"))]
impl Clone for VertexData {
    fn clone(&self) -> Self {
        Self {
            width: self.width,
            index: self.index,
            key: self.key.clone(),
            parents: self.parents.clone(),
            children: self.children.clone(),
            cached_string: std::sync::RwLock::new(None), // Don't clone cache
        }
    }
}

impl VertexData {
    pub(crate) fn new(
        index: VertexIndex,
        width: usize,
    ) -> Self {
        Self {
            width,
            key: VertexKey::default(),
            index,
            parents: VertexParents::default(),
            children: ChildPatterns::default(),
            #[cfg(any(test, feature = "test-api"))]
            cached_string: std::sync::RwLock::new(None),
        }
    }
    pub(crate) fn get_width(&self) -> usize {
        self.width
    }
    pub(crate) fn get_parent(
        &self,
        index: impl HasVertexIndex,
    ) -> Result<&Parent, ErrorReason> {
        let index = index.vertex_index();
        self.parents
            .get(&index)
            .ok_or(ErrorReason::ErrorReasoningParent(index))
    }
    pub(crate) fn get_parent_mut(
        &mut self,
        index: impl HasVertexIndex,
    ) -> Result<&mut Parent, ErrorReason> {
        let index = index.vertex_index();
        self.parents
            .get_mut(&index)
            .ok_or(ErrorReason::ErrorReasoningParent(index))
    }
    #[track_caller]
    pub(crate) fn expect_parent(
        &self,
        index: impl HasVertexIndex,
    ) -> &Parent {
        self.get_parent(index).unwrap()
    }
    #[track_caller]
    pub(crate) fn expect_parent_mut(
        &mut self,
        index: impl HasVertexIndex,
    ) -> &mut Parent {
        self.get_parent_mut(index).unwrap()
    }
    pub fn parents(&self) -> &VertexParents {
        &self.parents
    }
    pub(crate) fn parents_mut(&mut self) -> &mut VertexParents {
        &mut self.parents
    }
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
    pub(crate) fn get_child_pattern_with_prefix_width(
        &self,
        width: NonZeroUsize,
    ) -> Option<(&PatternId, &Pattern)> {
        self.children
            .iter()
            .find(|(_pid, pat)| pat[0].width() == width.get())
    }
    pub(crate) fn get_child_pattern(
        &self,
        id: &PatternId,
    ) -> Result<&Pattern, ErrorReason> {
        self.children
            .get(id)
            .ok_or(ErrorReason::InvalidPattern(*id))
    }
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
    pub fn expect_child_at(
        &self,
        location: &SubLocation,
    ) -> &Token {
        self.get_child_at(location).unwrap()
    }
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
    pub(crate) fn expect_child_mut_at(
        &mut self,
        location: &SubLocation,
    ) -> &mut Token {
        self.get_child_mut_at(location).unwrap()
    }
    #[track_caller]
    pub(crate) fn expect_pattern_len(
        &self,
        id: &PatternId,
    ) -> usize {
        self.expect_child_pattern(id).len()
    }
    pub fn expect_child_offset(
        &self,
        loc: &SubLocation,
    ) -> usize {
        pattern_width(
            &self.expect_child_pattern(&loc.pattern_id)[0..loc.sub_index],
        )
    }
    pub(crate) fn find_child_pattern_id(
        &self,
        f: impl FnMut(&(&PatternId, &Pattern)) -> bool,
    ) -> Option<PatternId> {
        self.children.iter().find(f).map(|r| *r.0)
    }
    pub(crate) fn get_child_pattern_mut(
        &mut self,
        id: &PatternId,
    ) -> Result<&mut Pattern, ErrorReason> {
        self.children
            .get_mut(id)
            .ok_or(ErrorReason::NoTokenPatterns)
    }
    #[track_caller]
    pub fn expect_any_child_pattern(&self) -> (&PatternId, &Pattern) {
        self.children.iter().next().unwrap_or_else(|| {
            panic!("Pattern vertex has no children {:#?}", self,)
        })
    }
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
    #[track_caller]
    pub(crate) fn expect_child_pattern_mut(
        &mut self,
        id: &PatternId,
    ) -> &mut Pattern {
        self.get_child_pattern_mut(id).unwrap_or_else(|_| {
            panic!("Token pattern with id {} does not exist in in vertex", id,)
        })
    }
    pub fn child_patterns(&self) -> &ChildPatterns {
        &self.children
    }
    pub fn child_patterns_mut(&mut self) -> &mut ChildPatterns {
        &mut self.children
    }
    pub(crate) fn child_pattern_iter(
        &'_ self
    ) -> impl Iterator<Item = Pattern> + '_ {
        clone_child_patterns(&self.children)
    }
    pub fn child_pattern_set(&self) -> HashSet<Pattern> {
        self.child_pattern_iter().collect()
    }
    pub(crate) fn child_pattern_vec(&self) -> Vec<Pattern> {
        self.child_pattern_iter().collect()
    }

    #[cfg(any(test, feature = "test-api"))]
    fn invalidate_string_cache(&self) {
        if let Ok(mut cache) = self.cached_string.write() {
            *cache = None;
        }
    }

    pub(crate) fn add_pattern_no_update(
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
    #[track_caller]
    pub(crate) fn validate_links(&self) {
        assert!(self.children.len() != 1 || self.parents.len() != 1);
    }
    #[track_caller]
    pub(crate) fn validate_patterns(&self) {
        self.children.iter().fold(
            Vec::new(),
            |mut acc: Vec<Vec<usize>>, (pid, p)| {
                let mut offset = 0;
                assert!(!p.is_empty(), "Empty pattern in index {:#?}", self.index);
                let pattern_width = pattern_width(p);
                assert_eq!(pattern_width, self.width, "Pattern width mismatch in index {:#?} token pattern:\n {:#?}", self.index, (pid, self.children.get(pid)));
                let mut p = p.iter().fold(Vec::new(), |mut pa, c| {
                    offset += c.width();
                    assert!(
                        !acc.iter().any(|pr| pr.contains(&offset)),
                        "Duplicate border in index {:#?} token patterns:\n {:#?}",
                        self.index,
                        self.children
                    );
                    pa.push(offset);
                    pa
                });
                p.pop().unwrap();
                assert!(!p.is_empty(), "Single index pattern in index {:#?}:\n {:#?}", self.index, (pid, self.children.get(pid)));
                acc.push(p);
                acc
            },
        );
    }
    #[track_caller]
    pub(crate) fn validate(&self) {
        //self.validate_links();
        if !self.children.is_empty() {
            self.validate_patterns();
        }
    }
    pub(crate) fn add_parent(
        &mut self,
        loc: ChildLocation,
    ) {
        if let Some(parent) = self.parents.get_mut(&loc.parent.vertex_index()) {
            parent.add_pattern_index(loc.pattern_id, loc.sub_index);
        } else {
            let mut parent_rel = Parent::new(loc.parent.width());
            parent_rel.add_pattern_index(loc.pattern_id, loc.sub_index);
            self.parents.insert(loc.parent.vertex_index(), parent_rel);
        }
        // not while indexing
        //self.validate_links();
    }
    pub(crate) fn remove_parent(
        &mut self,
        vertex: impl HasVertexIndex,
    ) {
        self.parents.remove(&vertex.vertex_index());
        // not while indexing
        //self.validate_links();
    }
    pub(crate) fn remove_parent_index(
        &mut self,
        vertex: impl HasVertexIndex,
        pattern: PatternId,
        index: usize,
    ) {
        if let Some(parent) = self.parents.get_mut(&vertex.vertex_index()) {
            if parent.pattern_indices.len() > 1 {
                parent.remove_pattern_index(pattern, index);
            } else {
                self.parents.remove(&vertex.vertex_index());
            }
        }
        // not while indexing
        //self.validate_links();
    }
    pub(crate) fn get_parents_below_width(
        &self,
        width_ceiling: Option<usize>,
    ) -> impl Iterator<Item = (&VertexIndex, &Parent)> + Clone {
        let parents = self.parents();
        // optionally filter parents by width
        if let Some(ceil) = width_ceiling {
            Either::Left(
                parents
                    .iter()
                    .filter(move |(_, parent)| parent.get_width() < ceil),
            )
        } else {
            Either::Right(parents.iter())
        }
    }
    pub(crate) fn to_pattern_strings<G: GraphKind>(
        &self,
        g: &Hypergraph<G>,
    ) -> Vec<Vec<String>>
    where
        G::Atom: Display,
    {
        self.child_pattern_iter()
            .map(|pat| {
                pat.iter()
                    .map(|c| g.index_string(c.index))
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>()
    }
    //pub(crate) fn get_parents_at_prefix(&self) -> HashMap<VertexIndex, PatternId> {
    //    self.get_parents_with_index_at(0)
    //}
    //pub(crate) fn get_parents_at_postfix(
    //    &self,
    //    graph: &crate::graph::Hypergraph,
    //) -> HashMap<VertexIndex, PatternId> {
    //    self.parents
    //        .iter()
    //        .filter_map(|(id, parent)| {
    //            parent
    //                .get_index_at_postfix_of(graph.expect_vertex(id))
    //                .map(|pat| (*id, pat.pattern_id))
    //        })
    //        .collect()
    //}
    //pub(crate) fn get_parents_with_index_at(
    //    &self,
    //    offset: usize,
    //) -> HashMap<VertexIndex, PatternId> {
    //    self.parents
    //        .iter()
    //        .filter_map(|(id, parent)| {
    //            parent
    //                .get_index_at_pos(offset)
    //                .map(|pat| (*id, pat.pattern_id))
    //        })
    //        .collect()
    //}
    //pub(crate) fn filter_parent_to(
    //    &self,
    //    parent: impl HasVertexIndex,
    //    cond: impl Fn(&&Parent) -> bool,
    //) -> Result<&'_ Parent, ErrorReason> {
    //    let index = parent.vertex_index();
    //    self.get_parent(index)
    //        .ok()
    //        .filter(cond)
    //        .ok_or(ErrorReason::ErrorReasoningParent(index))
    //}
    //pub(crate) fn get_parent_to_ending_at(
    //    &self,
    //    parent_key: impl HasVertexKey,
    //    offset: usize,
    //) -> Result<&'_ Parent, ErrorReason> {
    //    self.filter_parent_to(parent_key, |parent| {
    //        offset
    //            .checked_sub(self.width)
    //            .map(|p| parent.exists_at_pos(p))
    //            .unwrap_or(false)
    //    })
    //}
    pub(crate) fn get_parent_to_starting_at(
        &self,
        parent_index: impl HasVertexIndex,
        index_offset: usize,
    ) -> Result<PatternIndex, ErrorReason> {
        let index = parent_index.vertex_index();
        self.get_parent(index)
            .ok()
            .and_then(|parent| parent.get_index_at_pos(index_offset))
            .ok_or(ErrorReason::ErrorReasoningParent(index))
    }
    pub(crate) fn get_parent_at_prefix_of(
        &self,
        index: impl HasVertexIndex,
    ) -> Result<PatternIndex, ErrorReason> {
        self.get_parent_to_starting_at(index, 0)
    }
    pub(crate) fn get_parent_at_postfix_of(
        &self,
        vertex: &VertexData,
    ) -> Result<PatternIndex, ErrorReason> {
        self.get_parent(vertex.vertex_index())
            .ok()
            .and_then(|parent| parent.get_index_at_postfix_of(vertex))
            .ok_or(ErrorReason::ErrorReasoningParent(vertex.vertex_index()))
    }
    //pub(crate) fn find_ancestor_with_range(
    //    &self,
    //    half: Pattern,
    //    range: impl PatternRangeIndex,
    //) -> Result<PatternId, ErrorReason> {
    //    self.tokens
    //        .iter()
    //        .find_map(|(id, pat)| {
    //            if pat[range.clone()] == half[..] {
    //                Some(*id)
    //            } else {
    //                None
    //            }
    //        })
    //        .ok_or(ErrorReason::NoTokenPatterns)
    //}
    pub(crate) fn largest_postfix(&self) -> (PatternId, Token) {
        let (id, c) = self
            .children
            .iter()
            .fold(None, |acc: Option<(&PatternId, &Token)>, (pid, p)| {
                if let Some(acc) = acc {
                    let c = p.last().unwrap();
                    if c.width() > acc.1.width() {
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
    pub(crate) fn all_children_iter(&self) -> impl IntoIterator<Item = &Token> {
        self.children.iter().flat_map(|(_, pat)| pat.iter())
    }
    pub(crate) fn all_localized_children_iter(
        &self
    ) -> impl IntoIterator<Item = (ChildLocation, &Token)> {
        localized_children_iter_for_index(self.to_child(), &self.children)
    }
    pub(crate) fn top_down_containment_nodes(&self) -> Vec<(usize, Token)> {
        self.children
            .iter()
            .flat_map(|(_, pat)| {
                pat.iter()
                    .enumerate()
                    .filter(|(_, c)| c.width() + 1 == self.width())
                    .map(|(off, c)| (off, *c))
            })
            .sorted_by_key(|&(off, _)| off)
            .collect_vec()
    }
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
    pub fn prefix_children<G: HasGraph>(&self) -> Vec<SubToken> {
        self.selected_children(|_, pattern| {
            Some(TravDir::<G>::head_index(pattern))
        })
    }
    pub fn postfix_children<G: HasGraph>(&self) -> Vec<SubToken>
    where
        <<G::Kind as GraphKind>::Direction as Direction>::Opposite:
            PatternDirection,
    {
        self.selected_children(|_, pattern| {
            Some(TravDir::<G>::last_index(pattern))
        })
    }
}

impl std::fmt::Display for VertexData {
    fn fmt(
        &self,
        f: &mut std::fmt::Formatter<'_>,
    ) -> std::fmt::Result {
        self.fmt_indented(f, 0)
    }
}

impl CompactFormat for VertexData {
    fn fmt_compact(
        &self,
        f: &mut std::fmt::Formatter,
    ) -> std::fmt::Result {
        write!(
            f,
            "Vertex({}, w:{}, {}p, {}c)",
            self.index,
            self.width,
            self.parents.len(),
            self.children.len()
        )
    }

    fn fmt_indented(
        &self,
        f: &mut std::fmt::Formatter,
        indent: usize,
    ) -> std::fmt::Result {
        writeln!(f)?;
        write_indent(f, indent)?;
        writeln!(f, "Vertex {} {{", self.index)?;

        write_indent(f, indent + 1)?;
        writeln!(f, "width: {},", self.width)?;

        write_indent(f, indent + 1)?;
        writeln!(f, "parents: {} entries,", self.parents.len())?;

        write_indent(f, indent + 1)?;
        writeln!(f, "children: {} patterns", self.children.len())?;

        write_indent(f, indent)?;
        write!(f, "}}")
    }
}
