use derivative::Derivative;
use derive_more::derive::{
    Deref,
    DerefMut,
};

use crate::split::{
    SplitMap,
    vertex::pattern::{
        HasPatternTraceCtx,
        PatternTraceCtx,
    },
};
use context_trace::*;

pub(crate) mod borders;

/// Pattern join context that owns its data.
/// 
/// With interior mutability, we can't hold references across lock boundaries,
/// so this struct owns the pattern and split map data.
#[derive(Debug, Clone, Deref, DerefMut, Derivative)]
#[derivative(Hash, PartialEq, Eq)]
pub(crate) struct PatternJoinCtx {
    #[deref]
    #[deref_mut]
    pub(crate) ctx: PatternTraceCtx,
    #[derivative(Hash = "ignore", PartialEq = "ignore")]
    pub(crate) splits: SplitMap,
}

impl HasPatternTraceCtx for PatternJoinCtx {
    fn pattern_trace_context(&self) -> PatternTraceCtx {
        self.ctx.clone()
    }
}

impl From<PatternJoinCtx> for PatternId {
    fn from(value: PatternJoinCtx) -> Self {
        Self::from(value.ctx)
    }
}
