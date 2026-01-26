use std::{
    borrow::{
        Borrow,
        BorrowMut,
    },
    fmt::Debug,
    iter::FromIterator,
    num::NonZeroUsize,
};

use crate::*;
use context_trace::*;

#[derive(Clone, Debug, Copy, Hash, Eq, PartialEq)]
pub struct PosKey {
    pub index: Token,
    pub pos: NonZeroUsize,
}

impl PosKey {
    pub fn new<P: TryInto<NonZeroUsize>>(
        index: Token,
        pos: P,
    ) -> Self
    where
        P::Error: Debug,
    {
        Self {
            index,
            pos: pos.try_into().unwrap(),
        }
    }
}

impl From<Token> for PosKey {
    fn from(index: Token) -> Self {
        Self {
            index,
            pos: NonZeroUsize::new(*index.width()).unwrap(),
        }
    }
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct SplitPositionCache {
    pub top: HashSet<PosKey>,
    pub pattern_splits: TokenTracePositions,
}

impl std::ops::Sub<PatternSubDeltas> for SplitPositionCache {
    type Output = Self;
    fn sub(
        mut self,
        rhs: PatternSubDeltas,
    ) -> Self::Output {
        self -= &rhs;
        self
    }
}

impl std::ops::SubAssign<&PatternSubDeltas> for SplitPositionCache {
    fn sub_assign(&mut self, rhs: &PatternSubDeltas) {
        self.pattern_splits
            .iter_mut()
            .for_each(|(pid, pos)| {
                if let Some(&delta) = rhs.get(pid) {
                    let sub_index = pos.sub_index();
                    tracing::debug!(
                        ?pid,
                        sub_index,
                        delta,
                        inner_offset = ?pos.inner_offset(),
                        "SubAssign: about to subtract delta from sub_index"
                    );
                    assert!(
                        sub_index >= delta,
                        "Cannot subtract delta {} from sub_index {} for pattern {:?}. \
                         This likely means the offset is BEFORE the merged range \
                         and shouldn't have delta applied.",
                        delta, sub_index, pid
                    );
                    *pos.sub_index_mut() -= delta;
                }
            });
    }
}

impl Borrow<TokenTracePositions> for SplitPositionCache {
    fn borrow(&self) -> &TokenTracePositions {
        &self.pattern_splits
    }
}

impl Borrow<TokenTracePositions> for &SplitPositionCache {
    fn borrow(&self) -> &TokenTracePositions {
        &self.pattern_splits
    }
}

impl BorrowMut<TokenTracePositions> for SplitPositionCache {
    fn borrow_mut(&mut self) -> &mut TokenTracePositions {
        &mut self.pattern_splits
    }
}

impl SplitPositionCache {
    pub fn root(subs: impl ToVertexSplitPos) -> Self {
        Self {
            top: HashSet::default(),
            pattern_splits: subs.to_vertex_split_pos(),
        }
    }
    pub fn new(
        prev: PosKey,
        subs: Vec<SubSplitLocation>,
    ) -> Self {
        Self {
            top: HashSet::from_iter(Some(prev)),
            pattern_splits: subs.into_iter().map(Into::into).collect(),
        }
    }
    pub fn find_clean_split(&self) -> Option<SubLocation> {
        self.pattern_splits.iter().find_map(|(pid, s)| {
            s.inner_offset
                .is_none()
                .then_some(SubLocation::new(*pid, s.sub_index))
        })
    }
    
    /// Apply delta adjustment with inner_offset for positions inside a merged region.
    ///
    /// This is called for positions that fall INSIDE a merged token (not at its boundary).
    /// In addition to adjusting sub_index, this also sets the inner_offset to indicate
    /// the position within the merged token.
    pub fn apply_delta_with_inner_offset(
        &mut self, 
        deltas: &PatternSubDeltas, 
        inner_offset: NonZeroUsize,
    ) {
        self.pattern_splits
            .iter_mut()
            .for_each(|(pid, pos)| {
                if let Some(&delta) = deltas.get(pid) {
                    let sub_index = pos.sub_index();
                    tracing::debug!(
                        ?pid,
                        sub_index,
                        delta,
                        ?inner_offset,
                        "apply_delta_with_inner_offset: adjusting position inside merged region"
                    );
                    assert!(
                        sub_index >= delta,
                        "Cannot subtract delta {} from sub_index {} for pattern {:?}.",
                        delta, sub_index, pid
                    );
                    *pos.sub_index_mut() -= delta;
                    // Set inner_offset to indicate this position is inside the merged token
                    *pos.inner_offset_mut() = Some(inner_offset);
                }
            });
    }
    //pub fn add_location_split(&mut self, location: SubLocation, split: Split) {
    //    self.pattern_splits.insert(location, split);
    //}
    //pub fn join_splits(&mut self, indexer: &mut Indexer, key: &PosKey) -> Split {
    //    let (l, r): (Vec<_>, Vec<_>) = self.pattern_splits
    //        .drain()
    //        .map(|(_, s)| (s.left, s.right))
    //        .unzip();
    //    // todo detect existing splits
    //    let mut graph = indexer.graph_mut();
    //    let lc = graph.insert_patterns(l);
    //    let rc = graph.insert_patterns(r);
    //    graph.add_pattern_with_update(&key.index, vec![lc, rc]);
    //    let split = Split {
    //        left: vec![lc],
    //        right: vec![rc],
    //    };
    //    self.final_split = Some(split.clone());
    //    split
    //}
}
//impl From<Split> for SplitPositionCache {
//    fn from(split: Split) -> Self {
//        Self {
//            pattern_splits: Default::default(),
//            final_split: Some(split),
//        }
//    }
//}
