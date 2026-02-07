pub(crate) mod node;
pub(crate) mod output;
pub(crate) mod pattern;
pub(crate) mod position;

use std::{
    borrow::Borrow,
    num::NonZeroUsize,
};

use crate::split::cache::position::SplitPositionCache;
use crate::*;

use crate::split::{
    position_splits,
    vertex::{
        output::{
            NodeSplitOutput,
            NodeType,
            RootMode,
        },
        position::{
            Offset,
            SubSplitLocation,
        },
    },
};
use context_trace::*;
use derive_more::derive::Deref;
use derive_new::new;
use itertools::Itertools;

#[derive(Debug, Clone, Copy)]
pub(crate) struct PosSplitCtx<'a> {
    pub(crate) pos: &'a NonZeroUsize,
    pub(crate) split: &'a SplitPositionCache,
}

impl ToVertexSplits for PosSplitCtx<'_> {
    fn to_vertex_splits(self) -> VertexSplits {
        VertexSplits {
            pos: *self.pos,
            splits: self.split.pattern_splits.clone(),
        }
    }
}

impl<'a, N: Borrow<(&'a NonZeroUsize, &'a SplitPositionCache)>> From<N>
    for PosSplitCtx<'a>
{
    fn from(item: N) -> Self {
        let (pos, split) = item.borrow();
        Self { pos, split }
    }
}
#[derive(Debug, Clone)]
pub(crate) struct VertexSplits {
    pub(crate) pos: NonZeroUsize,
    pub(crate) splits: TokenTracePositions,
}

pub(crate) type TokenTracePositions = HashMap<PatternId, TokenTracePos>;

pub(crate) trait ToVertexSplits: Clone {
    fn to_vertex_splits(self) -> VertexSplits;
}

impl ToVertexSplits for VertexSplits {
    fn to_vertex_splits(self) -> VertexSplits {
        self
    }
}

impl ToVertexSplits for &VertexSplits {
    fn to_vertex_splits(self) -> VertexSplits {
        self.clone()
    }
}

impl<N: Borrow<NonZeroUsize> + Clone, S: Borrow<SplitPositionCache> + Clone>
    ToVertexSplits for (N, S)
{
    fn to_vertex_splits(self) -> VertexSplits {
        VertexSplits::from(self)
    }
}
impl<N: Borrow<NonZeroUsize>, S: Borrow<SplitPositionCache>> From<(N, S)>
    for VertexSplits
{
    fn from(item: (N, S)) -> VertexSplits {
        VertexSplits {
            pos: *item.0.borrow(),
            splits: item.1.borrow().pattern_splits.clone(),
        }
    }
}

pub(crate) trait ToVertexSplitPos {
    fn to_vertex_split_pos(self) -> TokenTracePositions;
}

impl ToVertexSplitPos for TokenTracePositions {
    fn to_vertex_split_pos(self) -> TokenTracePositions {
        self
    }
}

impl ToVertexSplitPos for Vec<SubSplitLocation> {
    fn to_vertex_split_pos(self) -> TokenTracePositions {
        self.into_iter()
            .map(|loc| {
                (
                    loc.location.pattern_id(),
                    TokenTracePos::new(
                        loc.inner_offset(),
                        loc.location.sub_index(),
                    ),
                )
            })
            .collect()
    }
}

impl ToVertexSplitPos for VertexSplits {
    fn to_vertex_split_pos(self) -> TokenTracePositions {
        self.splits
    }
}

#[derive(Debug, Copy, Clone, Deref, new)]
pub(crate) struct VertexSplitCtx<'a> {
    pub(crate) cache: &'a VertexCache,
}
impl VertexSplitCtx<'_> {
    pub(crate) fn bottom_up_splits<N: NodeType>(
        &self,
        node: &VertexData,
        output: &mut N::GlobalSplitOutput,
    ) -> bool {
        let mut front = false;
        // uses inner width of sub split position to calculate node offset
        for (inner_width, pos_cache) in self.bottom_up.iter() {
            // bottom up incoming edge
            for location in pos_cache.bottom().values() {
                // pattern location
                let token = node.expect_child_at(location);

                let inner_offset = Offset::new(*token.width() - **inner_width);
                let outer_offset = *node.expect_child_offset(location);
                if let Some(node_offset) = inner_offset
                    .and_then(|o| o.checked_add(outer_offset))
                    .or(NonZeroUsize::new(outer_offset))
                {
                    let split_loc =
                        SubSplitLocation::new(*location, inner_offset);
                    output
                        .splits_mut()
                        .entry(node_offset)
                        .and_modify(|e: &mut Vec<_>| e.push(split_loc.clone()))
                        .or_insert_with(|| vec![split_loc]);
                    front = true;
                } else {
                    break;
                }
            }
        }
        front
    }
    pub(crate) fn top_down_splits<N: NodeType>(
        &self,
        end_pos: AtomPosition,
        node: &VertexData,
        output: &mut N::GlobalSplitOutput,
    ) -> bool {
        let mut back = false;
        // uses end pos of sub split position to calculate node offset
        for (outer_offset, pos_cache) in self.top_down.iter() {
            // outer offset:
            let inner_offset = Offset::new(*(end_pos - *outer_offset)).unwrap();
            for location in pos_cache.bottom().values() {
                let token = node.expect_child_at(location);
                let inner_offset =
                    Offset::new(inner_offset.get() % *token.width());
                let location = SubLocation::new(
                    location.pattern_id(),
                    location.sub_index() + inner_offset.is_none() as usize,
                );

                let offset = *node.expect_child_offset(&location);
                let parent_offset = inner_offset
                    .map(|o| o.checked_add(offset).unwrap())
                    .unwrap_or_else(|| NonZeroUsize::new(offset).unwrap());

                if parent_offset.get() < *node.width() {
                    let bottom = SubSplitLocation::new(location, inner_offset);
                    if let Some(e) = output.splits_mut().get_mut(&parent_offset)
                    {
                        e.push(bottom)
                    } else {
                        output.splits_mut().insert(parent_offset, vec![bottom]);
                    }
                    back = true;
                }
            }
        }
        back
    }
    pub(crate) fn global_splits<N: NodeType>(
        &self,
        end_pos: AtomPosition,
        node: &VertexData,
    ) -> N::GlobalSplitOutput {
        let mut output = N::GlobalSplitOutput::default();
        let front = self.bottom_up_splits::<N>(node, &mut output);
        let back = self.top_down_splits::<N>(end_pos, node, &mut output);
        match (front, back) {
            (true, true) => output.set_root_mode(RootMode::Infix),
            (false, true) => output.set_root_mode(RootMode::Prefix),
            (true, false) => output.set_root_mode(RootMode::Postfix),
            (false, false) => { /* No splits found, return default output */ }
        }
        output
    }
    pub(crate) fn complete_splits<G: HasGraph, N: NodeType>(
        &self,
        trav: &G,
        end_pos: AtomPosition,
    ) -> N::CompleteSplitOutput {
        let graph = trav.graph();

        let node = graph.expect_vertex_data(self.index);

        let output = self.global_splits::<N>(end_pos, &node);

        N::map(output, |global_splits| {
            global_splits
                .into_iter()
                .map(|(parent_offset, mut locs)| {
                    if locs.len() < node.child_patterns().len() {
                        let pids: HashSet<_> = locs
                            .iter()
                            .map(|l| l.location.pattern_id())
                            .collect();
                        let missing = node
                            .child_patterns()
                            .iter()
                            .filter(|(pid, _)| !pids.contains(pid))
                            .collect_vec();
                        let new_splits =
                            position_splits(missing, parent_offset).splits;
                        locs.extend(new_splits.into_iter().map(|(pid, loc)| {
                            SubSplitLocation::new(
                                SubLocation::new(pid, loc.sub_index()),
                                loc.inner_offset(),
                            )
                        }))
                    }
                    (
                        parent_offset,
                        locs.into_iter()
                            .map(|sub| {
                                if sub.inner_offset().is_some()
                                    || node.child_patterns()
                                        [&sub.location.pattern_id()]
                                        .len()
                                        > 2
                                {
                                    // can't be clean
                                    Ok(sub)
                                } else {
                                    // must be clean
                                    Err(sub.location)
                                }
                            })
                            .collect(),
                    )
                })
                .collect()
        })
    }
}
