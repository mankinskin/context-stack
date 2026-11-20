//! Position-based local navigation for positioned search (search plan 2.1).
//!
//! Replaces the row model's "next sibling" and "prefix children": a token
//! starting at a position is found by [`PositionedGraph::start_at`], retained
//! prefixes by the [`PositionedGraph::prefix_child`] chain. No content is
//! read; only compositions, parent links and the atom index are used.

use super::{
    composition::Occurrence,
    graph::PositionedGraph,
    location::OccurrenceLocation,
};
use crate::{
    AtomOf,
    AtomPosition,
    GraphKind,
    Token,
    graph::vertex::{
        VertexIndex,
        has_vertex_index::HasVertexIndex,
    },
};

impl<G: GraphKind> PositionedGraph<G> {
    fn composition_of(
        &self,
        vertex: VertexIndex,
    ) -> Option<&super::Composition> {
        self.vertex(vertex)?.composition()
    }

    /// The largest retained proper descendant of `root` starting at `p`, and
    /// the occurrence steps from `root` down to it (offsets relative to each
    /// parent, summing to `p`).
    ///
    /// Descends through the covering occurrence (last with `offset ≤ p`) until
    /// one starts exactly at `p`. `None` iff `p ≥ width(root)`, `root` is an
    /// atom (it has no proper descendants) or unknown.
    pub fn start_at(
        &self,
        root: impl HasVertexIndex,
        p: impl Into<AtomPosition>,
    ) -> Option<(Token, Vec<OccurrenceLocation>)> {
        self.descend(root.vertex_index(), p.into(), Vec::new())
    }

    /// Continue `steps` with [`Self::start_at`]`(parent, p)`.
    fn descend(
        &self,
        mut parent: VertexIndex,
        mut p: AtomPosition,
        mut steps: Vec<OccurrenceLocation>,
    ) -> Option<(Token, Vec<OccurrenceLocation>)> {
        loop {
            let occurrence = *self.composition_of(parent)?.covering(p)?;
            steps.push(OccurrenceLocation {
                parent,
                offset: occurrence.offset,
                child: occurrence.child.index,
            });
            if occurrence.offset == p {
                return Some((occurrence.child, steps));
            }
            p = AtomPosition(p.0 - occurrence.offset.0);
            parent = occurrence.child.index;
        }
    }

    /// The unique occurrence at offset 0 of `vertex`; `None` for atoms and
    /// unknown vertices. Every retained proper prefix of `vertex` lies on the
    /// chain `prefix_child(v)`, `prefix_child(prefix_child(v))`, ….
    pub fn prefix_child(
        &self,
        vertex: impl HasVertexIndex,
    ) -> Option<Occurrence> {
        Some(*self.composition_of(vertex.vertex_index())?.prefix())
    }

    /// Every occurrence of `vertex` in a parent as `(parent, offset)`, ordered
    /// by parent width, parent index, offset. Empty for unknown vertices.
    pub fn parent_occurrences(
        &self,
        vertex: impl HasVertexIndex,
    ) -> impl Iterator<Item = (Token, AtomPosition)> {
        let mut occurrences: Vec<(Token, AtomPosition)> = self
            .vertex(vertex.vertex_index())
            .into_iter()
            .flat_map(|v| v.parents())
            .flat_map(|(&parent, offsets)| {
                let parent = self.vertices[parent.0].token();
                offsets.iter().map(move |&offset| (parent, offset))
            })
            .collect();
        occurrences.sort_by_key(|(parent, offset)| (parent.width, parent.index, *offset));
        occurrences.into_iter()
    }

    /// The atom vertex of `atom`, if any (read-only lookup in the atom index).
    pub fn atom_token(
        &self,
        atom: &AtomOf<G>,
    ) -> Option<Token> {
        self.atoms.get(atom).map(|&index| Token::new(index, 1))
    }

    /// Move an end path inside `root` to position `p`: the result equals
    /// [`Self::start_at`]`(root, p)`, recomputing only the changed tail.
    ///
    /// `steps` is a path from `root` (as returned by `start_at` for an
    /// earlier position). Corrected pop rule (search plan, "End-path
    /// update"): a step `(P, o, C)` with absolute child offset `a` is kept iff
    /// `C` covers `p`, no later occurrence of `P` starts at or before `p` and
    /// `a < p`. The path is truncated at the first step that fails and
    /// continued with `start_at(P, p − offset(P))`; if every step is kept, the
    /// descent continues from the last child.
    pub fn advance_end_path(
        &self,
        root: impl HasVertexIndex,
        mut steps: Vec<OccurrenceLocation>,
        p: impl Into<AtomPosition>,
    ) -> Option<(Token, Vec<OccurrenceLocation>)> {
        let root = root.vertex_index();
        let p = p.into();
        debug_assert!(steps.first().is_none_or(|s| s.parent == root));
        // Absolute offset of the current parent in `root`.
        let mut base = 0;
        let mut parent = root;
        let mut kept = 0;
        for step in &steps {
            let composition = self.composition_of(step.parent)?;
            let occurrences = composition.occurrences();
            let index = occurrences.partition_point(|o| o.offset < step.offset);
            let occurrence = occurrences.get(index)?;
            debug_assert_eq!(occurrence.offset, step.offset);
            debug_assert_eq!(occurrence.child.index, step.child);
            let start = base + occurrence.offset.0;
            let covers = start <= p.0 && p.0 < start + occurrence.child.width.0;
            let last = occurrences
                .get(index + 1)
                .is_none_or(|next| base + next.offset.0 > p.0);
            if !(covers && last && start < p.0) {
                break;
            }
            base = start;
            parent = step.child;
            kept += 1;
        }
        steps.truncate(kept);
        self.descend(parent, AtomPosition(p.0 - base), steps)
    }
}
