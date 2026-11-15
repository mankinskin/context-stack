pub(crate) mod block;
pub(crate) mod chain;

use chain::{
    band::Band,
    link::OverlapLink,
    BandState,
};
use context_insert::*;
use context_trace::*;
use tracing::debug;

/// Resolves non-overlapping spans of a known-atom block.
///
/// Each yielded token is final and must be appended to the accumulation sink
/// before requesting the next span, so subsequent searches see graph updates.
/// Local overlap anchors never come from the accumulation root.
#[derive(Debug)]
pub(crate) struct ExpansionCtx {
    pub(crate) graph: HypergraphRef,
    atoms: Vec<Token>,
    cursor: usize,
}

impl Iterator for ExpansionCtx {
    type Item = Token;

    fn next(&mut self) -> Option<Self::Item> {
        if self.cursor >= self.atoms.len() {
            return None;
        }

        let span_start = self.cursor;
        let remaining = &self.atoms[span_start..];
        let mut anchor = if remaining.len() == 1 {
            remaining[0]
        } else {
            match ToInsertCtx::<IndexWithPath>::insert_next_match(
                &self.graph,
                remaining.to_vec(),
            ) {
                Ok(outcome) => outcome.token(),
                Err(ErrorReason::SingleIndex(index)) => index.index,
                Err(error) => panic!("block expansion search failed: {:?}", error),
            }
        };
        assert!(
            anchor.width().0 > 0 && anchor.width().0 <= remaining.len(),
            "block expansion returned an invalid token width: {:?}",
            anchor
        );
        debug!(anchor = ?anchor, cursor = self.cursor, "ExpansionCtx::next: local match");

        self.cursor += anchor.width().0;
        let mut pending = anchor;

        while anchor.width().0 > 1 && self.cursor < self.atoms.len() {
            let Some((postfix, expansion, next_cursor)) =
                self.find_overlap(anchor)
            else {
                break;
            };

            let state =
                self.build_overlap_state(pending, postfix, expansion);
            pending = state.collapse(&mut self.graph)[0];
            self.cursor = next_cursor;
            anchor = expansion;
        }

        assert_eq!(
            pending.width().0,
            self.cursor - span_start,
            "finalized span must cover exactly the consumed block atoms"
        );
        debug!(token = ?pending, cursor = self.cursor, "ExpansionCtx::next: finalized span");
        Some(pending)
    }
}

impl ExpansionCtx {
    pub(crate) fn new(
        graph: HypergraphRef,
        atoms: Vec<Token>,
    ) -> Self {
        debug!(atoms_len = atoms.len(), "New ExpansionCtx");
        Self {
            graph,
            atoms,
            cursor: 0,
        }
    }

    /// Expand a postfix of the local anchor into the unconsumed block suffix.
    fn find_overlap(
        &self,
        anchor: Token,
    ) -> Option<(Token, Token, usize)> {
        for (_, postfix) in anchor.postfix_iter(self.graph.clone()) {
            let postfix_width = postfix.width().0;
            if postfix_width >= anchor.width().0 {
                continue;
            }
            let mut query = vec![postfix];
            query.extend_from_slice(&self.atoms[self.cursor..]);

            let result = match ToInsertCtx::<IndexWithPath>::insert_next_match(
                &self.graph,
                query,
            ) {
                Ok(outcome) => outcome.token(),
                Err(ErrorReason::SingleIndex(_)) => continue,
                Err(error) => panic!("block overlap search failed: {:?}", error),
            };

            debug!(
                anchor = ?anchor,
                postfix = ?postfix,
                result = ?result,
                cursor = self.cursor,
                "find_overlap: checking local postfix"
            );

            if result.width().0 > postfix_width {
                let next_cursor =
                    self.cursor + result.width().0 - postfix_width;
                assert!(
                    next_cursor <= self.atoms.len(),
                    "overlap extends beyond the known block"
                );
                return Some((postfix, result, next_cursor));
            }
        }
        None
    }

    fn build_overlap_state(
        &self,
        pending: Token,
        postfix: Token,
        expansion: Token,
    ) -> BandState {
        let postfix_locations = self
            .boundary_locations(pending, postfix, false)
            .expect("overlap postfix must be a suffix of the pending span");
        let mut root_postfix = IndexEndPath::from(postfix_locations[0]);
        for location in postfix_locations.into_iter().skip(1) {
            root_postfix.path_append(location);
        }

        let prefix_locations = self
            .boundary_locations(expansion, postfix, true)
            .expect("overlap postfix must be a prefix of its expansion");
        let mut expansion_prefix = IndexStartPath::from(prefix_locations[0]);
        for location in prefix_locations.into_iter().skip(1) {
            expansion_prefix.path_append(location);
        }

        BandState::WithOverlap {
            primary: Band::from(pending),
            overlap: Band::from(expansion),
            link: OverlapLink {
                child_path: root_postfix,
                search_path: expansion_prefix,
                start_bound: self.cursor - postfix.width().0,
            },
        }
    }

    /// Keep a connected path when a token has alternative decompositions.
    fn boundary_locations(
        &self,
        root: Token,
        target: Token,
        prefix: bool,
    ) -> Option<Vec<ChildLocation>> {
        let vertex = root.vertex(&self.graph);
        for (&pattern_id, pattern) in vertex.child_patterns() {
            let sub_index = if prefix { 0 } else { pattern.len() - 1 };
            let child = pattern[sub_index];
            let location = ChildLocation::new(root, pattern_id, sub_index);
            if child == target {
                return Some(vec![location]);
            }
            if child.width().0 > target.width().0 {
                if let Some(mut path) =
                    self.boundary_locations(child, target, prefix)
                {
                    path.insert(0, location);
                    return Some(path);
                }
            }
        }
        None
    }
}
