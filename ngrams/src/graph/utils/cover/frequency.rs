//! Select labelled ancestors that contribute distinct occurrences of a candidate.

use std::collections::VecDeque;

use derive_more::{
    Deref,
    DerefMut,
    From,
};
use itertools::Itertools;

use crate::graph::{
    containment::TextLocation,
    labelling::LabellingCtx,
    traversal::{
        direction::{
            BottomUp,
            TopDown,
            TraversalDirection,
        },
        pass::RunResult,
    },
    vocabulary::{
        entry::{
            HasVertexEntries,
            VertexCtx,
        },
        NGramId,
        ProcessStatus,
        Vocabulary,
    },
};
use context_trace::{
    graph::vertex::{
        has_vertex_index::HasVertexIndex,
        has_vertex_key::HasVertexKey,
        location::child::{
            ChildLocation,
            HasSubIndex,
        },
        parent::HasPatternId,
        pattern::Pattern,
        token::Token,
        wide::Wide,
        VertexIndex,
    },
    HashSet,
};

/// Labelled containing strings encountered while covering candidate occurrences.
///
/// This stores ancestor identities, not positional child spans. Construction
/// tracks occurrence positions separately to avoid counting the same position
/// twice when reached through multiple containment paths.
#[derive(Debug, Default, Clone, Deref)]
pub(crate) struct FrequencyCover {
    /// Ancestors that contributed at least one previously uncovered position.
    #[deref]
    cover: HashSet<NGramId>,
}
impl FrequencyCover {
    /// Search upward breadth-first, retaining labelled ancestors that add new
    /// occurrences.
    ///
    /// Each depth (width difference to the candidate) is processed as one
    /// layer, ordered by `(ancestor content, candidate offset in ancestor)`;
    /// the first ancestor in this order claims shared positions. An ancestor
    /// that contributes positions ends its branch; one that contributes none
    /// (unlabelled or already covered) is expanded. Each state is visited once.
    pub(crate) fn from_entry(
        ctx: &LabellingCtx,
        entry: &VertexCtx,
    ) -> RunResult<Self> {
        let vocab = entry.vocab;
        let mut cover: HashSet<_> = Default::default();
        let mut occ_set: HashSet<_> = Default::default();
        let mut visited: HashSet<(usize, NGramId)> = Default::default();
        let mut layer = Self::next_parent_offsets(entry);
        while !layer.is_empty() {
            let states = layer
                .drain(..)
                .filter(|state| visited.insert(*state))
                .map(|(off, p)| {
                    let pe = vocab.get_vertex(&p).unwrap();
                    (pe.ngram.clone(), off, pe)
                })
                .sorted_by(|(a, ao, _), (b, bo, _)| (a, ao).cmp(&(b, bo)))
                .collect_vec();
            let mut next = Vec::new();
            for (_, off, pe) in states {
                ctx.check_cancelled()?;
                let diff = Self::new_occurrences(ctx, off, &pe, &occ_set);
                if diff.is_empty() {
                    next.extend(
                        Self::next_parent_offsets(&pe)
                            .into_iter()
                            .map(|(o, p)| (o + off, p)),
                    );
                } else {
                    cover.insert(NGramId::new(
                        pe.vertex_key(),
                        pe.data.width().0,
                    ));
                    occ_set.extend(&diff);
                }
            }
            layer = next;
        }
        Ok(Self { cover })
    }
    /// Return each parent occurrence in a pattern with this child's local offset.
    ///
    /// Multiple pattern positions may yield the same parent at different offsets.
    fn next_parent_offsets(entry: &VertexCtx) -> Vec<(usize, NGramId)> {
        entry
            .data
            .parents()
            .iter()
            .flat_map(|(&id, p)| {
                p.pattern_indices().iter().map(move |ploc| {
                    (
                        entry.vocab.containment.expect_child_offset(
                            &ChildLocation::new(
                                Token::new(id, p.width()),
                                ploc.pattern_id(),
                                ploc.sub_index(),
                            ),
                        ),
                        NGramId::new(
                            entry.vocab.containment.expect_key_for_index(id),
                            p.width().0,
                        ),
                    )
                })
            })
            .collect_vec()
    }
    /// Project labelled-parent positions onto the candidate and subtract known ones.
    ///
    /// `offset` is the candidate's character offset within this ancestor.
    /// Unlabelled ancestors contribute no positions.
    pub(crate) fn new_occurrences(
        ctx: &LabellingCtx,
        offset: usize,
        parent_entry: &VertexCtx,
        occ_set: &HashSet<TextLocation>,
    ) -> HashSet<TextLocation> {
        if ctx.labels().contains(&parent_entry.vertex_key()) {
            {
                let occ: HashSet<_> = parent_entry
                    .occurrences
                    .iter()
                    .map(|loc| TextLocation::new(loc.texti, loc.x + offset))
                    .collect();
                occ.difference(occ_set).copied().collect()
            }
        } else {
            Default::default()
        }
    }
}
