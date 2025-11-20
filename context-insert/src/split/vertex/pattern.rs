use derivative::Derivative;

use context_trace::*;
use derive_new::new;

use crate::split::vertex::node::NodeTraceCtx;

#[derive(Debug, Clone, Derivative, new)]
#[derivative(Hash, PartialEq, Eq)]
pub struct PatternTraceCtx<'a> {
    pub(crate) loc: PatternLocation,
    #[derivative(Hash = "ignore", PartialEq = "ignore")]
    pub(crate) pattern: &'a Pattern,
}

impl<'p> From<PatternTraceCtx<'p>> for PatternId {
    fn from(value: PatternTraceCtx<'p>) -> Self {
        value.loc.pattern_id
    }
}

pub trait HasPatternTraceCtx {
    fn pattern_trace_context<'a>(&'a self) -> PatternTraceCtx<'a>
    where
        Self: 'a;
}
impl HasPatternTraceCtx for PatternTraceCtx<'_> {
    fn pattern_trace_context<'a>(&'a self) -> PatternTraceCtx<'a>
    where
        Self: 'a,
    {
        self.clone()
    }
}
pub trait GetPatternTraceCtx {
    fn get_pattern_trace_context<'b>(
        &'b self,
        pattern_id: &PatternId,
    ) -> PatternTraceCtx<'b>
    where
        Self: 'b;
}
pub trait GetPatternCtx {
    type PatternCtx<'b>: HasPatternTraceCtx
    where
        Self: 'b;
    fn get_pattern_context<'b>(
        &'b self,
        pattern_id: &PatternId,
    ) -> Self::PatternCtx<'b>
    where
        Self: 'b;
}

impl GetPatternCtx for NodeTraceCtx<'_> {
    type PatternCtx<'b>
        = PatternTraceCtx<'b>
    where
        Self: 'b;
    fn get_pattern_context<'b>(
        &'b self,
        pattern_id: &PatternId,
    ) -> Self::PatternCtx<'b>
    where
        Self: 'b,
    {
        self.get_pattern_trace_context(pattern_id)
    }
}
impl GetPatternTraceCtx for NodeTraceCtx<'_> {
    fn get_pattern_trace_context<'b>(
        &'b self,
        pattern_id: &PatternId,
    ) -> PatternTraceCtx<'b>
    where
        Self: 'b,
    {
        PatternTraceCtx {
            loc: self.index.to_pattern_location(*pattern_id),
            pattern: self.patterns.get(pattern_id).unwrap(),
        }
    }
}
