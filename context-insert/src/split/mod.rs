pub mod cache;
pub(crate) mod context;
pub(crate) mod pattern;
pub(crate) mod run;
pub(crate) mod trace;
pub(crate) mod vertex;

use std::{
    cmp::Ordering,
    fmt::Debug,
    num::NonZeroUsize,
};

use cache::position::PosKey;
use context_trace::*;
use derive_new::new;
use vertex::VertexSplits;

use crate::*;

#[derive(Debug, Clone, Eq, PartialEq, new)]
pub struct TokenTracePos {
    pub(crate) inner_offset: Option<NonZeroUsize>,
    pub(crate) sub_index: usize,
}
impl HasInnerOffset for TokenTracePos {
    fn inner_offset(&self) -> Option<NonZeroUsize> {
        self.inner_offset
    }
}
impl HasSubIndex for TokenTracePos {
    fn sub_index(&self) -> usize {
        self.sub_index
    }
}
impl HasSubIndexMut for TokenTracePos {
    fn sub_index_mut(&mut self) -> &mut usize {
        &mut self.sub_index
    }
}
impl TokenTracePos {
    /// Get mutable reference to inner_offset
    pub(crate) fn inner_offset_mut(&mut self) -> &mut Option<NonZeroUsize> {
        &mut self.inner_offset
    }
}
impl From<(usize, Option<NonZeroUsize>)> for TokenTracePos {
    fn from((sub_index, inner_offset): (usize, Option<NonZeroUsize>)) -> Self {
        Self {
            sub_index,
            inner_offset,
        }
    }
}

/// Side refers to border (front is indexing before front border, back is indexing after back border)
pub trait TraceSide:
    std::fmt::Debug + Sync + Send + Unpin + Clone + 'static
{
    fn trace_child_pos(
        pattern: impl IntoPattern,
        offset: NonZeroUsize,
    ) -> Option<TokenTracePos>;
}

/// for insert
#[derive(Debug, Clone)]
pub struct TraceBack;

impl TraceSide for TraceBack {
    fn trace_child_pos(
        pattern: impl IntoPattern,
        offset: NonZeroUsize,
    ) -> Option<TokenTracePos> {
        let mut offset = offset.get();
        pattern
            .into_pattern()
            .into_iter()
            .enumerate()
            .find_map(|(i, c)|
            // returns current index when remaining offset is smaller than current token
            match c.width().cmp(&TokenWidth(offset)) {
                Ordering::Less => {
                    offset -= *c.width();
                    None
                }
                Ordering::Equal => {
                    offset = 0;
                    None
                }
                Ordering::Greater => Some((i, NonZeroUsize::new(offset))),
            })
            .map(Into::into)
    }
}

#[derive(Debug, Clone)]
pub struct TraceFront;

impl TraceSide for TraceFront {
    fn trace_child_pos(
        pattern: impl IntoPattern,
        offset: NonZeroUsize,
    ) -> Option<TokenTracePos> {
        let mut offset = offset.get();
        pattern
            .into_pattern()
            .into_iter()
            .enumerate()
            .find_map(|(i, c)|
            // returns current index when remaining offset does not exceed current token
            match c.width().cmp(&TokenWidth(offset)) {
                Ordering::Less => {
                    offset -= *c.width();
                    None
                }
                Ordering::Equal => {
                    offset = 0;
                    Some((i, NonZeroUsize::new(offset)))
                }
                Ordering::Greater => Some((i, NonZeroUsize::new(offset))),
            })
            .map(Into::into)
    }
}

pub(crate) fn position_splits<'a>(
    patterns: impl IntoIterator<Item = (&'a PatternId, &'a Pattern)>,
    pos: NonZeroUsize,
) -> VertexSplits {
    VertexSplits {
        pos,
        splits: patterns
            .into_iter()
            .map(|(pid, pat)| {
                let pos = TraceBack::trace_child_pos(pat, pos).unwrap();
                (*pid, pos)
            })
            .collect(),
    }
}

pub(crate) fn range_splits<'a>(
    patterns: impl Iterator<Item = (&'a PatternId, &'a Pattern)>,
    parent_range: (NonZeroUsize, NonZeroUsize),
) -> (VertexSplits, VertexSplits) {
    let (ls, rs) = patterns
        .map(|(pid, pat)| {
            let lpos = TraceBack::trace_child_pos(pat, parent_range.0).unwrap();
            let rpos = TraceBack::trace_child_pos(pat, parent_range.1).unwrap();
            ((*pid, lpos), (*pid, rpos))
        })
        .unzip();
    (
        VertexSplits {
            pos: parent_range.0,
            splits: ls,
        },
        VertexSplits {
            pos: parent_range.1,
            splits: rs,
        },
    )
}

pub(crate) fn cleaned_position_splits<'a>(
    patterns: impl Iterator<Item = (&'a PatternId, &'a Pattern)>,
    parent_offset: NonZeroUsize,
) -> Result<Vec<SubSplitLocation>, SubLocation> {
    patterns
        .map(|(pid, pat)| {
            let pos = TraceBack::trace_child_pos(pat, parent_offset).unwrap();
            let location = SubLocation::new(*pid, pos.sub_index());
            if pos.inner_offset().is_some() || pat.len() > 2 {
                // can't be clean
                Ok(SubSplitLocation::new(location, pos.inner_offset()))
            } else {
                // must be clean
                Err(location)
            }
        })
        .collect()
}

pub(crate) trait SplitInner: Debug + Clone {}

impl<T: Debug + Clone> SplitInner for T {}

pub(crate) type SplitMap = HashMap<PosKey, Split>;

#[derive(Debug, Clone)]
pub(crate) struct Split<T: SplitInner = Token> {
    pub(crate) left: T,
    pub(crate) right: T,
}

impl<T: SplitInner> Split<T> {
    pub(crate) fn new(
        left: T,
        right: T,
    ) -> Self {
        Self { left, right }
    }
}

//impl<I, T: SplitInner + Extend<I> + IntoIterator<Item = I>> Split<T> {
//    pub(crate) fn infix(
//        &mut self,
//        mut inner: Split<T>,
//    ) {
//        self.left.extend(inner.left);
//        inner.right.extend(self.right.clone());
//        self.right = inner.right;
//    }
//}

//pub(crate) trait HasSplitMap {
//    fn split_map(&self) -> &SplitMap;
//}
//
//impl HasSplitMap for SplitMap {
//    fn split_map(&self) -> &SplitMap {
//        self
//    }
//}
//impl HasSplitMap for PosSplits<SplitVertexCache> {
//    fn split_map(&self) -> &SubSplits {
//        &self.into_iter().collect()
//    }
//}
