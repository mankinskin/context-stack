use crate::{
    compare::state::{
        CompareEndResult,
        CompareLeafResult::*,
        CompareState,
    },
    cursor::Candidate,
    SearchKind,
};
use context_trace::{
    path::accessors::has_path::HasRootedPath,
    *,
};

use std::fmt::Debug;

/// A record of one step in the child comparison loop.
///
/// Collected by [`CompareIterator::compare_with_events`] and forwarded to
/// the search-state layer so it can emit graph-op visualization events.
#[derive(Debug, Clone)]
pub(crate) enum CompareEvent {
    /// A child node was visited (token decomposition / prefix expansion).
    /// `parent` was expanded, producing `child` as one of its prefixes.
    VisitChild {
        parent: usize,
        child: usize,
        child_width: usize,
    },
    /// A leaf token comparison succeeded.
    ChildMatch {
        node: usize,
        cursor_pos: usize,
    },
    /// A leaf token comparison failed.
    ChildMismatch {
        node: usize,
        cursor_pos: usize,
        expected: usize,
        actual: usize,
    },
}

/// Result of `compare_with_events`: the final outcome plus intermediate steps.
#[derive(Debug)]
pub(crate) struct CompareOutcome {
    pub(crate) result: CompareEndResult,
    pub(crate) events: Vec<CompareEvent>,
}

#[derive(Debug)]
pub(crate) struct CompareIterator<K: SearchKind> {
    pub(crate) children: ChildIterator<
        K,
        CompareState<Candidate, Candidate, PositionAnnotated<ChildLocation>>,
    >,
}

impl<K: SearchKind> CompareIterator<K> {
    pub(crate) fn new(
        trav: K::Trav,
        queue: impl Into<
            ChildQueue<
                CompareState<
                    Candidate,
                    Candidate,
                    PositionAnnotated<ChildLocation>,
                >,
            >,
        >,
    ) -> Self {
        Self {
            children: ChildIterator::<
                K,
                CompareState<
                    Candidate,
                    Candidate,
                    PositionAnnotated<ChildLocation>,
                >,
            >::new(trav, queue),
        }
    }
    /// Run the comparison loop, collecting [`CompareEvent`]s for every
    /// intermediate child visit and leaf-token comparison.
    pub(crate) fn compare_with_events(mut self) -> CompareOutcome {
        let mut events = Vec::new();

        let result = loop {
            let cs = match self.children.next() {
                Some(cs) => cs,
                None => panic!("CompareIterator exhausted without result"),
            };

            // Extract the tokens being compared *before* consuming `cs`.
            let path_leaf =
                cs.rooted_path().role_rooted_leaf_token::<End, _>(&self.children.trav);
            let query_leaf =
                (*cs.query.current()).role_rooted_leaf_token::<End, _>(&self.children.trav);
            let cursor_pos = *cs.query.current().atom_position.as_ref();

            match cs.compare_leaf_tokens(&self.children.trav) {
                Prefixes(next) => {
                    tracing::debug!(
                        num_prefixes = next.len(),
                        "got Prefixes, extending queue"
                    );
                    // Record a VisitChild for each new prefix child.
                    for prefix_state in next.iter() {
                        let child_leaf = prefix_state
                            .rooted_path()
                            .role_rooted_leaf_token::<End, _>(&self.children.trav);
                        events.push(CompareEvent::VisitChild {
                            parent: path_leaf.index.0,
                            child: child_leaf.index.0,
                            child_width: child_leaf.width.0,
                        });
                    }
                    self.children.queue.extend(next);
                },
                Finished(result) => {
                    match &result {
                        CompareEndResult::FoundMatch(_) => {
                            events.push(CompareEvent::ChildMatch {
                                node: path_leaf.index.0,
                                cursor_pos,
                            });
                        },
                        CompareEndResult::Mismatch(_) => {
                            events.push(CompareEvent::ChildMismatch {
                                node: path_leaf.index.0,
                                cursor_pos,
                                expected: query_leaf.index.0,
                                actual: path_leaf.index.0,
                            });
                        },
                    }
                    break result;
                },
            }
        };

        CompareOutcome { result, events }
    }
}
impl<T: SearchKind> Iterator for CompareIterator<T> {
    type Item = Option<CompareEndResult>;
    fn next(&mut self) -> Option<Self::Item> {
        tracing::trace!(
            queue_len = self.children.queue.len(),
            "processing next state"
        );
        self.children.next().map(|cs| {
            match cs.compare_leaf_tokens(&self.children.trav) {
                Prefixes(next) => {
                    tracing::debug!(
                        num_prefixes = next.len(),
                        "got Prefixes, extending queue"
                    );
                    self.children.queue.extend(next);
                    None
                },
                Finished(result) => {
                    tracing::trace!(
                        result = ?result,
                        "got result (Match/Mismatch)"
                    );
                    Some(result)
                },
            }
        })
    }
}
