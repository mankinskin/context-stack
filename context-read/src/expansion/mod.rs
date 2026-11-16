pub(crate) mod block;
pub(crate) mod closure;

use crate::pipeline::root::{
    CommitReceipt,
    RootManager,
    Span,
};
use context_insert::*;
use context_search::Find;
use context_trace::*;

/// Resolves one consuming step against the latest committed graph.
///
/// The committed root is the only overlap anchor. Each yielded span must be
/// committed and observed before another step is requested.
#[derive(Debug)]
pub(crate) struct ExpansionCtx {
    pub(crate) graph: HypergraphRef,
    /// Known atoms of the current block, not the entire input.
    atoms: Vec<Token>,
    /// Number of block atoms consumed, also the committed seam width after commit.
    cursor: usize,
    /// Latest observed root, covering committed input including earlier blocks.
    root: Option<Token>,
}

impl ExpansionCtx {
    /// Start an unconsumed known-atom block with no committed block seam.
    pub(crate) fn new(
        graph: HypergraphRef,
        atoms: Vec<Token>,
    ) -> Self {
        Self {
            graph,
            atoms,
            cursor: 0,
            root: None,
        }
    }

    /// Record the sink's root after committing the previously yielded span.
    pub(crate) fn observe_commit(
        &mut self,
        receipt: CommitReceipt,
    ) {
        self.root = Some(receipt.root);
    }

    /// Resolve one extending root overlap, or consume the largest forward match.
    ///
    /// Returns `None` when the block is exhausted. Candidate probes are read-only,
    /// but accepted-match materialization and closure may mutate the graph.
    /// The caller must commit the returned span and pass its receipt back.
    ///
    /// # Panics
    ///
    /// Panics on stale root feedback, unexpected search errors, or violated
    /// occurrence, boundary, or width invariants. Mutations are not rolled back.
    pub(crate) fn next(
        &mut self,
        sink: &RootManager,
    ) -> Option<Span> {
        if self.cursor == self.atoms.len() {
            return None;
        }
        if let Some(root) = self.root {
            assert_eq!(sink.root, Some(root), "expansion root is stale");
            if let Some((postfix, path, expansion)) = self.expand_root(root) {
                let consumed = expansion.width().0 - postfix.width().0;
                let plan = closure::prepare(
                    &self.graph,
                    sink,
                    root,
                    postfix,
                    &path,
                    expansion,
                );
                let span = plan.apply(&self.graph);
                assert_eq!(
                    sink.root,
                    Some(root),
                    "closure changed the accumulation root"
                );
                if matches!(span, Span::ReplaceTail { .. }) {
                    assert!(
                        sink.is_editable(root),
                        "closure made the edited root non-editable"
                    );
                }
                self.cursor += consumed;
                return Some(span);
            }
        }
        let token = self.match_forward();
        self.cursor += token.width().0;
        if let Some(previous) = sink.root {
            if let Some(root) = closure::find_complete(
                &self.graph,
                &Pattern::from(vec![previous, token]),
            ) {
                return Some(Span::ReplaceRoot {
                    previous,
                    token: root,
                });
            }
        }
        Some(Span::Append(token))
    }

    /// Materialize the largest matching prefix of the unconsumed block.
    fn match_forward(&self) -> Token {
        let remaining = &self.atoms[self.cursor..];
        let token = if remaining.len() == 1 {
            remaining[0]
        } else {
            match ToInsertCtx::<IndexWithPath>::insert_next_match(
                &self.graph,
                remaining.to_vec(),
            ) {
                Ok(outcome) => outcome.token(),
                Err(ErrorReason::SingleIndex(index)) => index.index,
                Err(error) => {
                    panic!("block expansion search failed: {:?}", error)
                }
            }
        };
        assert!(
            token.width().0 > 0 && token.width().0 <= remaining.len(),
            "block expansion returned an invalid token width: {:?}",
            token
        );
        token
    }

    /// Find the first extending postfix, largest-first, within the block seam.
    ///
    /// Return the shared token, its reconnected root path, and the materialized
    /// expansion. Rejected probes do not mutate the graph.
    fn expand_root(
        &self,
        root: Token,
    ) -> Option<(Token, Vec<ChildLocation>, Token)> {
        if self.cursor == 0 {
            return None;
        }
        let mut path = Vec::new();
        for (location, postfix) in root.postfix_iter(self.graph.clone()) {
            path.push(location);
            if postfix.width().0 > self.cursor {
                continue;
            }
            let mut query = vec![postfix];
            query.extend_from_slice(&self.atoms[self.cursor..]);
            let matched = match self.graph.find_ancestor(query.clone()) {
                Ok(response) => usize::from(response.checkpoint_position()),
                Err(
                    ErrorReason::SingleIndex(_)
                    | ErrorReason::NoParents
                    | ErrorReason::NotFound,
                ) => continue,
                Err(error) => panic!("root postfix search failed: {:?}", error),
            };
            if matched <= postfix.width().0 {
                continue;
            }
            assert!(
                matched <= postfix.width().0 + self.atoms.len() - self.cursor,
                "root expansion extends beyond the block"
            );
            closure::validate_path(&self.graph, root, postfix, &path, false);
            let expansion =
                match ToInsertCtx::<IndexWithPath>::insert_next_match(
                    &self.graph,
                    query,
                ) {
                    Ok(outcome) => outcome.token(),
                    Err(error) => panic!(
                        "root expansion materialization failed: {:?}",
                        error
                    ),
                };
            assert_eq!(
                expansion.width().0,
                matched,
                "materialization changed the accepted match"
            );
            // Splitting the accepted match can rewrite the anchor's decomposition.
            let path =
                closure::boundary_path(&self.graph, root, postfix, false)
                    .expect(
                    "materialized root must still represent the shared postfix",
                );
            return Some((postfix, path, expansion));
        }
        None
    }
}
