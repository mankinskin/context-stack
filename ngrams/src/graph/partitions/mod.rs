//! Build the selected output graph while discovering additional gap-token labels.
//!
//! Selection is not frozen: processing a node may add labels that change later
//! nodes' covers. Already-visited nodes are not rebuilt, making decompositions
//! sensitive to traversal order.

pub(crate) mod collect;
pub(crate) mod container;

use derive_more::{
    Deref,
    DerefMut,
    IntoIterator,
};
use derive_new::new;
use itertools::Itertools;
use std::collections::VecDeque;
use tokio_util::sync::CancellationToken;

use crate::graph::{
    labelling::LabellingCtx,
    partitions::container::PartitionContainer,
    trace,
    traversal::{
        direction::{
            TopDown,
            TraversalDirection,
        },
        pass::{
            RunResult,
            TraversalPass,
        },
        queue::Queue,
    },
    utils::cover::ChildCover,
    vocabulary::{
        entry::{
            HasVertexEntries,
            VertexCtx,
        },
        NGramId,
        ProcessStatus,
    },
};
use context_trace::{
    graph::{
        vertex::{
            data::{
                VertexData,
                VertexDataBuilder,
            },
            has_vertex_index::{
                HasVertexIndex,
                ToToken,
            },
            has_vertex_key::HasVertexKey,
            location::SubLocation,
            pattern::{
                id::PatternId,
                Pattern,
            },
            token::Token,
            wide::Wide,
            ChildPatterns,
            VertexIndex,
        },
        Hypergraph,
    },
    HashMap,
    HashSet,
    VertexSet,
};

use super::{
    traversal::{
        queue::{
            LayeredQueue,
            LinearQueue,
        },
        visited::VisitTracking,
    },
    utils::dedup::ChildDedupPass,
    vocabulary::Vocabulary,
};

/// Read-only partition context scoped to the node whose patterns are being built.
#[derive(Debug, Deref, new)]
pub(crate) struct NodePartitionCtx<'a, 'b> {
    /// Containing string used to resolve positioned gap substrings.
    root: NGramId,
    /// Outer pass, exposed through dereferencing for vocabulary access.
    #[deref]
    ctx: &'a PartitionsCtx<'b>,
}

/// Top-down, FIFO output construction over roots and selected composite children.
#[derive(Debug, Deref, DerefMut)]
pub(crate) struct PartitionsCtx<'b> {
    /// Shared vocabulary and labels, mutated by nested gap discovery.
    #[deref]
    #[deref_mut]
    pub(crate) ctx: &'b mut LabellingCtx,
    /// Nodes already decomposed; later label discoveries do not invalidate them.
    visited: <Self as VisitTracking>::Collection,
    /// Derived graph sharing vocabulary keys but using its own vertex indices.
    pub(crate) graph: Hypergraph,
}

impl<'b> From<&'b mut LabellingCtx> for PartitionsCtx<'b> {
    /// Start a fresh graph and visit set while retaining the current label selection.
    fn from(ctx: &'b mut LabellingCtx) -> Self {
        Self {
            ctx,
            visited: Default::default(),
            graph: Default::default(),
        }
    }
}
impl VisitTracking for PartitionsCtx<'_> {
    type Collection = HashSet<<Self as TraversalPass>::Node>;
    /// Expose the set preventing repeated decomposition of a node.
    fn visited_mut(&mut self) -> &mut <Self as VisitTracking>::Collection {
        &mut self.visited
    }
}
impl TraversalPass for PartitionsCtx<'_> {
    type Node = NGramId;
    type NextNode = NGramId;
    type Queue = LinearQueue<Self>;
    /// Expose the shared context for traversal cancellation checks.
    fn ctx(&self) -> &LabellingCtx {
        self.ctx
    }
    /// Insert root placeholders and queue roots in their hash-set iteration order.
    fn start_queue(&mut self) -> RunResult<Self::Queue> {
        let queue =
            Self::Queue::from_iter(TopDown::starting_nodes(self.vocab()));
        for vk in queue.iter() {
            let data =
                self.vocab().containment.expect_vertex_data(vk.vertex_key());
            let builder =
                VertexDataBuilder::default().width(data.width()).key(**vk);
            // An input may be a single atom; keep its atom data like atom children.
            match self.vocab().containment.get_atom_by_key(&vk.vertex_key()) {
                Some(atom) if vk.width == 1 => {
                    let data = self.graph.finish_vertex_builder(builder);
                    self.graph.insert_atom_data(atom, data);
                },
                _ => {
                    self.graph.insert_vertex_builder(builder);
                },
            }
        }
        self.status.next_pass(
            ProcessStatus::Partitions,
            0,
            self.labels().len() + self.vocab().leaves.len(),
        );
        Ok(queue)
    }
    /// Admit labelled or leaf nodes once, marking them before decomposition.
    fn node_condition(
        &mut self,
        node: Self::Node,
    ) -> bool {
        // Check if node should be processed and mark as visited
        if !self.visited_mut().contains(&node) {
            if self.labels().contains(&node)
                || self.vocab().leaves.contains(&node)
            {
                self.visited_mut().insert(node);
                return true;
            }
        }
        false
    }
    /// Build patterns, discover gap labels, and schedule composite child nodes.
    ///
    /// Patterns are chosen before deduplication expands the label set. They are
    /// not recomputed here after that expansion. Atom children are inserted but
    /// normally not queued for this method.
    fn on_node(
        &mut self,
        node: &NGramId,
    ) -> RunResult<Option<Vec<Self::NextNode>>> {
        *self.status.steps_mut() += 1;
        if node.width == 1 {
            // Atoms (reachable here only as roots) have no decomposition.
            return Ok(None);
        }
        let entry = self.vocab().get_vertex(node).unwrap();
        let _span = trace::partition_span(&entry.ngram).entered();
        let container = PartitionContainer::from_ngram(self, *node);
        trace::partition_patterns(self.vocab(), &entry.ngram, &container);

        let pids: Vec<_> = std::iter::repeat_n((), container.len())
            .map(|_| PatternId::default())
            .collect();

        // Build child patterns and get child locations in one mutation
        let child_locations = self
            .graph
            .with_vertex_mut(node.vertex_key(), |parent_data| {
                // child patterns with indices in containment
                *parent_data.child_patterns_mut() = pids
                    .into_iter()
                    .zip(container)
                    .map(|(pid, tokens)| (pid, Pattern::from(tokens)))
                    .collect();

                // child locations parent in self.graph, children indices in self.vocab.containment
                parent_data
                    .all_localized_children_iter()
                    .into_iter()
                    .map(|(l, c)| (l, *c))
                    .collect_vec()
            })
            .unwrap();

        let unlabelled = child_locations
            .iter()
            .map(|(_, vi)| self.vocab().containment.expect_key_for_index(*vi))
            .filter(|k| !self.labels().contains(k))
            .collect_vec();

        // This nested pass mutates global labels; its result is currently ignored.
        ChildDedupPass::new(self.ctx, unlabelled).run();

        // create child nodes in self.graph
        // set child parents and translate child indices to self.graph
        for (loc, vi) in child_locations.iter().copied() {
            let key = self.vocab().containment.expect_key_for_index(vi);
            let out_index = if self.graph.contains_vertex(key) {
                self.graph
                    .with_vertex_mut(key, |v| {
                        v.add_parent(loc);
                        v.vertex_index()
                    })
                    .unwrap()
            } else {
                let builder =
                    VertexDataBuilder::default().width(vi.width()).key(key);
                let mut data = self.graph.finish_vertex_builder(builder);
                data.add_parent(loc);

                // For width=1 vertices (atoms), copy the atom data from containment graph
                if vi.width() == 1 {
                    if let Some(atom) =
                        self.vocab().containment.get_atom_by_key(&key)
                    {
                        self.graph.insert_atom_data(atom, data).vertex_index()
                    } else {
                        self.graph.insert_vertex_data(data).vertex_index()
                    }
                } else {
                    self.graph.insert_vertex_data(data).vertex_index()
                }
            };
            // Update the child token in the parent node
            let sub_loc = SubLocation::from(loc);
            self.graph
                .with_vertex_mut(node.vertex_key(), |parent_data| {
                    parent_data.expect_child_mut_at(&sub_loc).index = out_index;
                })
                .unwrap();
        }
        // Resolve through containment: child_locations still holds its indices,
        // even though the stored output patterns have now been translated.
        let next = child_locations
            .clone()
            .into_iter()
            .flat_map(|(_, p)| p)
            .filter(|c| c.width() > 1)
            .map(|c| {
                NGramId::new(
                    self.vocab().get_vertex(&c).unwrap().data.vertex_key(),
                    c.width().0,
                )
            })
            .collect();
        //let next = vec![];
        Ok(Some(next))
    }
    /// Announce partition construction on stderr.
    fn begin_run(&mut self) {
        eprintln!("Partition Pass");
    }

    /// Resolve root strings and mark construction finished, without a fixed-point rerun.
    fn finish_run(&mut self) -> RunResult<()> {
        self.vocab().roots.iter().for_each(|key| {
            let _ = self.graph.vertex_key_string(key);
        });
        //println!("{:#?}", &self.graph);
        *self.status.pass_mut() = ProcessStatus::Finished;
        Ok(())
    }
}
