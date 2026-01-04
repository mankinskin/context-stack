use std::num::NonZeroUsize;

use context_trace::*;

use std::fmt::Debug;

use crate::*;

pub trait PatternSplits: Debug + Clone {
    type Pos;
    type Offsets;
    fn get(
        &self,
        pid: &PatternId,
    ) -> Option<Self::Pos>;
    fn ids<'a>(&'a self) -> Box<dyn Iterator<Item = &'a PatternId> + 'a>;
    /// Get the atom position(s) for this split
    fn atom_pos(&self) -> Option<NonZeroUsize>;
    /// Get both atom positions for Infix splits (left, right)
    fn atom_pos_pair(&self) -> Option<(NonZeroUsize, NonZeroUsize)> {
        None
    }
    //fn offsets(&self) -> Self::Offsets;
}

impl PatternSplits for VertexSplits {
    type Pos = TokenTracePos;
    type Offsets = usize;
    fn get(
        &self,
        pid: &PatternId,
    ) -> Option<Self::Pos> {
        self.splits.get(pid).cloned()
    }
    fn ids<'a>(&'a self) -> Box<dyn Iterator<Item = &'a PatternId> + 'a> {
        Box::new(self.splits.keys())
    }
    fn atom_pos(&self) -> Option<NonZeroUsize> {
        Some(self.pos)
    }
    //fn offsets(&self) -> Self::Offsets {
    //    self.pos.get()
    //}
}

impl PatternSplits for &VertexSplits {
    type Pos = TokenTracePos;
    type Offsets = usize;
    fn get(
        &self,
        pid: &PatternId,
    ) -> Option<Self::Pos> {
        self.splits.get(pid).cloned()
    }
    fn ids<'a>(&'a self) -> Box<dyn Iterator<Item = &'a PatternId> + 'a> {
        Box::new(self.splits.keys())
    }
    fn atom_pos(&self) -> Option<NonZeroUsize> {
        Some(self.pos)
    }
    //fn offsets(&self) -> Self::Offsets {
    //    self.pos.get()
    //}
}

impl<A: PatternSplits, B: PatternSplits> PatternSplits for (A, B) {
    type Pos = (A::Pos, B::Pos);
    type Offsets = (A::Offsets, B::Offsets);
    fn get(
        &self,
        pid: &PatternId,
    ) -> Option<Self::Pos> {
        self.0.get(pid).map(|a| {
            let b = self.1.get(pid).unwrap();
            (a, b)
        })
    }
    fn ids<'a>(&'a self) -> Box<dyn Iterator<Item = &'a PatternId> + 'a> {
        self.0.ids()
    }
    fn atom_pos(&self) -> Option<NonZeroUsize> {
        // For tuples, we can't return a single position
        // This method is mainly used for Pre/Post modes
        None
    }
    fn atom_pos_pair(&self) -> Option<(NonZeroUsize, NonZeroUsize)> {
        match (self.0.atom_pos(), self.1.atom_pos()) {
            (Some(left), Some(right)) => Some((left, right)),
            _ => None,
        }
    }
    //fn offsets(&self) -> Self::Offsets {
    //    (self.0.offsets(), self.1.offsets())
    //}
}
