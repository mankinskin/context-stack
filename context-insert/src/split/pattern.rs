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
    //fn offsets(&self) -> Self::Offsets {
    //    (self.0.offsets(), self.1.offsets())
    //}
}
